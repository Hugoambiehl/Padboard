package io.github.hugoambiehl.padboard.connection

import io.github.hugoambiehl.padboard.protocol.ButtonEvent
import io.github.hugoambiehl.padboard.protocol.ErrorCode
import io.github.hugoambiehl.padboard.protocol.HelloPayload
import io.github.hugoambiehl.padboard.protocol.Layout
import io.github.hugoambiehl.padboard.protocol.Message
import io.github.hugoambiehl.padboard.protocol.PROTOCOL_VERSION
import io.github.hugoambiehl.padboard.protocol.parseMessage
import io.github.hugoambiehl.padboard.protocol.toJson
import java.util.logging.Level
import java.util.logging.Logger
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.serialization.SerializationException
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener

/** Why a connection ended without the user asking for it. */
enum class FailureReason {
    /** The socket could not be opened (wrong address, app not running, no `adb reverse`…). */
    Unreachable,

    /** The desktop speaks another protocol version. */
    Incompatible,

    /** The desktop refused the handshake (malformed hello…). */
    Rejected,

    /** The connection dropped after being established. */
    Lost
}

sealed interface ConnectionState {
    data object Idle : ConnectionState

    data class Connecting(val address: ServerAddress) : ConnectionState

    data class Connected(val address: ServerAddress, val serverName: String) : ConnectionState

    data class Failed(val address: ServerAddress, val reason: FailureReason) : ConnectionState
}

/**
 * WebSocket client for the desktop app. A "dumb" client: it shows the layout it
 * receives and reports presses; it holds no business logic.
 *
 * Thread-safe: OkHttp delivers callbacks on its own threads, state is exposed
 * through flows. Phase 1: no automatic reconnection or heartbeat yet.
 */
class PadboardClient(private val okHttp: OkHttpClient, private val deviceName: String) {
    private val _state = MutableStateFlow<ConnectionState>(ConnectionState.Idle)
    val state: StateFlow<ConnectionState> = _state.asStateFlow()

    private val _layout = MutableStateFlow<Layout?>(null)

    /** Last layout received, `null` until the first `layout:sync`. */
    val layout: StateFlow<Layout?> = _layout.asStateFlow()

    private val _actionFailures =
        MutableSharedFlow<String>(extraBufferCapacity = 8, onBufferOverflow = BufferOverflow.DROP_OLDEST)

    /** Emits the desktop's (English, developer-oriented) message each time an action fails on the PC. */
    val actionFailures: SharedFlow<String> = _actionFailures.asSharedFlow()

    private val lock = Any()
    private var socket: WebSocket? = null

    /** [deviceId]: stable identifier of this tablet, see [DeviceIdStore]. */
    fun connect(address: ServerAddress, deviceId: String) {
        synchronized(lock) {
            socket?.cancel()
            _layout.value = null
            _state.value = ConnectionState.Connecting(address)
            val request = Request.Builder().url(address.webSocketUrl).build()
            socket = okHttp.newWebSocket(request, Listener(address, deviceId))
        }
    }

    fun disconnect() {
        synchronized(lock) {
            socket?.close(NORMAL_CLOSURE, null)
            socket = null
            _layout.value = null
            _state.value = ConnectionState.Idle
        }
    }

    fun press(buttonId: String) = send(Message.ButtonPress(ButtonEvent(buttonId)))

    fun release(buttonId: String) = send(Message.ButtonRelease(ButtonEvent(buttonId)))

    private fun send(message: Message) {
        val ws = synchronized(lock) { socket.takeIf { _state.value is ConnectionState.Connected } }
        if (ws == null || !ws.send(message.toJson())) {
            log.fine("dropped ${message::class.simpleName}: not connected")
        }
    }

    /** Ignores callbacks from a socket that has since been replaced or closed. */
    private inline fun ifCurrent(webSocket: WebSocket, block: () -> Unit) {
        synchronized(lock) { if (socket === webSocket) block() }
    }

    private fun fail(webSocket: WebSocket, address: ServerAddress, reason: FailureReason) = ifCurrent(webSocket) {
        socket = null
        _layout.value = null
        _state.value = ConnectionState.Failed(address, reason)
    }

    private inner class Listener(private val address: ServerAddress, private val deviceId: String) :
        WebSocketListener() {
        override fun onOpen(webSocket: WebSocket, response: Response) {
            // Pairing and tokens arrive in phase 2 (step G); the desktop accepts everyone until then.
            webSocket.send(Message.Hello(HelloPayload(PROTOCOL_VERSION, deviceName, deviceId)).toJson())
        }

        override fun onMessage(webSocket: WebSocket, text: String) {
            val message =
                try {
                    parseMessage(text)
                } catch (e: SerializationException) {
                    // A newer desktop may send messages this version does not know: ignore them.
                    log.log(Level.WARNING, "ignoring unreadable message", e)
                    return
                } catch (e: IllegalArgumentException) {
                    log.log(Level.WARNING, "ignoring unreadable message", e)
                    return
                }
            handle(webSocket, message)
        }

        private fun handle(webSocket: WebSocket, message: Message) {
            when (message) {
                is Message.Welcome ->
                    ifCurrent(webSocket) {
                        _state.value = ConnectionState.Connected(address, message.payload.serverName)
                    }
                is Message.LayoutSync -> ifCurrent(webSocket) { _layout.value = message.payload }
                is Message.Error -> handleError(webSocket, message)
                is Message.Ping -> webSocket.send(Message.Pong(message.payload).toJson())
                is Message.Pong,
                is Message.Hello,
                is Message.ButtonPress,
                is Message.ButtonRelease,
                is Message.PairRequest,
                is Message.PairSuccess,
                is Message.PairFailure -> Unit
            }
        }

        private fun handleError(webSocket: WebSocket, message: Message.Error) {
            val error = message.payload
            log.warning("desktop error ${error.code}: ${error.message}")
            val connected = _state.value is ConnectionState.Connected
            when {
                error.code == ErrorCode.UnsupportedVersion -> fail(webSocket, address, FailureReason.Incompatible)
                error.code == ErrorCode.ActionFailed -> _actionFailures.tryEmit(error.message)
                // Before the welcome, any error means the handshake was refused.
                !connected -> fail(webSocket, address, FailureReason.Rejected)
                else -> Unit
            }
        }

        override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
            webSocket.close(NORMAL_CLOSURE, null)
            fail(webSocket, address, closedReason())
        }

        override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
            log.log(Level.INFO, "connection to ${address.display} failed", t)
            fail(webSocket, address, closedReason())
        }

        /** Unreachable if we never got the welcome, lost otherwise. */
        private fun closedReason(): FailureReason = when (val s = _state.value) {
            is ConnectionState.Connected -> FailureReason.Lost
            is ConnectionState.Failed -> s.reason
            else -> FailureReason.Unreachable
        }
    }

    private companion object {
        const val NORMAL_CLOSURE = 1000
        val log: Logger = Logger.getLogger(PadboardClient::class.java.name)
    }
}
