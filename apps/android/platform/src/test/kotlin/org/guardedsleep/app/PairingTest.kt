package org.guardedsleep.app

import java.io.File
import org.guardedsleep.app.pairing.PairingPayload
import org.guardedsleep.app.pairing.PairingPayload.Companion.Result
import org.guardedsleep.app.pairing.PairingStore
import org.guardedsleep.app.pairing.StoredMailbox
import org.guardedsleep.app.pairing.StoredPairing
import org.guardedsleep.app.pairing.TurnCredentials
import org.guardedsleep.app.ui.model.PairingError
import org.json.JSONObject
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
        v: Int = 4,
        k: String = PairingPayload.b64(psk),
        m: String = """["l:10.0.2.2:8787","r:a.example/sig"]""",
        x: Long = now / 1000 + 300,
    ) = """{"v":$v,"k":"$k","m":$m,"x":$x}"""

    private fun rejected(text: String) = (PairingPayload.parse(text, now) as Result.Rejected).error

    @Test
    fun roundTrip() {
        val p = PairingPayload.create(psk, listOf("wss://relay.example:8443/sig"), now)
        val parsed = PairingPayload.parse(p.toInviteText()!!, now) as Result.Ok
        assertEquals(p, parsed.payload)
        assertEquals((now + 300_000) / 1000, p.expiresAtEpochSec)
    }

    @Test
    fun inviteTextIsTheCompactV4Form() {
        val p =
            PairingPayload.create(psk, listOf("wss://a.example/sig", "ws://192.168.1.5:8787"), now)
        val json = JSONObject(p.toInviteText()!!)
        assertEquals(4, json.getInt("v"))
        assertEquals(PairingPayload.b64(psk), json.getString("k"))
        assertEquals(
            """["r:a.example/sig","l:192.168.1.5:8787"]""",
            json.getJSONArray("m").toString(),
        )
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
    fun anNEntryOfTheRemovedNostrKindIsNotOurCode() {
        val text = payload(m = """["r:a.example","n:relay.example.org"]""")
        assertEquals(PairingError.NOT_OUR_CODE, rejected(text))
    }

    @Test
    fun theCoreRefusesAnAddressThatIsNeitherRelayNorPrivateLan() {
        val publicLan = PairingPayload.create(psk, listOf("ws://8.8.8.8:8787"), now)
        assertNull(publicLan.toInviteText())
        val hostnameLan = PairingPayload.create(psk, listOf("ws://gs.local:8787"), now)
        assertNull(hostnameLan.toInviteText())
        assertNull(PairingPayload.create(psk, listOf("http://h"), now).toInviteText())
    }

    @Test
    fun expiryBoundaryIsExclusive() {
        assertTrue(PairingPayload.parse(payload(x = now / 1000 + 1), now) is Result.Ok)
        assertEquals(PairingError.EXPIRED, rejected(payload(x = now / 1000)))
    }

    @Test
    fun rejectsMissingExpiryAndBadMailbox() {
        assertEquals(
            PairingError.NOT_OUR_CODE,
            rejected("""{"v":4,"k":"${PairingPayload.b64(psk)}","m":["r:h.example"]}"""),
        )
        assertEquals(PairingError.NOT_OUR_CODE, rejected(payload(m = """["r:"]""")))
        assertEquals(PairingError.NOT_OUR_CODE, rejected(payload(m = """["l:169.254.1.1:80"]""")))
        assertEquals(PairingError.NOT_OUR_CODE, rejected(payload(m = "[]")))
    }

    @Test
    fun acceptsPastedWhitespace() {
        assertTrue(PairingPayload.parse("  \n${payload()}\n ", now) is Result.Ok)
    }

    @Test
    fun rejectsExpired() = assertEquals(PairingError.EXPIRED, rejected(payload(x = now / 1000 - 1)))

    @Test
    fun rejectsOtherVersionsIncludingV3() {
        assertEquals(PairingError.NOT_OUR_CODE, rejected(payload(v = 2)))
        assertEquals(PairingError.NOT_OUR_CODE, rejected(payload(v = 3)))
        val v3 =
            """{"v":3,"k":"${PairingPayload.b64(psk)}","s":["wss://h.example"],"x":${now / 1000 + 300}}"""
        assertEquals(PairingError.NOT_OUR_CODE, rejected(v3))
    }

    @Test
    fun rejectsShortKey() =
        assertEquals(
            PairingError.NOT_OUR_CODE,
            rejected(payload(k = PairingPayload.b64(ByteArray(16)))),
        )

    @Test
    fun rejectsBadScheme() {
        assertEquals(PairingError.NOT_OUR_CODE, rejected(payload(m = """["wss://h.example"]""")))
        assertEquals(PairingError.NOT_OUR_CODE, rejected(payload(m = """["x:h.example"]""")))
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
    fun theParentSecretAndTheMailboxSetLiveInTheEncryptedRecord() {
        val (s, _) = store()
        val secret = PairingPayload.b64(ByteArray(32) { (it + 100).toByte() })
        val set =
            listOf(
                StoredMailbox("relay", "wss://r.example/sig"),
                StoredMailbox("lan", "ws://192.168.1.5:8787"),
            )
        // Nothing to attach to before the first verification saved the record.
        assertFalse(s.setParentSecret(secret))
        assertFalse(s.setMailboxes(set))
        s.save(StoredPairing(PairingPayload.b64(psk), listOf("wss://old.example"), true))
        assertTrue(s.setParentSecret(secret))
        assertTrue(s.setMailboxes(set))
        val loaded = s.load()!!
        assertEquals(secret, loaded.parentSecretB64url)
        assertEquals(set, loaded.mailboxes)
        // The platform opens the relay and LAN mailboxes of the set.
        assertEquals(listOf("wss://r.example/sig", "ws://192.168.1.5:8787"), loaded.signalingUrls)
        // Never in the clear on disk, never in the string form.
        val onDisk = File(tmp.root, "pairing.bin").readText()
        assertFalse(onDisk.contains(secret))
        assertFalse(loaded.toString().contains(secret))
        assertTrue(loaded.pairedBefore)
        // Another save keeps what the record holds only if the caller passes it on.
        s.markPairedBefore()
        assertEquals(secret, s.load()!!.parentSecretB64url)
    }

    @Test
    fun aMailboxSetWithoutRelayOrLanKeepsTheCurrentUrls() {
        val (s, _) = store()
        s.save(StoredPairing(PairingPayload.b64(psk), listOf("wss://keep.example"), true))
        // A record of an earlier alpha may still name a `nostr` entry: it is not a signaling URL.
        s.setMailboxes(listOf(StoredMailbox("nostr", "wss://n.example")))
        assertEquals(listOf("wss://keep.example"), s.load()!!.signalingUrls)
    }

    @Test
    fun aRecordWithoutTheNewFieldsStillLoads() {
        val (s, cipher) = store()
        val old =
            JSONObject()
                .put("k", PairingPayload.b64(psk))
                .put("s", org.json.JSONArray(listOf("wss://old.example")))
                .put("p", true)
        File(tmp.root, "pairing.bin").writeBytes(cipher.encrypt(old.toString().toByteArray()))
        val loaded = s.load()!!
        assertNull(loaded.parentSecretB64url)
        assertEquals(emptyList<StoredMailbox>(), loaded.mailboxes)
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
