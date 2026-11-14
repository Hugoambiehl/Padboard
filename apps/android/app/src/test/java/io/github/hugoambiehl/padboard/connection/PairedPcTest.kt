package io.github.hugoambiehl.padboard.connection

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PairedPcTest {
    @Test
    fun toStringRedactsTheToken() {
        val pc = PairedPc(
            serverId = "pc-1",
            serverName = "Desktop-PC",
            token = "tok-SECRET",
            spkiSha256 = "AAAA",
            lastEndpoint = Endpoint("192.168.1.20", 47322)
        )
        val text = pc.toString()
        assertFalse(text, "SECRET" in text)
        assertTrue(text, "<redacted>" in text && "Desktop-PC" in text && "192.168.1.20" in text)
    }
}
