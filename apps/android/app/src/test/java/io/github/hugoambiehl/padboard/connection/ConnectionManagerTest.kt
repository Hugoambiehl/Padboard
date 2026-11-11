package io.github.hugoambiehl.padboard.connection

import io.github.hugoambiehl.padboard.protocol.AuthStatus
import io.github.hugoambiehl.padboard.protocol.ButtonView
import io.github.hugoambiehl.padboard.protocol.Category
import io.github.hugoambiehl.padboard.protocol.ErrorCode
import io.github.hugoambiehl.padboard.protocol.ErrorPayload
import io.github.hugoambiehl.padboard.protocol.Fingerprint
import io.github.hugoambiehl.padboard.protocol.GridSize
import io.github.hugoambiehl.padboard.protocol.Layout
import io.github.hugoambiehl.padboard.protocol.Message
import io.github.hugoambiehl.padboard.protocol.NavigatePayload
import io.github.hugoambiehl.padboard.protocol.NavigateTarget
import io.github.hugoambiehl.padboard.protocol.PROTOCOL_VERSION
import io.github.hugoambiehl.padboard.protocol.Page
import io.github.hugoambiehl.padboard.protocol.PairFailurePayload
import io.github.hugoambiehl.padboard.protocol.PairFailureReason
import io.github.hugoambiehl.padboard.protocol.PairSuccessPayload
import io.github.hugoambiehl.padboard.protocol.ProfileInfo
import io.github.hugoambiehl.padboard.protocol.WelcomePayload
import io.github.hugoambiehl.padboard.protocol.parseMessage
import io.github.hugoambiehl.padboard.protocol.toJson
import java.util.Base64
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.atomic.AtomicInteger
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import mockwebserver3.Dispatcher
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import mockwebserver3.RecordedRequest
import okhttp3.OkHttpClient
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okhttp3.tls.HandshakeCertificates
import okhttp3.tls.HeldCertificate
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The connection manager against a fake PC that has both transports: a plain
 * USB port and a TLS Wi-Fi port with its own key, like the desktop app.
 */
class ConnectionManagerTest {
    private val pc = FakePc("pc-1", "PC-TEST")
    private val other = FakePc("pc-2", "OTHER-PC")
    private val scope = CoroutineScope(SupervisorJob())
    private val pairing = MemoryPairing()
    private val directory = FakeDirectory()

    private val config =
        ConnectionManager.Config(
            usbEndpoint = Endpoint("127.0.0.1", pc.usb.port),
            handshakeTimeout = 2.seconds,
            usbProbeEvery = 150.milliseconds,
            pingEvery = 200.milliseconds,
            deadAfter = 1.seconds,
            retryMin = 50.milliseconds,
            retryMax = 200.milliseconds
        )
    private val manager = ConnectionManager(scope, OkHttpClient(), pairing, directory, "Test tablet", config)

    @After
    fun tearDown() {
        scope.cancel()
        pc.close()
        other.close()
    }

    private fun <T> await(flow: StateFlow<T>, what: String, predicate: (T) -> Boolean): T = runBlocking {
        try {
            withTimeout(8.seconds) { flow.first(predicate) }
        } catch (e: kotlinx.coroutines.TimeoutCancellationException) {
            throw AssertionError("timed out waiting for $what (last: ${flow.value})", e)
        }
    }

    private fun connected(transport: Transport) = await(manager.state, "connected over $transport") {
        it is LinkState.Connected && it.transport == transport
    } as LinkState.Connected

    private fun paired(withPc: FakePc = pc) {
        val token = "token-of-tablet"
        withPc.tokens[DEVICE_ID] = token
        runBlocking {
            pairing.save(
                PairedPc(
                    serverId = withPc.serverId,
                    serverName = withPc.name,
                    token = token,
                    spkiSha256 = Base64.getEncoder().encodeToString(Fingerprint.spkiSha256(withPc.spki)),
                    lastEndpoint = withPc.wifiEndpoint
                )
            )
        }
    }

