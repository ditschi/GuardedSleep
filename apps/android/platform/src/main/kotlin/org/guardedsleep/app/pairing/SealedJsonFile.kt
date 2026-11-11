package org.guardedsleep.app.pairing

import android.security.keystore.KeyPermanentlyInvalidatedException
import java.io.File
import javax.crypto.AEADBadTagException
import org.json.JSONException
import org.json.JSONObject

/**
 * One JSON object sealed with [PairingCipher] in an app-private file (guardrail 7.2). Callers
 * synchronise; this class holds no state beyond the file.
 */
internal class SealedJsonFile(private val file: File, private val cipher: PairingCipher) {
    fun exists(): Boolean = file.exists()

    /**
     * The outcome of a read. Only data that can never be read again is cleared; any other failure
     * (Keystore busy, device still locked after boot) leaves the record alone, because deleting it
     * would unpair the phone over a passing condition.
     */
    fun read(): SealedRead {
        if (!file.exists()) return SealedRead.Absent
        val plain =
            try {
                cipher.decrypt(file.readBytes())
            } catch (e: Exception) {
                return if (e.isUnrecoverable()) {
                    clear()
                    SealedRead.Cleared
                } else {
                    SealedRead.Unreadable(e)
                }
            }
        return try {
            SealedRead.Value(JSONObject(String(plain, Charsets.UTF_8)))
        } catch (_: JSONException) {
            // Decrypted fine but is not JSON: corrupt for good.
            clear()
            SealedRead.Cleared
        }
    }

    /**
     * The bytes reach the disk before the rename makes them the record: the pairing store persists
     * the parent secret before it is sent (persist before send). A failure throws.
     */
    fun write(o: JSONObject) {
        val tmp = tmpFile()
        java.io.FileOutputStream(tmp).use { out ->
            out.write(cipher.encrypt(o.toString().toByteArray(Charsets.UTF_8)))
            out.fd.sync()
        }
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

/** The result of [SealedJsonFile.read]. */
internal sealed interface SealedRead {
    /** No record on disk. */
    data object Absent : SealedRead

    /** The record was unrecoverable (tag mismatch, invalidated key, corrupt) and is deleted. */
    data object Cleared : SealedRead

    /** The record exists but cannot be read right now; it is kept. */
    class Unreadable(val cause: Exception) : SealedRead

    class Value(val json: JSONObject) : SealedRead
}

/**
 * Tag mismatch (key lost after a restore, tampering), an invalidated key and a truncated file (the
 * cipher's size check) never recover. A missing alias lands in the first case too: the cipher
 * generates a new key, so the old file fails its tag.
 */
private fun Exception.isUnrecoverable(): Boolean =
    this is AEADBadTagException ||
        this is KeyPermanentlyInvalidatedException ||
        this is IllegalArgumentException
