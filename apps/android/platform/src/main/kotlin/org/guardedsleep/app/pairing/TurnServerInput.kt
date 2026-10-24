package org.guardedsleep.app.pairing

/**
 * Syntax check of the TURN entry (design: add-turn-credentials). The rule is the same on the web
 * client; both test the same case table.
 */
object TurnServerInput {
    sealed interface Result {
        data class Ok(val url: String, val username: String, val password: String) : Result {
            override fun toString(): String = "Ok(url=<redacted>, password=<redacted>)"
        }

        data object InvalidAddress : Result

        data object InvalidCredentials : Result
    }

    const val MAX_CREDENTIAL_CHARS = 256

    // turn: or turns:, then an IPv4/DNS host or a bracketed IPv6, optional port, optional
    // transport.
    private val ADDRESS =
        Regex(
            "^turns?:(?:\\[[0-9A-Fa-f:.]+\\]|[A-Za-z0-9.-]+)(?::(\\d{1,5}))?" +
                "(?:\\?transport=(?:udp|tcp))?$"
        )

    fun validate(url: String, username: String, password: String): Result {
        val u = url.trim()
        val match = ADDRESS.matchEntire(u) ?: return Result.InvalidAddress
        val port = match.groupValues[1]
        if (port.isNotEmpty() && port.toInt() !in 1..65535) return Result.InvalidAddress
        val n = username.trim()
        val p = password.trim()
        if (n.isEmpty() != p.isEmpty()) return Result.InvalidCredentials
        if (n.length > MAX_CREDENTIAL_CHARS || p.length > MAX_CREDENTIAL_CHARS) {
            return Result.InvalidCredentials
        }
        return Result.Ok(u, n, p)
    }
}