    @Test
    fun usbIsUsedFirstWithoutPairingAndWithoutToken() {
        pc.usbAvailable = true
        paired()
        manager.start(DEVICE_ID)
        val state = connected(Transport.Usb)
        assertEquals("PC-TEST", state.serverName)
        assertTrue(state.paired)
        assertNotNull(await(manager.layout, "layout") { it != null })
        // The token never travels over plain USB.
        assertTrue(pc.hellos.all { it.second == null || it.first == Transport.Wifi })
    }

    @Test
    fun buttonUpdatesAndNavigationReachTheUi() {
        pc.usbAvailable = true
        paired()
        manager.start(DEVICE_ID)
        connected(Transport.Usb)
        await(manager.layout, "layout") { it != null }
        val navigation = CoroutineScope(SupervisorJob()).async {
            withTimeout(5.seconds) { manager.navigations.first() }
        }
        // The collector must be subscribed before the PC sends.
        Thread.sleep(100)

        pc.sendToTablets(Message.ButtonUpdate(button("copy", "Copié").copy(active = true, badge = "OK")))
        val updated = await(manager.layout, "updated button") { it?.pages?.first()?.buttons?.first()?.active == true }
        assertEquals("Copié", updated!!.pages.first().buttons.first().label)
        assertEquals("OK", updated.pages.first().buttons.first().badge)

        pc.sendToTablets(Message.Navigate(NavigatePayload(NavigateTarget.Next)))
        assertEquals(NavigateTarget.Next, runBlocking { navigation.await() }.to)
    }

    @Test
    fun withoutUsbThePairedPcIsReachedOverWifi() {
        paired()
        manager.start(DEVICE_ID)
        connected(Transport.Wifi)
        manager.press("copy")
        await(pc.pressesFlow, "press") { "copy" in it }
    }

    @Test
    fun unpairedWithoutUsbShowsSetup() {
        manager.start(DEVICE_ID)
        await(manager.state, "unpaired") { it == LinkState.Unpaired }
    }

    @Test
    fun pluggingUsbSwitchesBeforeWifiCloses() {
        paired()
        manager.start(DEVICE_ID)
        connected(Transport.Wifi)
        pc.usbAvailable = true
        connected(Transport.Usb)
        // The PC saw the Wi-Fi session superseded by USB: no gap.
        assertTrue(pc.superseded.get() >= 1)
        manager.press("terminal")
        await(pc.pressesFlow, "press over USB") { "terminal" in it }
    }

    @Test
    fun unpluggingUsbWithoutPairingKeepsLookingForThatPc() {
        pc.usbAvailable = true
        manager.start(DEVICE_ID)
        connected(Transport.Usb)
        pc.usbAvailable = false
        pc.dropUsb()
        val searching = await(manager.state, "searching over USB") { it is LinkState.Searching } as LinkState.Searching
        assertEquals(pc.name, searching.serverName)
        assertFalse("not paired: only the cable can bring it back", searching.wifi)
        assertNotNull("the grid stays", manager.layout.value)
        pc.usbAvailable = true
        connected(Transport.Usb)
    }

    @Test
    fun unpluggingUsbFallsBackToWifi() {
        pc.usbAvailable = true
        paired()
        manager.start(DEVICE_ID)
        connected(Transport.Usb)
        pc.usbAvailable = false
        pc.dropUsb()
        connected(Transport.Wifi)
    }

    @Test
    fun addressesAreTriedInOrderUntilTheRightPcAnswers() {
        paired()
        runBlocking { pairing.save(pairing.load()!!.copy(lastEndpoint = null)) }
        // mDNS lists another PC's address first: wrong serverId and key, skipped.
        directory.pcs.value =
            listOf(DiscoveredPc(pc.serverId, pc.name, 2, listOf("127.0.0.1"), other.wifi.port)) +
            listOf(DiscoveredPc(pc.serverId, pc.name, 2, listOf("127.0.0.1"), pc.wifi.port))
        manager.start(DEVICE_ID)
        connected(Transport.Wifi)
        assertEquals(pc.wifiEndpoint, runBlocking { pairing.load()?.lastEndpoint })
        assertTrue("the other PC must never see the token", other.hellos.none { it.second != null })
    }

