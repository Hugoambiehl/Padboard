package io.github.hugoambiehl.padboard.connection

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ServerAddressTest {
    private fun valid(host: String, port: String = "47321") =
        (ServerAddress.parse(host, port) as ServerAddress.ParseResult.Valid).address

    @Test
    fun acceptsIpv4AndHostnames() {
        assertEquals(ServerAddress("192.168.1.20", 47321), valid("192.168.1.20"))
        assertEquals(ServerAddress("pc-hugo.local", 47321), valid(" pc-hugo.local "))
        assertEquals(ServerAddress("127.0.0.1", 50000), valid("127.0.0.1", "50000"))
    }

    @Test
    fun rejectsInvalidHosts() {
        for (host in listOf("", "256.1.1.1", "192.168.1", "1.2.3.4.5", "pc hugo", "-pc", "http://pc")) {
            assertEquals(host, ServerAddress.ParseResult.InvalidHost, ServerAddress.parse(host, "47321"))
        }
    }

    @Test
    fun rejectsInvalidPorts() {
        for (port in listOf("", "0", "65536", "abc", "-1")) {
            assertEquals(port, ServerAddress.ParseResult.InvalidPort, ServerAddress.parse("10.0.0.2", port))
        }
    }

    @Test
    fun loopbackMeansUsb() {
        assertTrue(valid("127.0.0.1").isUsb)
        assertTrue(valid("localhost").isUsb)
        assertFalse(valid("192.168.1.20").isUsb)
        assertEquals("ws://192.168.1.20:47321/", valid("192.168.1.20").webSocketUrl)
    }
}
