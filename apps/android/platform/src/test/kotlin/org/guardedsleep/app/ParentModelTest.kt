package org.guardedsleep.app

import org.guardedsleep.app.alerts.NoiseKind
import org.guardedsleep.app.alerts.Notice
import org.guardedsleep.app.config.AlertPolicy
import org.guardedsleep.app.fixtures.RecordingSink
import org.guardedsleep.app.link.ParentModel
import org.guardedsleep.app.ui.model.LinkHint
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.ListenMode
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.guardedsleep_core.LinkEvent
import uniffi.guardedsleep_core.LinkState

class ParentModelTest {
    private val clock = FakeClock()
    private val sink = RecordingSink()
    private val effects = RecordingEffects()
    private val model = ParentModel(clock, sink, effects) { 0.12 }

    private fun st(s: LinkState, level: Int = 0, cause: String? = null, ack: Long? = null) {
        model.onStatus(status(s, level, cause = cause, ack = ack), clock.elapsed)
    }

    private fun connected(): Long {
        st(LinkState.CONNECTING)
        clock.advance(500)
        st(LinkState.CONNECTED)
        return clock.wall
    }

    private fun outage(seconds: Int) {
        st(LinkState.RECONNECTING)
        clock.advance(seconds * 1000L)
        st(LinkState.CONNECTED)
    }

    @Test
    fun calmHintsReachTheLinkStateAndAreNotAlarms() {
        connected()
        model.onEvent(LinkEvent.Hint("recovery_path_unavailable", true), clock.elapsed)
        assertEquals(setOf(LinkHint.RECOVERY_PATH_UNAVAILABLE), model.state.value.link.hints)
        assertEquals(LinkPhase.CONNECTED, model.state.value.link.phase)
        assertTrue(sink.lost.isEmpty())
        model.onEvent(LinkEvent.Hint("recovery_path_unavailable", false), clock.elapsed)
        assertTrue(model.state.value.link.hints.isEmpty())
        model.onEvent(LinkEvent.Hint("clock_skew", true), clock.elapsed)
        model.reset()
        assertTrue(model.state.value.link.hints.isEmpty())
    }

    @Test
    fun theSecretAndTheMailboxEventsChangeNothingTheParentShows() {
        connected()
        val before = model.state.value
        model.onEvent(
            LinkEvent.ParentPinned(uniffi.guardedsleep_core.ParentSecret(ByteArray(32)), null),
            clock.elapsed,
        )
        model.onEvent(LinkEvent.MailboxesUpdated(emptyList()), clock.elapsed)
        assertEquals(before, model.state.value)
    }

    @Test
    fun neverQuietWhileDisconnected() {
        assertNull(model.state.value.quietSinceWallMs)
        st(LinkState.CONNECTING)
        assertNull(model.state.value.quietSinceWallMs)
        val since = connected()
        assertEquals(since, model.state.value.quietSinceWallMs)
        st(LinkState.RECONNECTING, level = 1)
        assertNull(model.state.value.quietSinceWallMs)
        assertEquals(1, model.state.value.link.level)
    }

    @Test
    fun afterRecoveryQuietOnlyWithCompleteSummary() {
        val since = connected()
        outage(5)
        assertNull(model.state.value.quietSinceWallMs)
        model.onEvent(LinkEvent.MissedEvents(0u, 0.0, true, 5000u), clock.elapsed)
        assertEquals(since, model.state.value.quietSinceWallMs)
        assertEquals(0, model.state.value.missed!!.count)
    }

    @Test
    fun incompleteSummaryRestartsQuietAtReconnect() {
        connected()
        outage(5)
        val reconnect = clock.wall
        model.onEvent(LinkEvent.MissedEvents(0u, 0.0, false, 5000u), clock.elapsed)
        assertEquals(reconnect, model.state.value.quietSinceWallMs)
        assertNull(model.state.value.missed!!.count)
    }

    @Test
    fun missingSummaryFallsBackAfterGrace() {
        connected()
        outage(5)
        val reconnect = clock.wall
        clock.advance(AlertPolicy.MISSED_EVENTS_GRACE_MS - 1)
        model.onTick(clock.elapsed)
        assertNull(model.state.value.quietSinceWallMs)
        clock.advance(2)
        model.onTick(clock.elapsed)
        assertEquals(reconnect, model.state.value.quietSinceWallMs)
    }

