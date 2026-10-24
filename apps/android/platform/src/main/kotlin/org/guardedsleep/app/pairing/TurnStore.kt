package org.guardedsleep.app.pairing

import java.io.File
import org.json.JSONObject

/**
 * The user's own TURN server (address, user name, password) sealed with [PairingCipher] in an
 * app-private file, apart from the pairing so forgetting one does not delete the other. The
 * password is a secret like pairing material (guardrail 7.2): never in DataStore.
 */
class TurnStore(private val file: File, private val cipher: PairingCipher) {
    @Synchronized
    fun load(): TurnCredentials? {
        if (!file.exists()) return null
        return runCatching {
            val o = JSONObject(String(cipher.decrypt(file.readBytes()), Charsets.UTF_8))
            TurnCredentials(o.getString("u"), o.getString("n"), o.getString("c"))
        }
            .getOrElse {
                // Unreadable (key lost after a restore or a corrupt file): treat as not set.
                clear()
                null
            }
    }

    @Synchronized
    fun save(turn: TurnCredentials) {
        val o = JSONObject().put("u", turn.url).put("n", turn.username).put("c", turn.credential)
        val tmp = File(file.parentFile, file.name + ".tmp")
        tmp.writeBytes(cipher.encrypt(o.toString().toByteArray(Charsets.UTF_8)))
        check(tmp.renameTo(file)) { "cannot replace TURN file" }
    }

    /** Deletes the file and the key alias. */
    @Synchronized
    fun clear() {
        file.delete()
        File(file.parentFile, file.name + ".tmp").delete()
        cipher.deleteKey()
    }

    companion object {
        const val FILE_NAME = "turn.bin"
        const val KEY_ALIAS = "guardedsleep_turn"
    }
}
