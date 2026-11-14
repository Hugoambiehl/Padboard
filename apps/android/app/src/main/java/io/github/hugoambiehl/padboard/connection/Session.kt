package io.github.hugoambiehl.padboard.connection

import io.github.hugoambiehl.padboard.protocol.Message
import io.github.hugoambiehl.padboard.protocol.parseMessage
import io.github.hugoambiehl.padboard.protocol.toJson
import java.util.concurrent.atomic.AtomicBoolean
import java.util.logging.Level
import java.util.logging.Logger
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.channels.ReceiveChannel
import kotlinx.serialization.SerializationException
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString

/** What happens on a [Session], in order. [Closed] is always the last event. */
sealed interface SessionEvent {
    data object Opened : SessionEvent

    data class Received(val message: Message) : SessionEvent

    /** A binary frame (the pieces of an update). */
    class Bytes(val bytes: ByteArray) : SessionEvent

    data class Closed(val failure: Throwable?) : SessionEvent
}

/**
 * One WebSocket connection to the PC. A thin wrapper turning OkHttp callbacks
 * into a channel of [SessionEvent]s; the protocol logic is in [ConnectionManager].
 */
class Session private constructor() {
    private val channel = Channel<SessionEvent>(Channel.UNLIMITED)
    private val closed = AtomicBoolean(false)
    private lateinit var socket: WebSocket

    val events: ReceiveChannel<SessionEvent> get() = channel

    fun send(message: Message): Boolean = socket.send(message.toJson())

    /** Graceful close; [SessionEvent.Closed] follows. */
    fun close() {
        socket.close(NORMAL_CLOSURE, null)
        finish(null)
    }

    /** Immediate close (dead connection). */
    fun cancel() {
        socket.cancel()
        finish(null)
    }

    private fun finish(failure: Throwable?) {
        if (closed.compareAndSet(false, true)) {
            channel.trySend(SessionEvent.Closed(failure))
            channel.close()
        }
    }

    private inner class Listener : WebSocketListener() {
        override fun onOpen(webSocket: WebSocket, response: Response) {
            channel.trySend(SessionEvent.Opened)
        }

        override fun onMessage(webSocket: WebSocket, text: String) {
            val message =
                try {
                    parseMessage(text)
                } catch (e: SerializationException) {
                    // A newer PC may send messages this version does not know.
                    log.log(Level.WARNING, "ignoring unreadable message", e)
                    return
                } catch (e: IllegalArgumentException) {
                    log.log(Level.WARNING, "ignoring unreadable message", e)
                    return
                }
            channel.trySend(SessionEvent.Received(message))
        }

        override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
            channel.trySend(SessionEvent.Bytes(bytes.toByteArray()))
        }

        override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
            webSocket.close(NORMAL_CLOSURE, null)
            finish(null)
        }

        override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
            finish(t)
        }
    }

    companion object {
        private const val NORMAL_CLOSURE = 1000
        private val log: Logger = Logger.getLogger(Session::class.java.name)

        fun open(client: OkHttpClient, url: String): Session {
            val session = Session()
            session.socket = client.newWebSocket(Request.Builder().url(url).build(), session.Listener())
            return session
        }
    }
}
