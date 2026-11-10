package io.github.hugoambiehl.padboard.connection

/** Where the desktop app listens. Validated with [ServerAddress.parse]. */
data class ServerAddress(val host: String, val port: Int) {
    /** Through `adb reverse`, a USB-connected desktop is reached on the loopback address. */
    val isUsb: Boolean get() = host == LOOPBACK || host == "localhost"

    val webSocketUrl: String get() = "ws://$host:$port/"

    /** Host and port as typed by the user, e.g. `192.168.1.20:47321`. */
    val display: String get() = "$host:$port"

    sealed interface ParseResult {
        data class Valid(val address: ServerAddress) : ParseResult

        data object InvalidHost : ParseResult

        data object InvalidPort : ParseResult
    }

    companion object {
        const val DEFAULT_PORT = 47321
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
            return ParseResult.Valid(ServerAddress(h, p))
        }
    }
}
