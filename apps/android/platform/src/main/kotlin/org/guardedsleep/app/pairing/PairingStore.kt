package org.guardedsleep.app.pairing

import java.io.File
import org.json.JSONArray
import org.json.JSONObject

data class TurnCredentials(val url: String, val username: String, val credential: String) {
    // The password must never reach logs through string templates.
    override fun toString(): String = "TurnCredentials(url=<redacted>, credential=<redacted>)"
}

/** The one saved pairing of this phone (ADR-018: 1 baby + 1 parent). */
data class StoredPairing(
    val pskB64url: String,
    val signalingUrls: List<String>,
    /** The PSK verified at least once, so a later mismatch is never terminal (ADR-017 §3a). */
    val pairedBefore: Boolean,
    val turn: TurnCredentials? = null,
) {
    // The PSK must never reach logs through string templates.
    override fun toString(): String =
        "StoredPairing(pairedBefore=$pairedBefore, urls=${signalingUrls.size})"
}

/**
 * PSK, server list and TURN credentials sealed with [PairingCipher] in an app-private file. Plain
 * preferences never hold pairing material (guardrail 7.2).
 */
class PairingStore(file: File, cipher: PairingCipher) {
    private val sealed = SealedJsonFile(file, cipher)

    @Synchronized
    fun load(): StoredPairing? {
        val o = sealed.read() ?: return null
        // A well-formed file with missing fields is treated like an unreadable one.
        return runCatching {
            val urls = o.getJSONArray("s").let { a -> (0 until a.length()).map { a.getString(it) } }
            val turn = o.optJSONObject("t")?.let(::turnCredentialsFromJson)
            StoredPairing(o.getString("k"), urls, o.optBoolean("p", false), turn)
        }
            .getOrElse {
                sealed.clear()
                null
            }
    }

    @Synchronized
    fun save(pairing: StoredPairing) {
        val o =
            JSONObject()
                .put("k", pairing.pskB64url)
                .put("s", JSONArray(pairing.signalingUrls))
                .put("p", pairing.pairedBefore)
        pairing.turn?.let { o.put("t", it.toJson()) }
        sealed.write(o)
    }

    @Synchronized
    fun markPairedBefore() {
        load()?.takeIf { !it.pairedBefore }?.let { save(it.copy(pairedBefore = true)) }
    }

    @Synchronized fun exists(): Boolean = sealed.exists()

    /** Deletes the file and the key alias. */
    @Synchronized fun clear() = sealed.clear()
}
