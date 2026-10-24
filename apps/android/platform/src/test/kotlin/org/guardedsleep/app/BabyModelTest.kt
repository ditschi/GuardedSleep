package org.guardedsleep.app

import org.guardedsleep.app.link.BabyModel
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.guardedsleep_core.LinkEvent

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
    fun otherSettingsSurviveTheListenEvent() {
        model.setSettings(0.3f, 7)
        model.onEvent(LinkEvent.ListenChanged(true), clock.elapsed)
        assertEquals(0.3f, model.state.value.threshold, 0.0001f)
        assertEquals(7, model.state.value.holdSeconds)
    }
}