    @Test
    fun restoredNoticeExpires() {
        connected()
        st(LinkState.RECONNECTING, 2)
        clock.advance(90_000)
        model.onEvent(LinkEvent.LinkRestored(90_000u), clock.elapsed)
        st(LinkState.CONNECTED)
        assertEquals(90_000L, model.state.value.restoredAfterMs)
        assertTrue(sink.notices.any { it is Notice.LinkRestored })
        clock.advance(AlertPolicy.RESTORED_NOTICE_MS)
        model.onTick(clock.elapsed)
        assertNull(model.state.value.restoredAfterMs)
    }

    @Test
    fun linkLostAndStopNoticesReachTheSink() {
        connected()
        st(LinkState.RECONNECTING, 2, cause = "peer_absent")
        model.onEvent(LinkEvent.LinkLost(60_000u), clock.elapsed)
        assertEquals(listOf(60_000L), sink.lost)
        assertEquals(2, sink.snapshots.last().level)
        st(LinkState.PEER_STOPPED)
        model.onEvent(LinkEvent.PeerStopped, clock.elapsed)
        assertEquals(LinkPhase.PEER_STOPPED, model.state.value.link.phase)
        assertTrue(sink.notices.contains(Notice.PeerStopped))
        assertEquals(0, model.state.value.link.level)
    }

    @Test
    fun acknowledgeIsVisibleToTheSink() {
        connected()
        st(LinkState.RECONNECTING, 2)
        assertFalse(sink.snapshots.last().acknowledged)
        st(LinkState.RECONNECTING, 2, ack = clock.elapsed + 300_000)
        assertTrue(sink.snapshots.last().acknowledged)
    }

    @Test
    fun noiseEventsAndSnooze() {
        connected()
        model.onEvent(LinkEvent.NoiseAlert(0.4, false, false), clock.elapsed)
        assertTrue(model.state.value.noiseAlertActive)
        assertNotNull(model.state.value.noiseSinceWallMs)
        assertEquals(NoiseKind.NEW, sink.noise.last().first)
        assertNull(model.state.value.quietSinceWallMs)
        clock.advance(30_000)
        model.onEvent(LinkEvent.NoiseAlert(0.4, false, false), clock.elapsed)
        assertEquals(NoiseKind.CONTINUE, sink.noise.last().first)

        model.snooze(10)
        assertNotNull(model.state.value.snoozedUntilWallMs)
        model.onEvent(LinkEvent.NoiseAlert(0.4, false, true), clock.elapsed)
        assertEquals(true, sink.noise.last().third)
        assertEquals(true, effects.muted.last())
    }

    @Test
    fun noiseEndsAfterTwoQuietMinutes() {
        val since = connected()
        model.onEvent(LinkEvent.NoiseAlert(0.4, true, false), clock.elapsed)
        assertTrue(model.state.value.noiseIsTest)
        assertEquals(1, effects.tests)
        clock.advance(AlertPolicy.NOISE_QUIET_AFTER_MS)
        model.onTick(clock.elapsed)
        assertFalse(model.state.value.noiseAlertActive)
        assertEquals(1, sink.ended)
        assertTrue(model.state.value.quietSinceWallMs!! >= since)
    }

    @Test
    fun listenRulesAndLimit() {
        assertFalse(model.setListen(ListenMode.HOLD, clock.elapsed))
        connected()
        assertTrue(model.setListen(ListenMode.LOCKED, clock.elapsed))
        clock.advance(AlertPolicy.LISTEN_MAX_MS)
        model.onTick(clock.elapsed)
        assertEquals(ListenMode.OFF, model.state.value.listen)
        assertEquals(1, effects.listenStopped)
    }

    @Test
    fun webBabyStatusIsUnknownNotAlarming() {
        connected()
        val web =
            uniffi.guardedsleep_core.DeviceStatus(
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                emptyList(),
                null,
                null,
                null,
                null,
                null,
            )
        model.onEvent(LinkEvent.PeerStatus(web), clock.elapsed)
        assertTrue(model.state.value.peer.isWebClient)
    }

    @Test
    fun stalePairingHintAfterTenMinutesOfAbsence() {
        st(LinkState.CONNECTING)
        st(LinkState.CONNECTING, 2, cause = "peer_absent")
        clock.advance(9 * 60_000)
        model.onTick(clock.elapsed)
        assertFalse(model.state.value.link.stalePairingHint)
        clock.advance(61_000)
        model.onTick(clock.elapsed)
        assertTrue(model.state.value.link.stalePairingHint)
        st(LinkState.CONNECTING, 2, cause = "server_unreachable")
        assertFalse(model.state.value.link.stalePairingHint)
    }

    @Test
    fun failedAfterConnectedReachesSinkAsWasConnected() {
        connected()
        st(LinkState.FAILED_NEEDS_USER)
        assertTrue(sink.snapshots.last().wasConnected)
    }
}
