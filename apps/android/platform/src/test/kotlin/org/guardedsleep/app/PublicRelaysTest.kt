package org.guardedsleep.app

import org.guardedsleep.app.config.PublicLists
import org.guardedsleep.app.config.PublicRelays
import org.guardedsleep.app.config.RelayStun
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.guardedsleep_core.MailboxKind

class PublicRelaysTest {
    private val psk = ByteArray(32) { (it + 1).toByte() }

    @Test
    fun theReleaseValueOfTheGateIsOffAndDebugIsOn() {
        // The release gate until the disclosure and off switch exist (tasks 5.3 and 6.2).
        assertFalse("release builds must not add n: entries", PublicRelays.enabledFor(false))
        assertTrue(PublicRelays.enabledFor(true))
    }

    @Test
    fun anOffGateNamesNoPublicRelay() {
        assertEquals(emptyList<Any>(), PublicRelays.forInvite(psk, enabled = false))
    }

    @Test
    fun theShippedListHasEnoughOperatorsAndOnlyWssHosts() {
        val relays = PublicLists.nostrRelays
        assertTrue(relays.size >= 6)
        assertTrue(relays.map { it.operator }.distinct().size >= 4)
        relays.forEach { assertEquals("wss://${it.host}", it.url) }
    }

    @Test
    fun theInviteGetsThreeListedRelaysOfDistinctOperatorsDeterministically() {
        val picked = PublicRelays.forInvite(psk, enabled = true)
        assertEquals(PublicRelays.PICK_COUNT, picked.size)
        assertTrue(picked.all { it.kind == MailboxKind.NOSTR })
        val operators = picked.map { m ->
            PublicLists.nostrRelays.single { it.url == m.url }.operator
        }
        assertEquals(3, operators.distinct().size)
        assertEquals(picked, PublicRelays.forInvite(psk, enabled = true))
    }

    @Test
    fun parkedRelaysAreSkipped() {
        val first = PublicRelays.forInvite(psk, enabled = true).first().url.removePrefix("wss://")
        val after = PublicRelays.forInvite(psk, enabled = true, parked = listOf(first))
        assertEquals(3, after.size)
        assertFalse(after.any { it.url == "wss://$first" })
    }

    @Test
    fun theDefaultIceServersAreTwoListedOperators() {
        val urls = RelayStun.iceUrls(emptyList())
        val operators =
            urls.map { u -> PublicLists.stunServers.single { it.url == u }.operator }.distinct()
        assertEquals(2, operators.size)
        assertFalse(urls.any { it.contains("google") })
    }

    @Test
    fun anOwnRelayUsesOnlyItsOwnStun() {
        assertEquals(
            listOf("stun:relay.example.org:3478"),
            RelayStun.iceUrls(listOf("wss://relay.example.org/ws")),
        )
    }

    @Test
    fun onlySameWifiAllowsNoPublicStun() {
        assertEquals(emptyList<String>(), RelayStun.iceUrls(emptyList(), onlyLocal = true))
        // An own relay's STUN is still its own.
        assertEquals(
            listOf("stun:10.0.2.2:3478"),
            RelayStun.iceUrls(listOf("ws://10.0.2.2:8787"), onlyLocal = true),
        )
    }
}
