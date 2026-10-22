package org.guardedsleep.app

import org.guardedsleep.app.config.RelayStun
import org.junit.Assert.assertEquals
import org.junit.Test

class RelayStunTest {
    @Test
    fun usesTheRelayHostOnTheStunPort() {
        assertEquals(listOf("stun:relay.example.org:3478"), RelayStun.urls(listOf("wss://relay.example.org/ws")))
        assertEquals(listOf("stun:10.0.2.2:3478"), RelayStun.urls(listOf("ws://10.0.2.2:8787")))
    }

    @Test
    fun deduplicatesHostsAndBracketsIpv6() {
        assertEquals(
            listOf("stun:a.example:3478", "stun:[::1]:3478"),
            RelayStun.urls(listOf("wss://a.example/x", "ws://a.example:8787", "ws://[::1]:8787")),
        )
    }

    @Test
    fun noRelayMeansNoStunAndNeverAPublicDefault() {
        assertEquals(emptyList<String>(), RelayStun.urls(emptyList()))
        assertEquals(emptyList<String>(), RelayStun.urls(listOf("not a url", "")))
    }
}
