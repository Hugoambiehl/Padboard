package io.github.hugoambiehl.padboard.protocol

import java.io.File
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test

/** Same vector as the Rust test: both sides must show the same code. */
class FingerprintTest {
    private val vector =
        Json
            .parseToJsonElement(
                File(System.getProperty("padboard.fixtures"), "vectors/fingerprint.json").readText()
            ).jsonObject

    private fun field(name: String) = vector.getValue(name).jsonPrimitive.content

    private fun hex(s: String) = ByteArray(s.length / 2) { s.substring(it * 2, it * 2 + 2).toInt(16).toByte() }

    @Test
    fun matchesTheSharedVector() {
        val spki = hex(field("spkiDerHex"))
        assertArrayEquals(hex(field("sha256Hex")), Fingerprint.spkiSha256(spki))
        assertEquals(field("shortFingerprint"), Fingerprint.short(spki))
    }

    @Test
    fun formatIsThreeGroupsOfFour() {
        val code = Fingerprint.short("padboard".toByteArray())
        assertEquals(14, code.length)
        assertEquals(listOf(4, 4, 4), code.split('-').map { it.length })
    }
}
