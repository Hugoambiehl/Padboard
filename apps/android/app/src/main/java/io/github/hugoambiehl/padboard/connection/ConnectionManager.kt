package io.github.hugoambiehl.padboard.connection

import io.github.hugoambiehl.padboard.protocol.AuthStatus
import io.github.hugoambiehl.padboard.protocol.ButtonEvent
import io.github.hugoambiehl.padboard.protocol.ErrorCode
import io.github.hugoambiehl.padboard.protocol.Fingerprint
import io.github.hugoambiehl.padboard.protocol.Heartbeat
import io.github.hugoambiehl.padboard.protocol.HelloPayload
import io.github.hugoambiehl.padboard.protocol.Layout
import io.github.hugoambiehl.padboard.protocol.Message
import io.github.hugoambiehl.padboard.protocol.NavigatePayload
import io.github.hugoambiehl.padboard.protocol.PROTOCOL_VERSION
import io.github.hugoambiehl.padboard.protocol.PairFailureReason
import io.github.hugoambiehl.padboard.protocol.PairRequestPayload
import io.github.hugoambiehl.padboard.protocol.WelcomePayload
import io.github.hugoambiehl.padboard.protocol.withButton
import java.util.Base64
import java.util.logging.Logger
import kotlin.time.Duration
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds
import kotlin.time.TimeSource
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.selects.select
import kotlinx.coroutines.withTimeoutOrNull
import okhttp3.OkHttpClient

enum class Transport { Usb, Wifi }

/** What the UI shows. */
sealed interface LinkState {
    /** Starting up. */
    data object Starting : LinkState

    /** No paired PC and no USB: the setup screen. */
    data object Unpaired : LinkState

    /**
     * Looking for the PC: the paired one (USB or Wi-Fi), or the one last
     * reached over USB in this run ([wifi] false: only the cable can bring it
     * back). The next attempt starts in [retryInMs]; [attempt] counts them so
     * each one is a new state.
     */
    data class Searching(
        val serverName: String,
        val wifi: Boolean = true,
        val retryInMs: Long = 0,
        val attempt: Int = 0
    ) : LinkState

    data class Connected(
        val transport: Transport,
        val serverName: String,
        /** Paired over Wi-Fi with this PC (else: USB only). */
        val paired: Boolean,
        val latencyMs: Int?
    ) : LinkState

    /** Pairing with [serverName]: the user types the PIN shown on the PC. */
    data class Pairing(
        val serverName: String,
        /** Short fingerprint of the key received over TLS, to compare with the PC. */
        val fingerprint: String,
        val failure: PairFailureReason? = null,
        val attemptsLeft: Int? = null,
        val submitting: Boolean = false
    ) : LinkState

    /** The PC speaks another protocol version: both apps must be updated. */
    data object Incompatible : LinkState
}

/** Why a pairing attempt stopped before the PIN screen or during it. */
enum class PairingProblem {
    /** No address answered (or not with the expected PC). */
    Unreachable,

    /** The connection dropped (PC closed, PIN screen left open too long…). */
    Lost
}

