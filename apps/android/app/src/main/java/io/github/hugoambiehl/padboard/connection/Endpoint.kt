package io.github.hugoambiehl.padboard.connection

/** Where the PC listens. User input is validated with [Endpoint.parse]. */
data class Endpoint(val host: String, val port: Int) {
    /** Host and port, e.g. `192.168.1.20:47322` (IPv6 in brackets), as in URLs. */
    val display: String get() = if (':' in host) "[$host]:$port" else "$host:$port"

    sealed interface ParseResult {
        data class Valid(val endpoint: Endpoint) : ParseResult

        data object InvalidHost : ParseResult

        data object InvalidPort : ParseResult
    }

    companion object {
        /** USB: plain WebSocket, reached at 127.0.0.1 through `adb reverse` (`USB_PORT` in crates/protocol). */
        const val USB_PORT = 47321

        /** Wi-Fi: WebSocket over TLS (`WIFI_PORT` in crates/protocol). */
        const val WIFI_PORT = 47322
        const val LOOPBACK = "127.0.0.1"

        private val IPV4 = Regex("""^((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.){3}(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)$""")
        private val HOSTNAME =
            Regex(
                """^(?=.{1,253}$)([a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)(\.[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$"""
            )

        fun parse(host: String, port: String): ParseResult {
            val h = host.trim()
            val looksNumeric = h.all { it.isDigit() || it == '.' }
            val hostOk = if (looksNumeric) IPV4.matches(h) else HOSTNAME.matches(h)
            if (!hostOk) return ParseResult.InvalidHost
            val p = port.trim().toIntOrNull()
            if (p == null || p !in 1..65535) return ParseResult.InvalidPort
            return ParseResult.Valid(Endpoint(h, p))
        }
    }
}
