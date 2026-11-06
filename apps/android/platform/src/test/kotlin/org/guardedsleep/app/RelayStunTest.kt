package org.guardedsleep.app

import org.guardedsleep.app.config.PublicLists
import org.guardedsleep.app.config.RelayStun
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

class RelayStunTest {
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
