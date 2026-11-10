package io.github.hugoambiehl.padboard.connection

import org.junit.Assert.assertEquals
import org.junit.Test

class EndpointTest {
    private fun valid(host: String, port: String = "47321") =
        (Endpoint.parse(host, port) as Endpoint.ParseResult.Valid).endpoint

    @Test
    fun acceptsIpv4AndHostnames() {
        assertEquals(Endpoint("192.168.1.20", 47321), valid("192.168.1.20"))
        assertEquals(Endpoint("pc-hugo.local", 47321), valid(" pc-hugo.local "))
        assertEquals(Endpoint("127.0.0.1", 50000), valid("127.0.0.1", "50000"))
    }

    @Test
    fun rejectsInvalidHosts() {
        for (host in listOf("", "256.1.1.1", "192.168.1", "1.2.3.4.5", "pc hugo", "-pc", "http://pc")) {
            assertEquals(host, Endpoint.ParseResult.InvalidHost, Endpoint.parse(host, "47321"))
        }
    }

    @Test
    fun rejectsInvalidPorts() {
        for (port in listOf("", "0", "65536", "abc", "-1")) {
            assertEquals(port, Endpoint.ParseResult.InvalidPort, Endpoint.parse("10.0.0.2", port))
        }
    }

    @Test
    fun displaysHostAndPort() {
        assertEquals("192.168.1.20:47322", valid("192.168.1.20", "47322").display)
        assertEquals("[fd71::1]:47322", Endpoint("fd71::1", 47322).display)
    }
}
