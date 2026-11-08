package org.guardedsleep.app

import org.guardedsleep.app.alerts.FeedbackPlans
import org.guardedsleep.app.alerts.NoiseKind
import org.guardedsleep.app.config.AlertPolicy
import org.guardedsleep.app.link.ParentModel
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.ListenMode
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.guardedsleep_core.LinkEvent
import uniffi.guardedsleep_core.LinkState

/**
 * Alert style "Discreet": noise vibrates and shows, but plays no audio until the parent listens.
 */
class DiscreetAlertTest {
    private val clock = FakeClock()
    private val sink = RecordingSink()
    private val effects = RecordingEffects()
    private var discreet = true
    private val model = ParentModel(clock, sink, effects, discreet = { discreet }) { 0.12 }

    private fun connect() {
        model.onStatus(status(LinkState.CONNECTING), clock.elapsed)
        clock.advance(500)
        model.onStatus(status(LinkState.CONNECTED), clock.elapsed)
    }

    private fun noise(snoozed: Boolean = false, probe: Boolean = false) =
        model.onEvent(LinkEvent.NoiseAlert(0.4, probe, snoozed), clock.elapsed)

    @Test
    fun planKeepsVibrationAndScreenButWithholdsAudio() {
        val sound = FeedbackPlans.forNoise(NoiseKind.NEW, snoozed = false, discreet = false)
        val quiet = FeedbackPlans.forNoise(NoiseKind.NEW, snoozed = false, discreet = true)
        assertTrue(sound.playAudio)
        assertFalse(quiet.playAudio)
        assertEquals(sound.copy(playAudio = false), quiet)
        val cont = FeedbackPlans.forNoise(NoiseKind.CONTINUE, snoozed = false, discreet = true)
        assertTrue(cont.notify && cont.shortHaptic && !cont.fullScreen && !cont.playAudio)
        // Snooze still silences everything, in either style.
        assertFalse(FeedbackPlans.forNoise(NoiseKind.NEW, snoozed = true, discreet = true).notify)
        assertFalse(FeedbackPlans.forNoise(NoiseKind.NEW, snoozed = true).playAudio)
    }

    @Test
    fun discreetNoiseAlertsButPlaysNothingUntilListen() {
        connect()
        noise()
        assertEquals(NoiseKind.NEW, sink.noise.last().first)
        assertEquals(false, sink.noise.last().third)
        assertTrue(model.state.value.noiseAlertActive)
        assertEquals(true, effects.muted.last())

        assertTrue(model.setListen(ListenMode.HOLD, clock.elapsed))
        assertEquals(false, effects.muted.last())
        // A continuing event while listening stays audible.
        clock.advance(10_000)
        noise()
        assertEquals(false, effects.muted.last())
        // Ending Listen during the running event mutes again.
        assertTrue(model.setListen(ListenMode.OFF, clock.elapsed))
        assertEquals(true, effects.muted.last())
    }

    @Test
    fun soundStyleStillPlaysAudio() {
        discreet = false
        connect()
        noise()
        assertEquals(false, effects.muted.last())
        assertTrue(model.setListen(ListenMode.HOLD, clock.elapsed))
        assertTrue(model.setListen(ListenMode.OFF, clock.elapsed))
        assertEquals(false, effects.muted.last())
    }

    @Test
    fun switchingToDiscreetMutesARunningEventUnlessListening() {
        discreet = false
        connect()
        noise()
        discreet = true
        model.onAlertStyleChanged()
        assertEquals(true, effects.muted.last())
        assertTrue(model.setListen(ListenMode.LOCKED, clock.elapsed))
        model.onAlertStyleChanged()
        assertEquals(false, effects.muted.last())
    }

    @Test
    fun snoozeEndingDuringNoiseAlertsFullyButStaysSilentWhenDiscreet() {
        connect()
        model.snooze(10)
        noise(snoozed = true)
        assertEquals(true, effects.muted.last())
        clock.advance(10 * 60_000L)
        noise(snoozed = true)
        model.onTick(clock.elapsed)
        assertEquals(1, effects.snoozeEnded)
        assertEquals(NoiseKind.NEW, sink.noise.last().first)
        assertEquals(false, sink.noise.last().third)
        assertEquals(true, effects.muted.last())
    }

    @Test
    fun testAlertRunsTheActiveStyle() {
        connect()
        noise(probe = true)
        assertEquals(1, effects.tests)
        assertTrue(sink.noise.last().second)
        assertTrue(model.state.value.noiseIsTest)
        assertEquals(true, effects.muted.last())
    }

    @Test
    fun connectionLevelsAreTheSameInBothStyles() {
        // The style never reaches the link feedback plan: level 3 stays an audible alarm.
        val l3 = FeedbackPlans.forLink(LinkPhase.RECONNECTING, 3, acknowledged = false)
        assertTrue(l3.continuousAlarm && l3.fullScreen)
        model.onStatus(status(LinkState.RECONNECTING, level = 3), clock.elapsed)
        assertEquals(3, sink.snapshots.last().level)
        assertEquals(LinkPhase.RECONNECTING, sink.snapshots.last().phase)
        assertEquals(0, effects.muted.size)
    }

    @Test
    fun noiseEndsAfterTheQuietRuleInDiscreet() {
        connect()
        noise()
        clock.advance(AlertPolicy.NOISE_QUIET_AFTER_MS)
        model.onTick(clock.elapsed)
        assertFalse(model.state.value.noiseAlertActive)
        assertEquals(1, sink.ended)
    }
}