    @Test
    fun aPcWithAnotherKeyIsRefused() {
        paired()
        // Same serverId, but a different key: a PC impersonating the paired one.
        runBlocking { pairing.save(pairing.load()!!.copy(lastEndpoint = other.wifiEndpoint)) }
        manager.start(DEVICE_ID)
        await(manager.state, "searching") { it is LinkState.Searching }
        assertTrue("TLS must fail before hello", other.hellos.isEmpty())
    }

    @Test
    fun revocationForgetsThePairing() {
        paired()
        manager.start(DEVICE_ID)
        connected(Transport.Wifi)
        pc.revokeAll()
        await(manager.state, "unpaired") { it == LinkState.Unpaired }
        assertNull(runBlocking { pairing.load() })
    }

    @Test
    fun pairingStoresTokenKeyAndServer() {
        pc.pin = "123456"
        manager.start(DEVICE_ID)
        await(manager.state, "unpaired") { it == LinkState.Unpaired }
        manager.pair(DiscoveredPc(pc.serverId, pc.name, 2, listOf("127.0.0.1"), pc.wifi.port))
        val pinScreen = await(manager.state, "PIN screen") { it is LinkState.Pairing } as LinkState.Pairing
        assertEquals(Fingerprint.short(pc.spki), pinScreen.fingerprint)

        manager.submitPin("000000")
        val refused = await(manager.state, "refused") {
            it is LinkState.Pairing && it.failure != null
        } as LinkState.Pairing
        assertEquals(PairFailureReason.InvalidPin, refused.failure)
        assertEquals(4, refused.attemptsLeft)

        manager.submitPin("123456")
        assertTrue(connected(Transport.Wifi).paired)
        val stored = runBlocking { pairing.load() }!!
        assertEquals(pc.serverId, stored.serverId)
        assertEquals(pc.tokens[DEVICE_ID], stored.token)
        assertTrue(Fingerprint.spkiSha256(pc.spki).contentEquals(stored.pin))
    }

    @Test
    fun aDeadLinkIsDetectedByTheHeartbeat() {
        paired()
        manager.start(DEVICE_ID)
        val first = connected(Transport.Wifi)
        assertFalse(first.latencyMs != null && first.latencyMs!! < 0)
        await(manager.state, "latency") { it is LinkState.Connected && it.latencyMs != null }
        pc.mute = true // stops answering, socket stays open
        await(manager.state, "lost") { it !is LinkState.Connected }
    }

    private companion object {
        const val DEVICE_ID = "tablet-1"
    }
}

// ─── Fakes ──────────────────────────────────────────────────────────────

private class MemoryPairing : PairingRepository {
    @Volatile private var pc: PairedPc? = null

    override suspend fun load() = pc

    override suspend fun save(pc: PairedPc) {
        this.pc = pc
    }

    override suspend fun clear() {
        pc = null
    }

    override suspend fun updateEndpoint(endpoint: Endpoint) {
        pc = pc?.copy(lastEndpoint = endpoint)
    }
}

private class FakeDirectory : PcDirectory {
    override val pcs = MutableStateFlow<List<DiscoveredPc>>(emptyList())
}

/** A desktop app: USB port (plain) and Wi-Fi port (TLS, own key). */
private class FakePc(val serverId: String, val name: String) {
    private val certificate = HeldCertificate.Builder().build()
    val spki: ByteArray = certificate.certificate.publicKey.encoded
    val tokens = ConcurrentHashMap<String, String>()
    val hellos = CopyOnWriteArrayList<Pair<Transport, String?>>()
    val pressesFlow = MutableStateFlow<List<String>>(emptyList())
    val superseded = AtomicInteger()

    @Volatile var usbAvailable = false

    @Volatile var pin: String? = null

    @Volatile var mute = false

    private val sessions = ConcurrentHashMap<String, WebSocket>()
    private val usbSockets = CopyOnWriteArrayList<WebSocket>()

