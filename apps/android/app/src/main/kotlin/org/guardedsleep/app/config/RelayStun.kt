package org.guardedsleep.app.config

import java.net.URI

/**
 * ICE STUN servers come from the configured relay, never from a public default: the relay answers
 * STUN Binding requests on UDP 3478 (add-rust-signaling-server). Only connection metadata reaches a
 * server we run or the user chose (AGENTS.md section 7.1).
 */
object RelayStun {
    const val PORT = 3478

    /** One `stun:` URL per distinct host of the signaling URLs; unparsable entries are skipped. */
    fun urls(signalingUrls: List<String>): List<String> =
        signalingUrls
            .mapNotNull {
                runCatching { URI(it.trim()).host }.getOrNull()?.takeIf { it.isNotEmpty() }
            }
            .distinct()
            .map { host ->
                if (':' in host && !host.startsWith("[")) "stun:[$host]:$PORT"
                else "stun:$host:$PORT"
            }
}
