package org.guardedsleep.app.config

import java.net.URI

/**
 * ICE STUN servers (design section 7): a configured relay brings its own, the relay answers STUN
 * Binding requests on UDP 3478 (add-rust-signaling-server). Without one the default pair of
 * operators from `data/stun-servers.json` is used ([PublicLists.defaultStunUrls]). Only connection
 * metadata reaches a server we run, the user chose or the curated list names (AGENTS.md section
 * 7.1); `just lint` rejects every other STUN host in the source.
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

    /**
     * The STUN URLs of an attempt: only the own relay's when one is configured; otherwise
     * [publicDefault]; none at all with "Only same Wi-Fi or my own server" ([onlyLocal]).
     */
    fun iceUrls(
        signalingUrls: List<String>,
        onlyLocal: Boolean = false,
        publicDefault: List<String> = PublicLists.defaultStunUrls,
    ): List<String> {
        val own = urls(signalingUrls)
        return when {
            own.isNotEmpty() -> own
            onlyLocal -> emptyList()
            else -> publicDefault
        }
    }
}