    val usb = MockWebServer().apply { dispatcher = dispatcherFor(Transport.Usb) }
    val wifi =
        MockWebServer().apply {
            dispatcher = dispatcherFor(Transport.Wifi)
            useHttps(HandshakeCertificates.Builder().heldCertificate(certificate).build().sslSocketFactory())
        }

    init {
        usb.start()
        wifi.start()
    }

    val wifiEndpoint get() = Endpoint("127.0.0.1", wifi.port)

    fun close() {
        usb.close()
        wifi.close()
    }

    /** Cable unplugged: the forwarded connections end. */
    fun dropUsb() = usbSockets.forEach { it.close(1001, "unplugged") }

    /** Sends to every authenticated session (state changes, navigation). */
    fun sendToTablets(message: Message) = sessions.values.forEach { send(it, message) }

    fun revokeAll() {
        tokens.clear()
        sessions.values.forEach { send(it, Message.Error(ErrorPayload(ErrorCode.Revoked, "revoked"))) }
    }

    private fun dispatcherFor(transport: Transport) = object : Dispatcher() {
        override fun dispatch(request: RecordedRequest): MockResponse = if (transport == Transport.Usb &&
            !usbAvailable
        ) {
            MockResponse.Builder().code(503).build()
        } else {
            MockResponse.Builder().webSocketUpgrade(Listener(transport)).build()
        }
    }

    private fun send(ws: WebSocket, message: Message) = ws.send(message.toJson())

    private val layout =
        Layout(
            ProfileInfo("demo", "Démo"),
            GridSize(5u, 3u),
            listOf(Page("p1", listOf(button("copy", "Copier")))),
            folders = emptyList()
        )

    private inner class Listener(private val transport: Transport) : WebSocketListener() {
        private var deviceId: String? = null
        private var authenticated = false

        override fun onOpen(webSocket: WebSocket, response: Response) {
            if (transport == Transport.Usb) usbSockets += webSocket
        }

        override fun onMessage(webSocket: WebSocket, text: String) {
            if (mute) return
            when (val message = parseMessage(text)) {
                is Message.Hello -> {
                    val hello = message.payload
                    deviceId = hello.deviceId
                    hellos += transport to hello.token
                    val auth =
                        when {
                            transport == Transport.Usb -> AuthStatus.Authenticated
                            hello.token == null -> AuthStatus.PairingRequired
                            tokens[hello.deviceId] == hello.token -> AuthStatus.Authenticated
                            else -> AuthStatus.TokenRejected
                        }
                    send(webSocket, Message.Welcome(WelcomePayload(PROTOCOL_VERSION, name, serverId, auth)))
                    if (auth == AuthStatus.Authenticated) authenticate(webSocket)
                }
                is Message.PairRequest -> {
                    if (message.payload.pin == pin) {
                        val token = "token-${System.nanoTime()}"
                        tokens[deviceId!!] = token
                        send(webSocket, Message.PairSuccess(PairSuccessPayload(token)))
                        authenticate(webSocket)
                    } else {
                        send(webSocket, Message.PairFailure(PairFailurePayload(PairFailureReason.InvalidPin, 4u)))
                    }
                }
                is Message.Ping -> if (authenticated) send(webSocket, Message.Pong(message.payload))
                is Message.ButtonPress -> pressesFlow.value = pressesFlow.value + message.payload.buttonId
                else -> Unit
            }
        }

        private fun authenticate(webSocket: WebSocket) {
            authenticated = true
            send(webSocket, Message.LayoutSync(layout))
            sessions.put(deviceId!!, webSocket)?.let { previous ->
                superseded.incrementAndGet()
                send(previous, Message.Error(ErrorPayload(ErrorCode.Superseded, "superseded")))
                previous.close(1000, null)
            }
        }

        override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
            webSocket.close(1000, null)
            sessions.remove(deviceId ?: return, webSocket)
        }

        override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
            sessions.remove(deviceId ?: return, webSocket)
        }
    }
}

private fun button(id: String, label: String) =
    ButtonView(id, 0u, 0u, label, category = Category.Blue, active = false, unavailable = false)
