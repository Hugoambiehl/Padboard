package io.github.hugoambiehl.padboard.connection

import io.github.hugoambiehl.padboard.protocol.ButtonEvent
import io.github.hugoambiehl.padboard.protocol.ButtonView
import io.github.hugoambiehl.padboard.protocol.Category
import io.github.hugoambiehl.padboard.protocol.ErrorCode
import io.github.hugoambiehl.padboard.protocol.ErrorPayload
import io.github.hugoambiehl.padboard.protocol.GridSize
import io.github.hugoambiehl.padboard.protocol.Layout
import io.github.hugoambiehl.padboard.protocol.Message
import io.github.hugoambiehl.padboard.protocol.PROTOCOL_VERSION
import io.github.hugoambiehl.padboard.protocol.Page
import io.github.hugoambiehl.padboard.protocol.ProfileInfo
import io.github.hugoambiehl.padboard.protocol.WelcomePayload
import io.github.hugoambiehl.padboard.protocol.parseMessage
import io.github.hugoambiehl.padboard.protocol.toJson
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import okhttp3.OkHttpClient
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test

/** Runs the client against a fake desktop (OkHttp MockWebServer). */
class PadboardClientTest {
    private val server = MockWebServer()
    private val okHttp = OkHttpClient()
    private val received = LinkedBlockingQueue<Message>()
    private lateinit var client: PadboardClient

    private val layout =
        Layout(
            profile = ProfileInfo("demo", "Démo"),
            grid = GridSize(5u, 3u),
            pages = listOf(Page("p1", listOf(ButtonView("copy", 0u, 0u, "Copier", null, Category.Blue))))
        )

    @Before
    fun setUp() {
        server.start()
        client = PadboardClient(okHttp, "Test tablet")
    }

    @After
    fun tearDown() {
        client.disconnect()
        server.close()
        okHttp.dispatcher.executorService.shutdown()
    }

    /** A fake desktop: records what it receives and answers the hello with [onHello]. */
    private fun desktop(onHello: (WebSocket) -> Unit) = server.enqueue(
        MockResponse
            .Builder()
            .webSocketUpgrade(
                object : WebSocketListener() {
                    override fun onMessage(webSocket: WebSocket, text: String) {
                        val message = parseMessage(text)
                        received.put(message)
                        if (message is Message.Hello) onHello(webSocket)
                    }

                    override fun onOpen(webSocket: WebSocket, response: Response) = Unit
                }
            ).build()
    )

    private fun address() = ServerAddress(server.hostName, server.port)

    private fun nextReceived(): Message = received.poll(5, TimeUnit.SECONDS) ?: error("nothing received")

    @Test
    fun handshakeThenLayoutThenPress() = runBlocking {
        desktop { ws ->
            ws.send(Message.Welcome(WelcomePayload(PROTOCOL_VERSION, "TEST-PC")).toJson())
            ws.send(Message.LayoutSync(layout).toJson())
        }
        client.connect(address())

        val connected = withTimeout(5_000) { client.state.first { it is ConnectionState.Connected } }
        assertEquals("TEST-PC", (connected as ConnectionState.Connected).serverName)
        assertEquals(layout, withTimeout(5_000) { client.layout.first { it != null } })

        val hello = nextReceived() as Message.Hello
        assertEquals(PROTOCOL_VERSION, hello.payload.protocolVersion)
        assertEquals("Test tablet", hello.payload.deviceName)

        client.press("copy")
        client.release("copy")
        assertEquals(Message.ButtonPress(ButtonEvent("copy")), nextReceived())
        assertEquals(Message.ButtonRelease(ButtonEvent("copy")), nextReceived())
    }

    @Test
    fun incompatibleVersionFails() = runBlocking {
        desktop { ws ->
            ws.send(Message.Error(ErrorPayload(ErrorCode.UnsupportedVersion, "nope")).toJson())
            ws.close(1000, null)
        }
        client.connect(address())
        val failed = withTimeout(5_000) { client.state.first { it is ConnectionState.Failed } }
        assertEquals(FailureReason.Incompatible, (failed as ConnectionState.Failed).reason)
    }

    @Test
    fun unreachableServerFails() = runBlocking {
        val port = server.port
        server.close()
        client.connect(ServerAddress("127.0.0.1", port))
        val failed = withTimeout(5_000) { client.state.first { it is ConnectionState.Failed } }
        assertEquals(FailureReason.Unreachable, (failed as ConnectionState.Failed).reason)
    }

    @Test
    fun lostConnectionIsReported() = runBlocking {
        var desktopSocket: WebSocket? = null
        desktop { ws ->
            desktopSocket = ws
            ws.send(Message.Welcome(WelcomePayload(PROTOCOL_VERSION, "TEST-PC")).toJson())
        }
        client.connect(address())
        withTimeout(5_000) { client.state.first { it is ConnectionState.Connected } }
        desktopSocket!!.close(1001, "going away")
        val failed = withTimeout(5_000) { client.state.first { it is ConnectionState.Failed } }
        assertEquals(FailureReason.Lost, (failed as ConnectionState.Failed).reason)
    }
}
