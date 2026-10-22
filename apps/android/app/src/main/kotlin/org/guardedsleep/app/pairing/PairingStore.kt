package org.guardedsleep.app.pairing

import java.io.File
import org.json.JSONArray
import org.json.JSONObject

data class TurnCredentials(val url: String, val username: String, val credential: String)

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
class PairingStore(private val file: File, private val cipher: PairingCipher) {
    @Synchronized
    fun load(): StoredPairing? {
        if (!file.exists()) return null
        return runCatching {
            val o = JSONObject(String(cipher.decrypt(file.readBytes()), Charsets.UTF_8))
            val urls = o.getJSONArray("s").let { a -> (0 until a.length()).map { a.getString(it) } }
            val turn =
                o.optJSONObject("t")?.let {
                    TurnCredentials(it.getString("u"), it.getString("n"), it.getString("c"))
                }
            StoredPairing(o.getString("k"), urls, o.optBoolean("p", false), turn)
        }
            .getOrElse {
                // Unreadable (key lost after a restore or a corrupt file): treat as not paired.
                clear()
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
        pairing.turn?.let {
            o.put("t", JSONObject().put("u", it.url).put("n", it.username).put("c", it.credential))
        }
        val tmp = File(file.parentFile, file.name + ".tmp")
        tmp.writeBytes(cipher.encrypt(o.toString().toByteArray(Charsets.UTF_8)))
        check(tmp.renameTo(file)) { "cannot replace pairing file" }
    }

    @Synchronized
    fun markPairedBefore() {
        load()?.takeIf { !it.pairedBefore }?.let { save(it.copy(pairedBefore = true)) }
    }

    @Synchronized fun exists(): Boolean = file.exists()

    /** Deletes the file and the key alias. */
    @Synchronized
    fun clear() {
        file.delete()
        File(file.parentFile, file.name + ".tmp").delete()
        cipher.deleteKey()
    }
}
