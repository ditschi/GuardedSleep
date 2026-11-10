package org.guardedsleep.app.pairing

import java.io.File

/**
 * The user's own TURN server (address, user name, password) sealed with [PairingCipher] in an
 * app-private file, apart from the pairing so forgetting one does not delete the other. The
 * password is a secret like pairing material (guardrail 7.2): never in DataStore.
 */
class TurnStore(file: File, cipher: PairingCipher) {
    private val sealed = SealedJsonFile(file, cipher)

    @Synchronized
    fun load(): TurnCredentials? {
        val o = (sealed.read() as? SealedRead.Value)?.json ?: return null
        return runCatching { turnCredentialsFromJson(o) }
            .getOrElse {
                sealed.clear()
                null
            }
    }

    @Synchronized fun save(turn: TurnCredentials) = sealed.write(turn.toJson())

    /** Deletes the file and the key alias. */
    @Synchronized fun clear() = sealed.clear()

    companion object {
        const val FILE_NAME = "turn.bin"
        const val KEY_ALIAS = "guardedsleep_turn"
    }
}
