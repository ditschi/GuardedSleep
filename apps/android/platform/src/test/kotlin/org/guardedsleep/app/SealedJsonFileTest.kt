package org.guardedsleep.app

import java.io.File
import javax.crypto.AEADBadTagException
import org.guardedsleep.app.pairing.PairingCipher
import org.guardedsleep.app.pairing.SealedJsonFile
import org.guardedsleep.app.pairing.SealedRead
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class SealedJsonFileTest {
    @get:Rule val tmp = TemporaryFolder()

    private val file by lazy { File(tmp.root, "x.bin") }

    private fun sealedWith(cipher: PairingCipher): SealedJsonFile =
        SealedJsonFile(file, cipher).also { it.write(JSONObject().put("a", 1)) }

    @Test
    fun absentIsAbsent() {
        assertTrue(SealedJsonFile(file, FakeCipher()).read() is SealedRead.Absent)
    }

    @Test
    fun transientErrorKeepsTheRecordAndTheNextReadSucceeds() {
        val f = sealedWith(FlakyCipher(1) { java.security.KeyStoreException("busy") })
        assertTrue(f.read() is SealedRead.Unreadable)
        assertTrue(file.exists())
        assertEquals(1, (f.read() as SealedRead.Value).json.getInt("a"))
    }

    @Test
    fun badTagClearsTheRecord() {
        val f = sealedWith(FlakyCipher(1) { AEADBadTagException() })
        assertTrue(f.read() is SealedRead.Cleared)
        assertFalse(file.exists())
        assertTrue(f.read() is SealedRead.Absent)
    }

    @Test
    fun corruptJsonAfterDecryptClears() {
        val cipher = FakeCipher()
        file.writeBytes(cipher.encrypt("not json".toByteArray()))
        assertTrue(SealedJsonFile(file, cipher).read() is SealedRead.Cleared)
        assertFalse(file.exists())
        assertTrue(cipher.keyDeleted)
    }
}
