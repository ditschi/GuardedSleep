package org.guardedsleep.app.pairing

import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.security.keystore.StrongBoxUnavailableException
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/** Seals the pairing file. Abstracted so JVM tests can use a fake without AndroidKeyStore. */
interface PairingCipher {
    fun encrypt(plain: ByteArray): ByteArray

    fun decrypt(sealed: ByteArray): ByteArray

    /** Removes the key itself, so a copied file can never be decrypted again. */
    fun deleteKey()
}

/** AES-256-GCM with a non-exportable AndroidKeyStore key (StrongBox when the device has it). */
class KeystorePairingCipher(private val alias: String = KEY_ALIAS) : PairingCipher {
    override fun encrypt(plain: ByteArray): ByteArray {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, key())
        return cipher.iv + cipher.doFinal(plain)
    }

    override fun decrypt(sealed: ByteArray): ByteArray {
        require(sealed.size > IV_BYTES) { "sealed data too short" }
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(TAG_BITS, sealed, 0, IV_BYTES))
        return cipher.doFinal(sealed, IV_BYTES, sealed.size - IV_BYTES)
    }

    override fun deleteKey() {
        runCatching { keyStore().deleteEntry(alias) }
    }

    private fun keyStore(): KeyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }

    private fun key(): SecretKey {
        (keyStore().getKey(alias, null) as? SecretKey)?.let { return it }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            try {
                return generate(strongBox = true)
            } catch (_: StrongBoxUnavailableException) {
                // Fall through to the TEE-backed key.
            }
        }
        return generate(strongBox = false)
    }

    private fun generate(strongBox: Boolean): SecretKey {
        val spec =
            KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .apply {
                    if (strongBox && Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) setIsStrongBoxBacked(true)
                }
                .build()
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)
            .apply { init(spec) }
            .generateKey()
    }

    companion object {
        const val KEY_ALIAS = "gs_pairing_v1"
        private const val ANDROID_KEYSTORE = "AndroidKeyStore"
        private const val TRANSFORMATION = "AES/GCM/NoPadding"
        private const val IV_BYTES = 12
        private const val TAG_BITS = 128
    }
}
