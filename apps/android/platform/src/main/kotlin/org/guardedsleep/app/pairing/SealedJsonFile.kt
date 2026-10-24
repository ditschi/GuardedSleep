package org.guardedsleep.app.pairing

import java.io.File
import org.json.JSONObject

/**
 * One JSON object sealed with [PairingCipher] in an app-private file (guardrail 7.2). Callers
 * synchronise; this class holds no state beyond the file.
 */
internal class SealedJsonFile(private val file: File, private val cipher: PairingCipher) {
    fun exists(): Boolean = file.exists()

    /** Null when absent. An unreadable file (key lost after a restore, corrupt) is cleared. */
    fun read(): JSONObject? {
        if (!file.exists()) return null
        return runCatching { JSONObject(String(cipher.decrypt(file.readBytes()), Charsets.UTF_8)) }
            .getOrElse {
                clear()
                null
            }
    }

    fun write(o: JSONObject) {
        val tmp = tmpFile()
        tmp.writeBytes(cipher.encrypt(o.toString().toByteArray(Charsets.UTF_8)))
        check(tmp.renameTo(file)) { "cannot replace ${file.name}" }
    }

    /** Deletes the file and the key alias. */
    fun clear() {
        file.delete()
        tmpFile().delete()
        cipher.deleteKey()
    }

    private fun tmpFile() = File(file.parentFile, file.name + ".tmp")
}

internal fun TurnCredentials.toJson(): JSONObject =
    JSONObject().put("u", url).put("n", username).put("c", credential)

internal fun turnCredentialsFromJson(o: JSONObject): TurnCredentials =
    TurnCredentials(o.getString("u"), o.getString("n"), o.getString("c"))
