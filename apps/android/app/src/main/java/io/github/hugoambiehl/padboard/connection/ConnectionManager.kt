package io.github.hugoambiehl.padboard.connection

import io.github.hugoambiehl.padboard.protocol.AuthStatus
import io.github.hugoambiehl.padboard.protocol.ButtonEvent
import io.github.hugoambiehl.padboard.protocol.ErrorCode
import io.github.hugoambiehl.padboard.protocol.Fingerprint
import io.github.hugoambiehl.padboard.protocol.Heartbeat
import io.github.hugoambiehl.padboard.protocol.HelloPayload
import io.github.hugoambiehl.padboard.protocol.Layout
import io.github.hugoambiehl.padboard.protocol.MAX_UPDATE_SIZE
import io.github.hugoambiehl.padboard.protocol.Message
import io.github.hugoambiehl.padboard.protocol.NavigatePayload
import io.github.hugoambiehl.padboard.protocol.PROTOCOL_VERSION
import io.github.hugoambiehl.padboard.protocol.PairFailureReason
import io.github.hugoambiehl.padboard.protocol.PairRequestPayload
import io.github.hugoambiehl.padboard.protocol.SessionMode
import io.github.hugoambiehl.padboard.protocol.SystemStatusPayload
import io.github.hugoambiehl.padboard.protocol.TabletOrder
import io.github.hugoambiehl.padboard.protocol.TabletSettingsPayload
import io.github.hugoambiehl.padboard.protocol.TabletStatusPayload
import io.github.hugoambiehl.padboard.protocol.UpdateOfferPayload
import io.github.hugoambiehl.padboard.protocol.UpdateOutcome
import io.github.hugoambiehl.padboard.protocol.UpdateRequestPayload
import io.github.hugoambiehl.padboard.protocol.UpdateResultPayload
import io.github.hugoambiehl.padboard.protocol.WelcomePayload
import io.github.hugoambiehl.padboard.protocol.withButton
import java.io.File
import java.io.IOException
import java.io.OutputStream
import java.security.MessageDigest
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
import kotlinx.coroutines.withContext
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
     * Looking for the PC: the paired one (USB or Wi-Fi), or the one just
     * lost when it was another PC reached over USB ([wifi] false: only the
     * cable can bring it back). The next attempt starts in [retryInMs]; [attempt] counts them so
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

    /**
     * Connected, but the PC speaks another protocol version: the session
     * only serves updates. [pcIsNewer]: the tablet is the one to update
     * (from the PC); otherwise Padboard on the PC is.
     */
    data class UpdateOnly(val transport: Transport, val serverName: String, val pcIsNewer: Boolean) : LinkState

    /** The PC speaks another protocol version and cannot update the tablet: update the PC. */
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
    private val config: Config = Config(),
    private val updater: AppUpdater = AppUpdater.None
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
        val retryMax: Duration = 5.seconds,
        /** An update whose next piece does not come for this long is abandoned. */
        val updateStall: Duration = 30.seconds
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

    private val _orders =
        MutableSharedFlow<TabletOrder>(extraBufferCapacity = 4, onBufferOverflow = BufferOverflow.DROP_OLDEST)

    /** Orders given from the PC (leave kiosk mode…). */
    val orders: SharedFlow<TabletOrder> = _orders.asSharedFlow()

    private val _settings = MutableStateFlow<TabletSettingsPayload?>(null)

    /** The tablet's settings, kept on the PC; the last ones received stay while disconnected. */
    val settings: StateFlow<TabletSettingsPayload?> = _settings.asStateFlow()

    private val _systemStatus = MutableStateFlow<SystemStatusPayload?>(null)

    /** PC states for the screensaver; null while disconnected (they would be stale). */
    val systemStatus: StateFlow<SystemStatusPayload?> = _systemStatus.asStateFlow()

    private val _update = MutableStateFlow<UpdateProgress?>(null)

    /** An update sent by the PC over the connection (kiosk mode only). */
    val update: StateFlow<UpdateProgress?> = _update.asStateFlow()

    /** The update being received (connection thread only). */
    private var download: Download? = null

    private class Download(val offer: UpdateOfferPayload, val file: File, val out: OutputStream) {
        val digest: MessageDigest = MessageDigest.getInstance("SHA-256")
        var received = 0L
        var lastPiece = TimeSource.Monotonic.markNow()
    }

    /** Kiosk mode, reported to the PC (`tablet:status`). */
    private var kiosk = false

    /** The session presses go to (read from the UI thread). */
    @Volatile private var active: Active? = null

    private var deviceId: String = ""

    /**
     * The PC of the last session in this run (any transport): the one the
     * disconnected card names. Over USB it may not be the paired PC.
     */
    private var lastServer: LastServer? = null

    private data class LastServer(val id: String, val name: String)
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
                lastServer = null
                auto(initial = null)
            }
        }
    }

    /** Kiosk mode turned on or off: tell the PC. */
    fun setKiosk(on: Boolean) {
        scope.launch {
            if (kiosk == on) return@launch
            kiosk = on
            active?.session?.send(Message.TabletStatus(TabletStatusPayload(kiosk)))
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
                            lastServer = null
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
            val last = lastServer
            attempt++
            _state.value =
                if (last != null && last.id != paired?.serverId) {
                    // Lost a PC reached over USB that is not the paired one
                    // (another PC on the cable): only the cable brings it back.
                    LinkState.Searching(last.name, wifi = false, retry.inWholeMilliseconds, attempt)
                } else if (paired != null) {
                    LinkState.Searching(paired.serverName, wifi = true, retry.inWholeMilliseconds, attempt)
                } else {
                    LinkState.Unpaired
                }
            withTimeoutOrNull(retry) { wakeUp.receive() }
            retry = (retry * 2).coerceAtMost(config.retryMax)
        }
    }

    private fun hello(token: String?) = Message.Hello(
        HelloPayload(
            PROTOCOL_VERSION,
            deviceName,
            deviceId,
            token,
            appVersion = updater.versionName,
            appVersionCode = updater.versionCode?.takeIf { it in 0..UInt.MAX_VALUE.toLong() }?.toUInt(),
            silentInstall = updater.canInstallSilently()
        )
    )

    /** Dismisses a finished (failed) update shown by the UI. */
    fun dismissUpdate() {
        scope.launch { if (_update.value is UpdateProgress.Failed) _update.value = null }
    }

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
                        // Nothing binary is expected before `welcome`.
                        is SessionEvent.Bytes -> return@withTimeoutOrNull null
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
        // Another protocol: only the frozen core (pings, updates), nothing else is sent.
        val full = link.welcome.mode != SessionMode.UpdateOnly
        lastServer = LastServer(link.welcome.serverId, link.welcome.serverName)
        if (full) {
            active = link
            _state.value = LinkState.Connected(link.transport, link.welcome.serverName, paired, latencyMs = null)
            log.info("connected to ${link.welcome.serverName} over ${link.transport}")
            link.session.send(Message.TabletStatus(TabletStatusPayload(kiosk)))
        } else {
            val pcIsNewer = link.welcome.protocolVersion > PROTOCOL_VERSION
            _state.value = LinkState.UpdateOnly(link.transport, link.welcome.serverName, pcIsNewer)
            log.info(
                "connected to ${link.welcome.serverName} for updates only (its protocol: ${link.welcome.protocolVersion})"
            )
        }
        // An install that failed after the app handed it over.
        val installFailures = launch {
            updater.failures.collect { outcome ->
                _update.value = UpdateProgress.Failed(_update.value?.version.orEmpty(), outcome)
                link.session.send(Message.UpdateResult(UpdateResultPayload(outcome)))
            }
        }

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
                                    handle(link, event.message, pingsSent, paired, full)
                                }
                                is SessionEvent.Bytes -> {
                                    lastHeard = TimeSource.Monotonic.markNow()
                                    piece(link, event.bytes)
                                    null
                                }
                            }
                        }
                        ticker.onReceive {
                            download?.let { d ->
                                if (d.lastPiece.elapsedNow() > config.updateStall) {
                                    log.warning("update stalled: abandoned")
                                    failUpdate(link, UpdateOutcome.TransferFailed)
                                }
                            }
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
            installFailures.cancel()
            abandonDownload()
            if (active === link) {
                active = null
                _systemStatus.value = null
            }
            // Make-before-break: the new session is already up when this one closes.
            link.session.close()
            // A USB session found while this one was ending must not leak.
            usbFound.tryReceive().getOrNull()?.session?.cancel()
        }
    }

    /** Handles a message of an authenticated session; returns non-null to end it. */
    private suspend fun handle(
        link: Active,
        message: Message,
        pingsSent: MutableMap<UInt, TimeSource.Monotonic.ValueTimeMark>,
        paired: Boolean,
        full: Boolean
    ): End? {
        if (!full &&
            message !is Message.Ping &&
            message !is Message.Pong &&
            message !is Message.Error &&
            message !is Message.UpdateOffer
        ) {
            log.warning("ignoring ${message::class.simpleName} in an update-only session")
            return null
        }
        when (message) {
            is Message.LayoutSync -> _layout.value = message.payload
            is Message.ButtonUpdate -> _layout.value?.let { _layout.value = it.withButton(message.payload) }
            is Message.Navigate -> _navigations.tryEmit(message.payload)
            is Message.TabletCommand -> _orders.tryEmit(message.payload.order)
            is Message.TabletSettings -> _settings.value = message.payload
            is Message.SystemStatus -> _systemStatus.value = message.payload
            is Message.Ping -> link.session.send(Message.Pong(message.payload))
            is Message.Pong ->
                pingsSent.remove(message.payload.seq)?.let { sent ->
                    val latency = sent.elapsedNow().inWholeMilliseconds.toInt()
                    if (full) {
                        _state.value =
                            LinkState.Connected(link.transport, link.welcome.serverName, paired, latency)
                    }
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
            is Message.PairFailure,
            is Message.TabletStatus,
            is Message.UpdateRequest,
            is Message.UpdateResult -> Unit
            is Message.UpdateOffer -> offered(link, message.payload)
        }
        return null
    }

    // ─── Updates from the PC ────────────────────────────────────────────

    /**
     * `update:offer`: accepted only in kiosk mode (silent install), for a
     * higher version; the APK is then pulled piece by piece.
     */
    private fun offered(link: Active, offer: UpdateOfferPayload) {
        fun refuse(outcome: UpdateOutcome) {
            log.info("update ${offer.version} refused: $outcome")
            link.session.send(Message.UpdateResult(UpdateResultPayload(outcome)))
        }
        if (download != null) return refuse(UpdateOutcome.Busy)
        if (!updater.canInstallSilently()) return refuse(UpdateOutcome.NotDeviceOwner)
        val own = updater.versionCode
        if (own == null || offer.versionCode.toLong() <= own) return refuse(UpdateOutcome.NotNewer)
        val size = offer.size.toLong()
        if (size == 0L || size > MAX_UPDATE_SIZE) return refuse(UpdateOutcome.TransferFailed)
        val file =
            try {
                updater.newApkFile()
            } catch (e: IOException) {
                log.warning("cannot create the update file: $e")
                return refuse(UpdateOutcome.TransferFailed)
            }
        log.info("receiving update ${offer.version} ($size bytes)")
        download = Download(offer, file, file.outputStream().buffered())
        _update.value = UpdateProgress.Receiving(offer.version, 0, size)
        link.session.send(Message.UpdateRequest(UpdateRequestPayload(0u)))
    }

    /** A binary frame: the next piece of the APK (4-byte big-endian offset, then bytes). */
    private suspend fun piece(link: Active, frame: ByteArray) {
        val d = download ?: return
        val size = d.offer.size.toLong()
        if (frame.size <= 4) return failUpdate(link, UpdateOutcome.TransferFailed)
        val offset = ((frame[0].toLong() and 0xff) shl 24) or ((frame[1].toLong() and 0xff) shl 16) or
            ((frame[2].toLong() and 0xff) shl 8) or (frame[3].toLong() and 0xff)
        val length = frame.size - 4
        if (offset != d.received || d.received + length > size) return failUpdate(link, UpdateOutcome.TransferFailed)
        try {
            d.out.write(frame, 4, length)
        } catch (e: IOException) {
            log.warning("cannot write the update: $e")
            return failUpdate(link, UpdateOutcome.TransferFailed)
        }
        d.digest.update(frame, 4, length)
        d.received += length
        d.lastPiece = TimeSource.Monotonic.markNow()
        _update.value = UpdateProgress.Receiving(d.offer.version, d.received, size)
        if (d.received < size) {
            link.session.send(Message.UpdateRequest(UpdateRequestPayload(d.received.toUInt())))
        } else {
            received(link, d)
        }
    }

    /** All pieces in: same bytes as announced, then the app's checks, then the installer. */
    private suspend fun received(link: Active, d: Download) {
        download = null
        try {
            d.out.close()
        } catch (e: IOException) {
            log.warning("cannot write the update: $e")
            return finishFailed(link, d, UpdateOutcome.TransferFailed)
        }
        val hash = d.digest.digest().joinToString("") { "%02x".format(it) }
        if (hash != d.offer.sha256.lowercase()) return finishFailed(link, d, UpdateOutcome.BadHash)
        val refused = withContext(Dispatchers.IO) { updater.check(d.file, d.offer) }
        if (refused != null) return finishFailed(link, d, refused)
        log.info("update ${d.offer.version} checked: installing")
        _update.value = UpdateProgress.Installing(d.offer.version)
        link.session.send(Message.UpdateResult(UpdateResultPayload(UpdateOutcome.Installing)))
        withContext(Dispatchers.IO) { updater.install(d.file) }
    }

    private fun failUpdate(link: Active, outcome: UpdateOutcome) {
        val d = download ?: return
        download = null
        runCatching { d.out.close() }
        finishFailed(link, d, outcome)
    }

    private fun finishFailed(link: Active, d: Download, outcome: UpdateOutcome) {
        log.warning("update ${d.offer.version} failed: $outcome")
        d.file.delete()
        // The tablet's state first: the PC may act on the result at once.
        _update.value = UpdateProgress.Failed(d.offer.version, outcome)
        link.session.send(Message.UpdateResult(UpdateResultPayload(outcome)))
    }

    /** The session ended during a transfer: nothing is kept. */
    private fun abandonDownload() {
        val d = download ?: return
        download = null
        runCatching { d.out.close() }
        d.file.delete()
        _update.value = null
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
                            SessionEvent.Opened, is SessionEvent.Bytes -> Unit
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