/**
 * Keeps the tablet connected to its PC (see CLAUDE.md, "Connection"):
 * - **USB first** (`ws://127.0.0.1`, through `adb reverse`): no pairing;
 * - else **Wi-Fi** to the paired PC (`wss://`, key pinned, token): every
 *   address known for it (mDNS, then the last one that worked) is tried in
 *   order; an address only counts if it answers with the paired `serverId`
 *   and the pinned key;
 * - while on Wi-Fi, USB is probed regularly: once it answers, the tablet
 *   switches to it **before** closing Wi-Fi (no gap; the PC closes the older
 *   session with `superseded`); when USB drops, Wi-Fi takes over at once;
 * - reconnection with backoff, `ping` heartbeat (latency), dead links detected.
 *
 * All the work runs on one thread ([dispatcher]), so the state needs no locks.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class ConnectionManager(
    parentScope: CoroutineScope,
    private val http: OkHttpClient,
    private val pairing: PairingRepository,
    private val directory: PcDirectory,
    private val deviceName: String,
    private val config: Config = Config()
) {
    data class Config(
        val usbEndpoint: Endpoint = Endpoint(Endpoint.LOOPBACK, Endpoint.USB_PORT),
        /** Time for the PC to answer `hello`. */
        val handshakeTimeout: Duration = 4.seconds,
        val usbProbeEvery: Duration = 2.seconds,
        val pingEvery: Duration = 5.seconds,
        /** No message for this long: the link is dead. */
        val deadAfter: Duration = 15.seconds,
        val retryMin: Duration = 500.milliseconds,
        val retryMax: Duration = 5.seconds
    )

    private val dispatcher = Dispatchers.Default.limitedParallelism(1)
    private val scope = CoroutineScope(
        parentScope.coroutineContext + Job(parentScope.coroutineContext[Job]) + dispatcher
    )

    private val _state = MutableStateFlow<LinkState>(LinkState.Starting)
    val state: StateFlow<LinkState> = _state.asStateFlow()

    private val _layout = MutableStateFlow<Layout?>(null)

    /** Last layout received; kept while reconnecting so the grid stays visible. */
    val layout: StateFlow<Layout?> = _layout.asStateFlow()

    private val _navigations =
        MutableSharedFlow<NavigatePayload>(extraBufferCapacity = 8, onBufferOverflow = BufferOverflow.DROP_OLDEST)

    /** `navigate` requests from the PC (a navigation button was pressed). */
    val navigations: SharedFlow<NavigatePayload> = _navigations.asSharedFlow()

    private val _actionFailures =
        MutableSharedFlow<String>(extraBufferCapacity = 8, onBufferOverflow = BufferOverflow.DROP_OLDEST)
    val actionFailures: SharedFlow<String> = _actionFailures.asSharedFlow()

    private val _pairingProblems =
        MutableSharedFlow<PairingProblem>(extraBufferCapacity = 4, onBufferOverflow = BufferOverflow.DROP_OLDEST)
    val pairingProblems: SharedFlow<PairingProblem> = _pairingProblems.asSharedFlow()

    /** The session presses go to (read from the UI thread). */
    @Volatile private var active: Active? = null

    private var deviceId: String = ""

    /** The PC reached over USB in this run, looked for again when the cable goes. */
    private var lastUsbServer: String? = null
    private var job: Job? = null
    private val wakeUp = Channel<Unit>(Channel.CONFLATED)
    private val pins = Channel<String>(Channel.CONFLATED)

    private class Active(val session: Session, val transport: Transport, val welcome: WelcomePayload)

    private sealed interface End {
        /** The link dropped: reconnect. */
        data object Lost : End

        /** USB came up while on Wi-Fi: carry on with it. */
        class SwitchedTo(val next: Active) : End

        data object Revoked : End

        data object Incompatible : End

        /** Another session of this tablet replaced this one (normally: our USB probe). */
        data object Superseded : End
    }

    private sealed interface PairingStep {
        class Pin(val pin: String) : PairingStep

        class Event(val event: SessionEvent?) : PairingStep
    }

    // ─── Public API (any thread) ────────────────────────────────────────

    fun start(deviceId: String) {
        scope.launch {
            this@ConnectionManager.deviceId = deviceId
            restart { auto(initial = null) }
        }
    }

    fun press(buttonId: String) = send(Message.ButtonPress(ButtonEvent(buttonId)))

    fun release(buttonId: String) = send(Message.ButtonRelease(ButtonEvent(buttonId)))

    /** Pairs over Wi-Fi with a discovered PC (its addresses are tried in order). */
    fun pair(pc: DiscoveredPc) {
        scope.launch { restart { pairWith(pc.endpoints, expectedServerId = pc.serverId) } }
    }

    /** Pairs over Wi-Fi with a PC typed by hand. */
    fun pair(endpoint: Endpoint) {
        scope.launch { restart { pairWith(listOf(endpoint), expectedServerId = null) } }
    }

    fun submitPin(pin: String) {
        pins.trySend(pin)
    }

    fun cancelPairing() {
        scope.launch { restart { auto(initial = null) } }
    }

    /** Forgets the paired PC (its token and key); USB keeps working. */
    fun forget() {
        scope.launch {
            restart {
                pairing.clear()
                lastUsbServer = null
                auto(initial = null)
            }
        }
    }

    /** Something changed (a PC appeared on the network…): retry now. */
    fun nudge() {
        wakeUp.trySend(Unit)
    }

    /** After an incompatible PC was updated. */
    fun retry() = cancelPairing()

    private fun send(message: Message) {
        val session = active?.session
        if (session == null || !session.send(message)) log.fine("dropped ${message::class.simpleName}: not connected")
    }

    /** Stops the current job (closing its session) and runs [block] instead. */
    private suspend fun restart(block: suspend CoroutineScope.() -> Unit) {
        job?.cancelAndJoin()
        job = scope.launch(block = block)
    }

    // ─── Auto mode ──────────────────────────────────────────────────────

    private suspend fun auto(initial: Active?) {
        var current = initial
        var retry = config.retryMin
        var attempt = 0
        while (scope.isActive) {
            if (current == null) current = tryUsb() ?: tryWifi()
            if (current != null) {
                retry = config.retryMin
                attempt = 0
                val started = TimeSource.Monotonic.markNow()
                current =
                    when (val end = hold(current)) {
                        is End.SwitchedTo -> end.next
                        End.Revoked -> {
                            log.info("pairing revoked by the PC")
                            pairing.clear()
                            lastUsbServer = null
                            null
                        }
                        End.Incompatible -> {
                            _state.value = LinkState.Incompatible
                            return
                        }
                        End.Lost, End.Superseded -> {
                            // A link that keeps dropping right away must not spin.
                            if (started.elapsedNow() < config.pingEvery) delay(config.retryMin)
                            null
                        }
                    }
                continue
            }
            val paired = pairing.load()
            val name = paired?.serverName ?: lastUsbServer
            attempt++
            _state.value =
                name?.let {
                    LinkState.Searching(
                        it,
                        wifi = paired != null,
                        retryInMs = retry.inWholeMilliseconds,
                        attempt = attempt
                    )
                } ?: LinkState.Unpaired
            withTimeoutOrNull(retry) { wakeUp.receive() }
            retry = (retry * 2).coerceAtMost(config.retryMax)
        }
    }

    private fun hello(token: String?) = Message.Hello(HelloPayload(PROTOCOL_VERSION, deviceName, deviceId, token))

    /** Opens a session, sends `hello` and waits for `welcome`. */
    private suspend fun handshake(session: Session, token: String?): WelcomePayload? {
        val result =
            withTimeoutOrNull(config.handshakeTimeout) {
                for (event in session.events) {
                    when (event) {
                        SessionEvent.Opened -> session.send(hello(token))
                        is SessionEvent.Received -> {
                            val message = event.message
                            if (message is Message.Welcome) return@withTimeoutOrNull message.payload
                            if (message is Message.Error && message.payload.code == ErrorCode.UnsupportedVersion) {
                                _state.value = LinkState.Incompatible
                            }
                            return@withTimeoutOrNull null
                        }
                        is SessionEvent.Closed -> return@withTimeoutOrNull null
                    }
                }
                null
            }
        if (result == null) session.cancel()
        return result
    }

    private suspend fun tryUsb(): Active? {
        val session = Session.open(http, "ws://${config.usbEndpoint.display}/")
        // USB needs no token: never send it outside TLS.
        val welcome = handshake(session, token = null) ?: return null
        if (welcome.auth != AuthStatus.Authenticated) {
            session.cancel()
            return null
        }
        return Active(session, Transport.Usb, welcome)
    }

    private suspend fun tryWifi(): Active? {
        val pc = pairing.load() ?: return null
        val client = http.pinnedTo(pc.pin)
        for (endpoint in candidates(pc)) {
            val session = Session.open(client, "wss://${endpoint.display}/")
            // A pinning failure closes the session: handshake() returns null.
            val welcome = handshake(session, pc.token) ?: continue
            if (welcome.serverId != pc.serverId) {
                session.cancel()
                continue
            }
            when (welcome.auth) {
                AuthStatus.Authenticated -> {
                    if (endpoint != pc.lastEndpoint) pairing.updateEndpoint(endpoint)
                    return Active(session, Transport.Wifi, welcome)
                }
                AuthStatus.TokenRejected -> {
                    log.info("the paired PC rejected the token: pairing again is needed")
                    session.cancel()
                    pairing.clear()
                    return null
                }
                AuthStatus.PairingRequired -> session.cancel()
            }
        }
        return null
    }

    /** mDNS addresses of the paired PC in their order, then the last one that worked. */
    private fun candidates(pc: PairedPc): List<Endpoint> = (
        directory.pcs.value.filter { it.serverId == pc.serverId }.flatMap { it.endpoints } +
            listOfNotNull(pc.lastEndpoint)
        )
        .distinct()

    // ─── Connected ──────────────────────────────────────────────────────

    /** Runs an authenticated session until it ends. */
    private suspend fun hold(link: Active): End = coroutineScope {
        val paired = pairing.load()?.serverId == link.welcome.serverId
        active = link
        if (link.transport == Transport.Usb) lastUsbServer = link.welcome.serverName
        _state.value = LinkState.Connected(link.transport, link.welcome.serverName, paired, latencyMs = null)
        log.info("connected to ${link.welcome.serverName} over ${link.transport}")

        var lastHeard = TimeSource.Monotonic.markNow()
        val pingsSent = mutableMapOf<UInt, TimeSource.Monotonic.ValueTimeMark>()
        var seq = 0u
        val ticker = Channel<Unit>(Channel.CONFLATED)
        val usbFound = Channel<Active>(1)
        val heartbeat = launch {
            while (isActive) {
                delay(config.pingEvery)
                ticker.send(Unit)
            }
        }
        val usbProbe =
            if (link.transport == Transport.Wifi) {
                launch {
                    while (isActive) {
                        delay(config.usbProbeEvery)
                        tryUsb()?.let {
                            usbFound.send(it)
                            return@launch
                        }
                    }
                }
            } else {
                null
            }

        try {
            var end: End? = null
            while (end == null) {
                end =
                    select {
                        link.session.events.onReceiveCatching { result ->
                            when (val event = result.getOrNull()) {
                                null, is SessionEvent.Closed -> End.Lost
                                SessionEvent.Opened -> null
                                is SessionEvent.Received -> {
                                    lastHeard = TimeSource.Monotonic.markNow()
                                    handle(link, event.message, pingsSent, paired)
                                }
                            }
                        }
                        ticker.onReceive {
                            if (lastHeard.elapsedNow() > config.deadAfter) {
                                log.info("no answer for ${config.deadAfter}: link dead")
                                End.Lost
                            } else {
                                seq++
                                pingsSent[seq] = TimeSource.Monotonic.markNow()
                                if (pingsSent.size > MAX_PENDING_PINGS) pingsSent.remove(pingsSent.keys.first())
                                link.session.send(Message.Ping(Heartbeat(seq)))
                                null
                            }
                        }
                        usbFound.onReceive { usb ->
                            log.info("USB available: switching from Wi-Fi")
                            End.SwitchedTo(usb)
                        }
                    }
            }
            // The PC closes the Wi-Fi session as soon as our USB probe is
            // authenticated, possibly before the probe hands its session over.
            if (end == End.Superseded && usbProbe != null) {
                withTimeoutOrNull(config.handshakeTimeout) { usbFound.receive() }?.let { End.SwitchedTo(it) }
                    ?: End.Lost
            } else {
                end
            }
        } finally {
            heartbeat.cancel()
            usbProbe?.cancel()
            if (active === link) active = null
            // Make-before-break: the new session is already up when this one closes.
            link.session.close()
            // A USB session found while this one was ending must not leak.
            usbFound.tryReceive().getOrNull()?.session?.cancel()
        }
    }

    /** Handles a message of an authenticated session; returns non-null to end it. */
    private fun handle(
        link: Active,
        message: Message,
        pingsSent: MutableMap<UInt, TimeSource.Monotonic.ValueTimeMark>,
        paired: Boolean
    ): End? {
        when (message) {
            is Message.LayoutSync -> _layout.value = message.payload
            is Message.ButtonUpdate -> _layout.value?.let { _layout.value = it.withButton(message.payload) }
            is Message.Navigate -> _navigations.tryEmit(message.payload)
            is Message.Ping -> link.session.send(Message.Pong(message.payload))
            is Message.Pong ->
                pingsSent.remove(message.payload.seq)?.let { sent ->
                    val latency = sent.elapsedNow().inWholeMilliseconds.toInt()
                    _state.value = LinkState.Connected(link.transport, link.welcome.serverName, paired, latency)
                }
            is Message.Error -> {
                val error = message.payload
                log.warning("PC error ${error.code}: ${error.message}")
                return when (error.code) {
                    ErrorCode.ActionFailed -> {
                        _actionFailures.tryEmit(error.message)
                        null
                    }
                    ErrorCode.Revoked -> End.Revoked
                    ErrorCode.UnsupportedVersion -> End.Incompatible
                    ErrorCode.Superseded -> End.Superseded
                    ErrorCode.Unauthorized -> End.Lost
                    ErrorCode.BadRequest, ErrorCode.UnknownButton -> null
                }
            }
            is Message.Hello,
            is Message.Welcome,
            is Message.ButtonPress,
            is Message.ButtonRelease,
            is Message.PairRequest,
            is Message.PairSuccess,
            is Message.PairFailure -> Unit
        }
        return null
    }

    // ─── Pairing ────────────────────────────────────────────────────────

    private suspend fun pairWith(endpoints: List<Endpoint>, expectedServerId: String?) {
        while (pins.tryReceive().isSuccess) Unit // drop stale input
        for (endpoint in endpoints) {
            // First contact: any key, captured to show its fingerprint.
            val trust = SpkiPinTrustManager(pin = null)
            val session = Session.open(http.pinnedTo(trust), "wss://${endpoint.display}/")
            val welcome = handshake(session, token = null) ?: continue
            val spki = trust.acceptedSpki
            if (spki == null || (expectedServerId != null && welcome.serverId != expectedServerId)) {
                session.cancel()
                continue
            }
            val paired = runPairing(session, welcome, spki, endpoint)
            if (paired != null) {
                auto(initial = paired)
            } else {
                _pairingProblems.tryEmit(PairingProblem.Lost)
                auto(initial = null)
            }
            return
        }
        _pairingProblems.tryEmit(PairingProblem.Unreachable)
        auto(initial = null)
    }

    /** PIN entry on an open, unauthenticated session. Returns the session once paired. */
    private suspend fun runPairing(
        session: Session,
        welcome: WelcomePayload,
        spki: ByteArray,
        endpoint: Endpoint
    ): Active? {
        var state = LinkState.Pairing(welcome.serverName, Fingerprint.short(spki))
        _state.value = state
        try {
            while (true) {
                val step =
                    select {
                        pins.onReceive { PairingStep.Pin(it) }
                        session.events.onReceiveCatching { PairingStep.Event(it.getOrNull()) }
                    }
                when (step) {
                    is PairingStep.Pin -> {
                        session.send(Message.PairRequest(PairRequestPayload(step.pin)))
                        state = state.copy(submitting = true)
                    }
                    is PairingStep.Event ->
                        when (val event = step.event) {
                            null, is SessionEvent.Closed -> return null
                            SessionEvent.Opened -> Unit
                            is SessionEvent.Received ->
                                when (val message = event.message) {
                                    is Message.PairSuccess -> {
                                        pairing.save(
                                            PairedPc(
                                                serverId = welcome.serverId,
                                                serverName = welcome.serverName,
                                                token = message.payload.token,
                                                spkiSha256 = Base64.getEncoder().encodeToString(
                                                    Fingerprint.spkiSha256(spki)
                                                ),
                                                lastEndpoint = endpoint
                                            )
                                        )
                                        log.info("paired with ${welcome.serverName}")
                                        return Active(
                                            session,
                                            Transport.Wifi,
                                            welcome.copy(auth = AuthStatus.Authenticated)
                                        )
                                    }
                                    is Message.PairFailure ->
                                        state =
                                            state.copy(
                                                failure = message.payload.reason,
                                                attemptsLeft = message.payload.attemptsLeft.toInt(),
                                                submitting = false
                                            )
                                    else -> Unit
                                }
                        }
                }
                _state.value = state
            }
        } catch (e: CancellationException) {
            session.close()
            throw e
        }
    }

    private companion object {
        const val MAX_PENDING_PINGS = 8
        val log: Logger = Logger.getLogger(ConnectionManager::class.java.name)
    }
}
