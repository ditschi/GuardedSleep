package org.guardedsleep.app

import org.guardedsleep.app.link.BabyModel
import org.guardedsleep.app.ui.model.LinkHint
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.guardedsleep_core.LinkEvent
import uniffi.guardedsleep_core.LinkState

class BabyModelTest {
    private val clock = FakeClock()
    private val model = BabyModel(clock)

    @Test
    fun theParentsListenIntentReachesTheBabyScreenAndIsReset() {
        assertFalse(model.state.value.parentListening)
        model.onEvent(LinkEvent.ListenChanged(true), clock.elapsed)
        assertTrue(model.state.value.parentListening)
        model.onEvent(LinkEvent.ListenChanged(false), clock.elapsed)
        assertFalse(model.state.value.parentListening)
    }

    @Test
    fun theParentsNameShowsOnlyWhileConnected() {
        model.onEvent(LinkEvent.ParentConnected("Pixel 8"), clock.elapsed)
        assertNull(model.state.value.parentName)
        model.onStatus(status(LinkState.CONNECTED), clock.elapsed)
        assertEquals("Pixel 8", model.state.value.parentName)
        model.onStatus(status(LinkState.RECONNECTING), clock.elapsed)
        assertNull(model.state.value.parentName)
        model.reset()
        model.onStatus(status(LinkState.CONNECTED), clock.elapsed)
        assertNull(model.state.value.parentName)
    }

    @Test
    fun theFirstContactStartsOnceAndResetsWithTheRun() {
        assertNull(model.state.value.firstContactElapsedMs)
        model.onEvent(LinkEvent.ParentConnected("Pixel 8"), 5_000)
        model.onEvent(LinkEvent.ParentConnected("Pixel 8"), 9_000)
        assertEquals(5_000L, model.state.value.firstContactElapsedMs)
        model.reset()
        assertNull(model.state.value.firstContactElapsedMs)
    }

    @Test
    fun theOwnBatteryAndTheChargerSeenFollowThePlatformReadings() {
        assertNull(model.state.value.ownBatteryPct)
        assertFalse(model.state.value.chargerSeen)
        // Unplugged from the start: no charger was seen, so an unplug is not news.
        model.onOwnBattery(64, false)
        assertEquals(64, model.state.value.ownBatteryPct)
        assertFalse(model.state.value.ownPlugged)
        assertFalse(model.state.value.chargerSeen)
        model.onOwnBattery(65, true)
        assertTrue(model.state.value.ownPlugged)
        assertTrue(model.state.value.chargerSeen)
        // Pulled out: the charger stays seen, so the kiosk can say it is no longer charging.
        model.onOwnBattery(64, false)
        assertFalse(model.state.value.ownPlugged)
        assertTrue(model.state.value.chargerSeen)
        // Other state changes keep it.
        model.onStatus(status(LinkState.CONNECTED), clock.elapsed)
        assertEquals(64, model.state.value.ownBatteryPct)
        assertTrue(model.state.value.chargerSeen)
    }

    @Test
    fun aNewRunClearsTheOwnBattery() {
        model.onOwnBattery(80, true)
        model.reset()
        assertNull(model.state.value.ownBatteryPct)
        assertFalse(model.state.value.ownPlugged)
        assertFalse(model.state.value.chargerSeen)
    }

    @Test
    fun anUnknownParentAttemptSetsTheWarningUntilReset() {
        assertFalse(model.state.value.securityWarning)
        model.onEvent(LinkEvent.SecurityWarning("unknown_parent_attempt"), clock.elapsed)
        assertTrue(model.state.value.securityWarning)
        model.reset()
        assertFalse(model.state.value.securityWarning)
    }

    @Test
    fun calmHintsComeAndGoAndUnknownOnesAreIgnored() {
        model.onEvent(LinkEvent.Hint("clock_skew", true), clock.elapsed)
        model.onEvent(LinkEvent.Hint("something_new", true), clock.elapsed)
        assertEquals(setOf(LinkHint.CLOCK_SKEW), model.state.value.link.hints)
        model.onStatus(status(LinkState.CONNECTED), clock.elapsed)
        assertEquals(setOf(LinkHint.CLOCK_SKEW), model.state.value.link.hints)
        model.onEvent(LinkEvent.Hint("clock_skew", false), clock.elapsed)
        assertTrue(model.state.value.link.hints.isEmpty())
    }

    @Test
    fun otherSettingsSurviveTheListenEvent() {
        model.setSettings(0.3f, 7)
        model.onEvent(LinkEvent.ListenChanged(true), clock.elapsed)
        assertEquals(0.3f, model.state.value.threshold, 0.0001f)
        assertEquals(7, model.state.value.holdSeconds)
    }
}
