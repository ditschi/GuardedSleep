package org.guardedsleep.app

import java.io.File
import org.guardedsleep.app.pairing.PairingPayload
import org.guardedsleep.app.pairing.PairingPayload.Companion.Result
import org.guardedsleep.app.pairing.PairingStore
import org.guardedsleep.app.pairing.StoredPairing
import org.guardedsleep.app.pairing.TurnCredentials
import org.guardedsleep.app.ui.model.PairingError
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class PairingTest {
    @get:Rule val tmp = TemporaryFolder()

    private val now = 1_700_000_000_000L
    private val psk = ByteArray(32) { it.toByte() }

    private fun payload(
        v: Int = 3,
        k: String = PairingPayload.b64(psk),
        s: String = """["ws://10.0.2.2:8787","wss://a.example/sig"]""",
        x: Long = now / 1000 + 300,
    ) = """{"v":$v,"k":"$k","s":$s,"x":$x}"""

    private fun rejected(text: String) = (PairingPayload.parse(text, now) as Result.Rejected).error

    @Test
    fun roundTrip() {
        val p = PairingPayload.create(psk, listOf("ws://h:1"), now)
        val parsed = PairingPayload.parse(p.toJson(), now) as Result.Ok
        assertEquals(p, parsed.payload)
        assertEquals((now + 300_000) / 1000, p.expiresAtEpochSec)
    }

    @Test
    fun parsedInviteCarriesTheCoreValues() {
        val parsed = PairingPayload.parse(payload(x = now / 1000 + 5), now) as Result.Ok
        assertEquals(PairingPayload.b64(psk), parsed.payload.pskB64url)
        assertEquals(
            listOf("ws://10.0.2.2:8787", "wss://a.example/sig"),
            parsed.payload.signalingUrls,
        )
        assertEquals(now / 1000 + 5, parsed.payload.expiresAtEpochSec)
    }

    @Test
    fun expiryBoundaryIsExclusive() {
        assertTrue(PairingPayload.parse(payload(x = now / 1000 + 1), now) is Result.Ok)
        assertEquals(PairingError.EXPIRED, rejected(payload(x = now / 1000)))
    }

    @Test
    fun rejectsMissingExpiryAndBadUrlHost() {
        assertEquals(
            PairingError.NOT_OUR_CODE,
            rejected("""{"v":3,"k":"${PairingPayload.b64(psk)}","s":["ws://h"]}"""),
        )
        assertEquals(PairingError.NOT_OUR_CODE, rejected(payload(s = """["ws:///path"]""")))
    }

    @Test
    fun acceptsPastedWhitespace() {
        assertTrue(PairingPayload.parse("  \n${payload()}\n ", now) is Result.Ok)
    }

    @Test
    fun rejectsExpired() = assertEquals(PairingError.EXPIRED, rejected(payload(x = now / 1000 - 1)))

    @Test
    fun rejectsWrongVersion() = assertEquals(PairingError.NOT_OUR_CODE, rejected(payload(v = 2)))

    @Test
    fun rejectsShortKey() =
        assertEquals(
            PairingError.NOT_OUR_CODE,
            rejected(payload(k = PairingPayload.b64(ByteArray(16)))),
        )

    @Test
    fun rejectsBadScheme() {
        assertEquals(PairingError.NOT_OUR_CODE, rejected(payload(s = """["http://h"]""")))
        assertEquals(PairingError.NOT_OUR_CODE, rejected(payload(s = "[]")))
    }

    @Test
    fun truncatedJsonIsIncomplete() {
        assertEquals(PairingError.INCOMPLETE_PASTE, rejected(payload().dropLast(10)))
        assertEquals(PairingError.INCOMPLETE_PASTE, rejected("   "))
    }

    @Test
    fun otherTextIsNotOurCode() {
        assertEquals(PairingError.NOT_OUR_CODE, rejected("https://example.com"))
        assertEquals(
            PairingError.NOT_OUR_CODE,
            rejected("""{"v":1,"r":"room","s":"ws://x","k":"abc"}"""),
        )
    }

    private fun store(cipher: FakeCipher = FakeCipher()) =
        PairingStore(File(tmp.root, "pairing.bin"), cipher) to cipher

    @Test
    fun storeRoundTripAndFileIsNotPlain() {
        val (s, _) = store()
        assertNull(s.load())
        val p =
            StoredPairing(
                PairingPayload.b64(psk),
                listOf("ws://h"),
                false,
                TurnCredentials("turn:t", "u", "c"),
            )
        s.save(p)
        assertEquals(p, s.load())
        assertFalse(File(tmp.root, "pairing.bin").readText().contains(p.pskB64url))
        s.markPairedBefore()
        assertTrue(s.load()!!.pairedBefore)
        assertFalse(p.toString().contains(p.pskB64url))
    }

    @Test
    fun clearDeletesFileAndKey() {
        val (s, cipher) = store()
        s.save(StoredPairing("k", listOf("ws://h"), true))
        s.clear()
        assertFalse(s.exists())
        assertTrue(cipher.keyDeleted)
    }

    @Test
    fun corruptFileCountsAsNotPaired() {
        val (s, _) = store()
        File(tmp.root, "pairing.bin").writeBytes(byteArrayOf(1, 2, 3))
        assertNull(s.load())
        assertFalse(s.exists())
    }
}
