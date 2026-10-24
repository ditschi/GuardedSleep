package org.guardedsleep.app

import java.io.File
import org.guardedsleep.app.pairing.TurnCredentials
import org.guardedsleep.app.pairing.TurnServerInput
import org.guardedsleep.app.pairing.TurnServerInput.Result
import org.guardedsleep.app.pairing.TurnStore
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class TurnTest {
    @get:Rule val tmp = TemporaryFolder()

    private fun address(url: String) = TurnServerInput.validate(url, "", "")

    @Test
    fun validAddresses() {
        listOf(
                "turn:203.0.113.10:3478",
                "turns:turn.example.org:5349?transport=tcp",
                "turn:[2001:db8::1]:3478",
                "turn:host",
                "  turn:host:65535?transport=udp  ",
            )
            .forEach { assertTrue(it, address(it) is Result.Ok) }
    }

    @Test
    fun invalidAddresses() {
        listOf(
                "stun:x",
                "turn:",
                "turn: host",
                "turn:host:0",
                "turn:host:70000",
                "turn:host?transport=sctp",
                "https://x",
                "",
            )
            .forEach { assertEquals(it, Result.InvalidAddress, address(it)) }
    }

    @Test
    fun addressAndCredentialsAreTrimmed() {
        val r = TurnServerInput.validate(" turn:h ", " u ", " pw ")
        assertEquals(Result.Ok("turn:h", "u", "pw"), r)
    }

    @Test
    fun credentialRules() {
        val url = "turn:h"
        assertEquals(Result.InvalidCredentials, TurnServerInput.validate(url, "u", ""))
        assertEquals(Result.InvalidCredentials, TurnServerInput.validate(url, "", "pw"))
        val long = "a".repeat(257)
        assertEquals(Result.InvalidCredentials, TurnServerInput.validate(url, long, "pw"))
        assertEquals(Result.InvalidCredentials, TurnServerInput.validate(url, "u", long))
        val max = "a".repeat(256)
        assertTrue(TurnServerInput.validate(url, max, max) is Result.Ok)
        assertTrue(TurnServerInput.validate(url, "", "") is Result.Ok)
    }

    @Test
    fun storeRoundTripAndClear() {
        val cipher = FakeCipher()
        val file = File(tmp.root, "turn.bin")
        val store = TurnStore(file, cipher)
        assertNull(store.load())
        store.save(TurnCredentials("turn:h", "u", "pw"))
        val loaded = store.load()
        assertEquals("pw", loaded?.credential)
        assertFalse(String(file.readBytes(), Charsets.UTF_8).contains("pw"))
        store.clear()
        assertNull(store.load())
        assertFalse(file.exists())
        assertTrue(cipher.keyDeleted)
    }

    @Test
    fun unreadableFileIsClearedAndNull() {
        val file = File(tmp.root, "turn.bin")
        file.writeBytes(byteArrayOf(1, 2, 3))
        assertNull(TurnStore(file, FakeCipher()).load())
        assertFalse(file.exists())
    }

    @Test
    fun credentialsToStringHidesPassword() {
        assertFalse(TurnCredentials("turn:h", "u", "secretpw").toString().contains("secretpw"))
    }
}
