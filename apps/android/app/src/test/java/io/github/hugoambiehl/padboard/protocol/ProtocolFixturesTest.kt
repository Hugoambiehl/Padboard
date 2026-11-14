package io.github.hugoambiehl.padboard.protocol

import java.io.File
import kotlinx.serialization.descriptors.elementNames
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Cross-language contract: decodes the fixtures of crates/protocol/fixtures (the
 * same files the Rust tests use) and checks they re-encode to the same JSON.
 */
class ProtocolFixturesTest {
    private val dir =
        File(
            System.getProperty("padboard.fixtures")
                ?: error("system property padboard.fixtures is not set (see app/build.gradle.kts)")
        )

    private fun fixtures(): Map<String, String> = dir
        .listFiles { f -> f.extension == "json" }
        .orEmpty()
        .associate { it.nameWithoutExtension to it.readText() }

    private fun fixture(stem: String): Message = parseMessage(fixtures().getValue(stem))

    @Test
    fun everyMessageTypeHasAFixture() {
        // Serial names of the sealed Message variants, e.g. "button:press" -> "button_press".
        val variants =
            Message
                .serializer()
                .descriptor
                .getElementDescriptor(1)
                .elementNames
                .map { it.replace(':', '_') }
                .toSet()
        assertEquals(variants, fixtures().keys)
    }

    @Test
    fun everyFixtureRoundTrips() {
        val fixtures = fixtures()
        assertTrue("no fixture found in $dir", fixtures.isNotEmpty())
        for ((stem, text) in fixtures) {
            val message = parseMessage(text)
            val original = Json.parseToJsonElement(text)
            val reencoded: JsonElement = Json.parseToJsonElement(message.toJson())
            assertEquals("$stem.json does not round-trip", original, reencoded)
        }
    }

    @Test
    fun layoutFixtureDecodesToTheExpectedValues() {
        val layout = (fixture("layout_sync") as Message.LayoutSync).payload
        assertEquals("Streaming", layout.profile.name)
        assertEquals(5u, layout.grid.cols)
        assertEquals(3u, layout.grid.rows)
        assertEquals(2, layout.pages.size)
        val terminal = layout.pages[0].buttons.single { it.id == "terminal" }
        assertEquals(Category.Grey, terminal.category)
        assertNull(terminal.subtitle)
        assertEquals("Volume 64 %", layout.pages[0].buttons.single { it.id == "volume" }.subtitle)
    }

    @Test
    fun handshakeFixturesDecode() {
        val hello = (fixture("hello") as Message.Hello).payload
        assertEquals(PROTOCOL_VERSION, hello.protocolVersion)
        assertEquals("3f2c8a9e-5b1d-4c7e-9a06-2d8f4b1e7c53", hello.deviceId)
        val welcome = (fixture("welcome") as Message.Welcome).payload
        assertEquals(AuthStatus.Authenticated, welcome.auth)
        val failure = (fixture("pair_failure") as Message.PairFailure).payload
        assertEquals(PairFailurePayload(PairFailureReason.InvalidPin, 3u), failure)
        val error = (fixture("error") as Message.Error).payload
        assertEquals(ErrorCode.UnsupportedVersion, error.code)
        assertEquals(Message.ButtonPress(ButtonEvent("terminal")), fixture("button_press"))
        assertEquals(Message.Pong(Heartbeat(42u)), fixture("pong"))
    }

    /** The frozen core of every protocol version (fixtures/core) still decodes, see the Rust crate. */
    @Test
    fun theFrozenCoreOfEveryVersionStillDecodes() {
        val core = File(dir, "core").listFiles { f -> f.extension == "json" }.orEmpty()
        assertTrue("no core fixture in $dir/core", core.isNotEmpty())
        for (file in core) {
            val message = parseMessage(file.readText())
            assertTrue(
                "core/${file.name} is not a core message",
                message is Message.Hello ||
                    message is Message.Welcome ||
                    message is Message.UpdateOffer ||
                    message is Message.UpdateRequest ||
                    message is Message.UpdateResult
            )
        }
        // Protocol 4 had no mode: read as a full session by the caller.
        val oldWelcome = parseMessage(File(dir, "core/welcome-v4.json").readText()) as Message.Welcome
        assertNull(oldWelcome.payload.mode)
        val offer = (fixture("update_offer") as Message.UpdateOffer).payload
        assertEquals(20099u, offer.versionCode)
        assertEquals(64, offer.sha256.length)
    }

    @Test
    fun unknownFieldsAreIgnored() {
        val message = parseMessage("""{"type":"ping","payload":{"seq":1,"future":true},"extra":0}""")
        assertEquals(Message.Ping(Heartbeat(1u)), message)
    }
}
