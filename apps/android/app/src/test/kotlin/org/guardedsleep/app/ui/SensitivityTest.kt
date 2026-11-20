package org.guardedsleep.app.ui

import org.guardedsleep.app.ui.model.Sensitivity
import org.guardedsleep.app.ui.model.TalkVolumeSteps
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SensitivityTest {
    private fun dbfs(threshold: Float) = 20.0 * Math.log10(threshold.toDouble())

    @Test
    fun theEndsAreMinusFiftyFiveAndMinusFifteenDbfs() {
        assertEquals(-55.0, dbfs(Sensitivity.thresholdOf(1f)), 1e-3)
        assertEquals(-15.0, dbfs(Sensitivity.thresholdOf(0f)), 1e-3)
    }

    @Test
    fun theMiddleIsMinusThirtyFiveDbfsNotAHalfOfTheLinearRange() {
        assertEquals(-35.0, dbfs(Sensitivity.thresholdOf(0.5f)), 1e-3)
        assertEquals(-35.0, Sensitivity.dbfsOf(0.5f), 1e-9)
    }

    @Test
    fun positionAndThresholdRoundTrip() {
        for (i in 0..20) {
            val p = i / 20f
            assertEquals(p, Sensitivity.positionOf(Sensitivity.thresholdOf(p)), 1e-4f)
        }
    }

    @Test
    fun aValueOutsideTheRangeIsClamped() {
        assertEquals(1f, Sensitivity.positionOf(0.0001f), 0f)
        assertEquals(0f, Sensitivity.positionOf(0.9f), 0f)
    }

    @Test
    fun theDefaultIsAboutMinusThirtySixDbfs() {
        assertEquals(-36.0, dbfs(Sensitivity.DEFAULT_THRESHOLD), 0.1)
    }

    @Test
    fun theMostSensitiveThresholdIsAboveTheCoresLowestLimit() {
        // 0.0015 is the core's shared limit (`min_noise_threshold()`); the platform clamps with it.
        assertTrue(Sensitivity.thresholdOf(1f) >= 0.0015f)
    }

    @Test
    fun theMeterAndTheThresholdShareOneScale() {
        assertEquals(0f, Sensitivity.meterFraction(0.0001f), 0f)
        assertEquals(1f, Sensitivity.meterFraction(1f), 0f)
        // A more sensitive setting (higher position) is a lower threshold, so a shorter tick.
        assertTrue(
            Sensitivity.meterFraction(Sensitivity.thresholdOf(0.7f)) <
                Sensitivity.meterFraction(Sensitivity.thresholdOf(0.3f))
        )
    }

    @Test
    fun theParentMetersPutTheTickWhereTheSliderDoes() {
        // The parent's SoundMeter and SoundStrip draw `meterFraction` of the level and of the
        // threshold: -35 dBFS is the middle, equal slider steps are equal tick steps, and a level
        // at the threshold reaches the tick.
        assertEquals(
            0.5f,
            Sensitivity.meterFraction(Math.pow(10.0, -35.0 / 20.0).toFloat()),
            0.001f,
        )
        val tick = { p: Float -> Sensitivity.meterFraction(Sensitivity.thresholdOf(p)) }
        assertEquals(tick(0.5f) - tick(1f), tick(0f) - tick(0.5f), 0.01f)
        val t = Sensitivity.thresholdOf(0.4f)
        assertEquals(Sensitivity.meterFraction(t), Sensitivity.meterFraction(t), 0f)
        assertTrue(Sensitivity.meterFraction(t * 1.5f) > Sensitivity.meterFraction(t))
    }

    @Test
    fun theTalkVolumeSnapsToTenToSeventyInTens() {
        assertEquals(10, TalkVolumeSteps.snap(0))
        assertEquals(50, TalkVolumeSteps.snap(52))
        assertEquals(60, TalkVolumeSteps.snap(55))
        assertEquals(70, TalkVolumeSteps.snap(100))
        assertEquals(TalkVolumeSteps.DEFAULT_PCT, 50)
    }
}
