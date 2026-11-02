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

    /** What saving the entry does; the same outcomes as the web `resolveTurnSave`. */
    sealed interface Resolution {
        /** An empty address removes the saved entry. */
        data object Cleared : Resolution

        data class Saved(val turn: TurnCredentials) : Resolution

        /** [reason] is [Result.InvalidAddress] or [Result.InvalidCredentials]. */
        data class Invalid(val reason: Result) : Resolution
    }

    const val MAX_CREDENTIAL_CHARS = 256

    // turn: or turns:, then an IPv4/DNS host or a bracketed IPv6, optional port, optional
    // transport.
    private val ADDRESS =
        Regex(
            "^turns?:(?:\\[[0-9A-Fa-f:.]+\\]|[A-Za-z0-9][A-Za-z0-9.-]*)(?::(\\d{1,5}))?" +
                "(?:\\?transport=(?:udp|tcp))?$"
        )

    fun validate(url: String, username: String, password: String): Result {
        val u = url.trim()
        val match = ADDRESS.matchEntire(u) ?: return Result.InvalidAddress
        val port = match.groupValues[1]
        if (port.isNotEmpty() && port.toInt() !in 1..65535) return Result.InvalidAddress
        val n = username.trim()
        // The password is used as typed: spaces can be part of a valid credential.
        val p = password
        if (n.isEmpty() != p.isEmpty()) return Result.InvalidCredentials
        if (n.length > MAX_CREDENTIAL_CHARS || p.length > MAX_CREDENTIAL_CHARS) {
            return Result.InvalidCredentials
        }
        return Result.Ok(u, n, p)
    }

    /**
     * Turns the typed fields into a save: an empty address clears, address and user name are
     * trimmed (the password never is), and an empty password keeps the [saved] one while address
     * and user name are unchanged.
     */
    fun resolve(
        saved: TurnCredentials?,
        url: String,
        username: String,
        password: String,
    ): Resolution {
        val u = url.trim()
        if (u.isEmpty()) return Resolution.Cleared
        val n = username.trim()
        val keep = password.isEmpty() && saved != null && saved.url == u && saved.username == n
        val effective = if (keep && saved != null) saved.credential else password
        return when (val r = validate(u, n, effective)) {
            Result.InvalidAddress,
            Result.InvalidCredentials -> Resolution.Invalid(r)
            is Result.Ok -> Resolution.Saved(TurnCredentials(r.url, r.username, r.password))
        }
    }
}
