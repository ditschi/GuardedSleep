package org.guardedsleep.app.pairing

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** AGENTS.md section 7.3: the PSK never reaches a log or crash text through `toString()`. */
class PairingPayloadToStringTest {
    @Test
    fun toStringDoesNotShowThePsk() {
        val psk = PairingPayload.b64(ByteArray(32) { 0x2a })
        val text = PairingPayload(psk, listOf("wss://relay.example/ws"), 1_000L).toString()
        assertFalse(text, text.contains(psk))
        assertTrue(text, text.contains("<redacted>"))
    }
}
