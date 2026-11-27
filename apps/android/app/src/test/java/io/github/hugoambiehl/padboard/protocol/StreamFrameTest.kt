package io.github.hugoambiehl.padboard.protocol

import java.io.File
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The stream connection's messages against the vectors shared with Rust. */
class StreamFrameTest {
    private val vectors: JsonObject =
        Json
            .parseToJsonElement(
                File(
                    System.getProperty("padboard.fixtures")
                        ?: error("system property padboard.fixtures is not set (see app/build.gradle.kts)"),
                    "vectors/stream.json"
                ).readText()
            ).jsonObject

    private fun hex(s: String): ByteArray = ByteArray(s.length / 2) {
        s.substring(2 * it, 2 * it + 2).toInt(16).toByte()
    }

    private fun JsonObject.text(key: String): String = getValue(key).jsonPrimitive.content

    @Test
    fun framesDecodeToTheirMeaningAndEncodeBack() {
        for (element in vectors.getValue("frames").jsonArray) {
            val case = element.jsonObject
            val bytes = hex(case.text("hex"))
            val decoded = StreamFrame.decode(bytes)
            assertTrue("$case", decoded is StreamFrame.Decoded.Frame)
            val frame = (decoded as StreamFrame.Decoded.Frame).frame
            when (case.text("kind")) {
                "video" -> {
                    val video = frame as StreamFrame.Video
                    assertEquals(case.getValue("keyFrame").jsonPrimitive.boolean, video.keyFrame)
                    assertEquals(case.text("captureUs").toULong(), video.captureUs)
                    assertArrayEquals(
                        hex(case.text("dataHex")),
                        video.data.copyOfRange(video.offset, video.offset + video.length)
                    )
                }
                "ack" ->
                    assertEquals(
                        StreamFrame.Ack(
                            case.text("captureUs").toULong(),
                            case.getValue("decodeUs").jsonPrimitive.int.toUInt()
                        ),
                        frame
                    )
                "keyFrameRequest" -> assertEquals(StreamFrame.KeyFrameRequest, frame)
                else -> error("unknown kind in $case")
            }
            val encoded = frame.encode()
            assertArrayEquals("$case", bytes.copyOf(encoded.size), encoded)
        }
    }

    @Test
    fun badMessagesAreRefused() {
        for (element in vectors.getValue("refused").jsonArray) {
            val case = element.jsonObject
            val expected =
                when (case.text("error")) {
                    "empty" -> StreamFrame.Error.Empty
                    "unknownKind" -> StreamFrame.Error.UnknownKind
                    "truncated" -> StreamFrame.Error.Truncated
                    else -> error("unknown error in $case")
                }
            assertEquals("$case", StreamFrame.Decoded.Refused(expected), StreamFrame.decode(hex(case.text("hex"))))
        }
    }

    @Test
    fun aVideoFrameIsReadInPlace() {
        val bytes = StreamFrame.Video(true, 5uL, byteArrayOf(9, 8, 7), 1, 2).encode()
        val video = (StreamFrame.decode(bytes) as StreamFrame.Decoded.Frame).frame as StreamFrame.Video
        assertEquals(10, video.offset)
        assertEquals(2, video.length)
        assertArrayEquals(byteArrayOf(8, 7), video.data.copyOfRange(10, 12))
    }
}
