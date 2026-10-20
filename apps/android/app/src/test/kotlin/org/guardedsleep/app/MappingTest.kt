package org.guardedsleep.app

import org.guardedsleep.app.alerts.FeedbackPlans
import org.guardedsleep.app.alerts.NoiseKind
import org.guardedsleep.app.alerts.NotifyLevel
import org.guardedsleep.app.config.AlertPolicy
import org.guardedsleep.app.link.LinkMapping
import org.guardedsleep.app.link.NoiseTracker
import org.guardedsleep.app.silence.SilenceMapping
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.LinkQuality
import org.guardedsleep.app.ui.model.SilenceStatus
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.guardedsleep_core.DeviceStatus
import uniffi.guardedsleep_core.Dnd
import uniffi.guardedsleep_core.Ringer

class MappingTest {
    private fun dev(ringer: Ringer? = Ringer.NORMAL, dnd: Dnd? = Dnd.OFF, access: Boolean? = true, quiet: Boolean? = false) =
        DeviceStatus(null, null, ringer, dnd, true, access, quiet, null, null, null, emptyList())

    @Test fun silenceMapping() {
        assertEquals(SilenceStatus.MAY_BUZZ, SilenceMapping.silence(dev(Ringer.VIBRATE)))
        assertEquals(SilenceStatus.MAY_RING, SilenceMapping.silence(dev(Ringer.NORMAL)))
        assertEquals(SilenceStatus.SILENT, SilenceMapping.silence(dev(Ringer.NORMAL, Dnd.ALARMS_ONLY, quiet = true)))
        assertEquals(SilenceStatus.SCREEN_MAY_LIGHT, SilenceMapping.silence(dev(Ringer.SILENT)))
        assertEquals(SilenceStatus.SCREEN_MAY_LIGHT, SilenceMapping.silence(dev(Ringer.NORMAL, Dnd.ALARMS_ONLY)))
        assertEquals(SilenceStatus.NO_ACCESS, SilenceMapping.silence(dev(Ringer.NORMAL, access = false)))
        assertEquals(SilenceStatus.UNKNOWN, SilenceMapping.silence(null))
    }

    @Test fun webBabyIsUnknown() {
        val web = dev(ringer = null, dnd = null, access = null, quiet = null)
        assertEquals(SilenceStatus.UNKNOWN, SilenceMapping.silence(web))
        assertTrue(SilenceMapping.peerStatus(web).isWebClient)
        assertFalse(SilenceMapping.peerStatus(dev()).isWebClient)
    }

    @Test fun linkLevelsToFeedback() {
        val p = LinkPhase.RECONNECTING
        assertTrue(FeedbackPlans.forLink(p, 0, false).silent)
        val l1 = FeedbackPlans.forLink(p, 1, false)
        assertEquals(NotifyLevel.LOW, l1.notify)
        assertTrue(l1.silent) // owner default: level 1 is visual only
        val l2 = FeedbackPlans.forLink(p, 2, false)
        assertTrue(l2.fullScreen)
        assertEquals(AlertPolicy.LEVEL2_HAPTIC_INTERVAL_MS, l2.hapticEveryMs)
        assertEquals(10_000L, l2.toneEveryMs)
        assertFalse(l2.continuousAlarm)
        val l3 = FeedbackPlans.forLink(p, 3, false)
        assertTrue(l3.continuousAlarm)
        val acked = FeedbackPlans.forLink(p, 3, true)
        assertTrue(acked.silent)
        assertTrue(acked.fullScreen)
        assertTrue(FeedbackPlans.forLink(LinkPhase.CONNECTED, 2, false).silent)
        assertTrue(FeedbackPlans.forLink(LinkPhase.PEER_STOPPED, 2, false).silent)
    }

    @Test fun noiseToFeedback() {
        assertTrue(FeedbackPlans.forNoise(NoiseKind.NEW, false).fullScreen)
        assertTrue(FeedbackPlans.forNoise(NoiseKind.NEW, false).repeatingHaptic)
        val cont = FeedbackPlans.forNoise(NoiseKind.CONTINUE, false)
        assertFalse(cont.fullScreen)
        assertTrue(cont.shortHaptic)
        assertFalse(FeedbackPlans.forNoise(NoiseKind.NEW, true).notify)
    }

    @Test fun noiseEventLifecycle() {
        val t = NoiseTracker()
        assertEquals(NoiseKind.NEW, t.onAlert(0))
        assertEquals(NoiseKind.CONTINUE, t.onAlert(10_000))
        t.onLevel(20_000, 0.5, 0.12)
        assertFalse(t.tick(20_000 + AlertPolicy.NOISE_QUIET_AFTER_MS - 1))
        assertTrue(t.tick(20_000 + AlertPolicy.NOISE_QUIET_AFTER_MS))
        val ended = 20_000 + AlertPolicy.NOISE_QUIET_AFTER_MS
        assertEquals(NoiseKind.CONTINUE, t.onAlert(ended + 60_000))
        assertTrue(t.tick(ended + 60_000 + AlertPolicy.NOISE_QUIET_AFTER_MS))
        val ended2 = ended + 60_000 + AlertPolicy.NOISE_QUIET_AFTER_MS
        assertEquals(NoiseKind.NEW, t.onAlert(ended2 + AlertPolicy.NOISE_EVENT_JOIN_MS))
    }

    @Test fun linkQuality() {
        assertNull(LinkMapping.quality(5, 0))
        assertEquals(LinkQuality.GOOD, LinkMapping.quality(250, 2))
        assertEquals(LinkQuality.MEDIUM, LinkMapping.quality(250, 12))
        assertEquals(LinkQuality.POOR, LinkMapping.quality(100, 20))
    }

    @Test fun causeHints() {
        assertEquals(org.guardedsleep.app.ui.model.CauseHint.PEER_ABSENT, LinkMapping.causeHint("peer_absent"))
        assertNull(LinkMapping.causeHint("nonsense"))
    }

    @Test fun unitWarningsUseWireNames() {
        val s = dev().copy(unitWarnings = listOf(uniffi.guardedsleep_core.UnitWarning.NOT_CHARGING, uniffi.guardedsleep_core.UnitWarning.MIC_PERMISSION_LOST))
        assertEquals(listOf("not_charging", "mic_permission_lost"), SilenceMapping.peerStatus(s).unitWarnings)
    }

    @Test fun lastAliveComesFromTheCoreAndUpdateRequiredIsSticky() {
        val ui = LinkMapping.linkUi(status(uniffi.guardedsleep_core.LinkState.RECONNECTING, 1, cause = "update_required", lastAlive = 4_200), LinkQuality.UNKNOWN)
        assertEquals(4_200L, ui.lastAliveElapsedMs)
        assertEquals("update_required", ui.failureKey)
        assertNull(LinkMapping.causeHint("update_required"))
    }

    @Test fun failedAfterConnectedIsNeverSilent() {
        assertTrue(FeedbackPlans.forLink(LinkPhase.FAILED_NEEDS_USER, 0, false, wasConnected = false).silent)
        val plan = FeedbackPlans.forLink(LinkPhase.FAILED_NEEDS_USER, 0, false, wasConnected = true)
        assertTrue(plan.fullScreen)
        assertEquals(AlertPolicy.LEVEL2_HAPTIC_INTERVAL_MS, plan.hapticEveryMs)
        assertTrue(FeedbackPlans.forLink(LinkPhase.FAILED_NEEDS_USER, 0, true, wasConnected = true).silent)
    }
}
