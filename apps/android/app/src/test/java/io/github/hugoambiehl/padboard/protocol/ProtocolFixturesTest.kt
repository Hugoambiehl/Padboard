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
        val error = (fixture("error") as Message.Error).payload
        assertEquals(ErrorCode.UnsupportedVersion, error.code)
        assertEquals(Message.ButtonPress(ButtonEvent("terminal")), fixture("button_press"))
        assertEquals(Message.Pong(Heartbeat(42u)), fixture("pong"))
    }

    @Test
    fun unknownFieldsAreIgnored() {
        val message = parseMessage("""{"type":"ping","payload":{"seq":1,"future":true},"extra":0}""")
        assertEquals(Message.Ping(Heartbeat(1u)), message)
    }
}
