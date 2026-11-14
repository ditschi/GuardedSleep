package org.guardedsleep.app.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import org.guardedsleep.app.ui.common.BatteryReading
import org.guardedsleep.app.ui.common.KioskAttention
import org.guardedsleep.app.ui.common.KioskMarkShape
import org.guardedsleep.app.ui.common.KioskMinBrightness
import org.guardedsleep.app.ui.common.KioskWakeBrightness
import org.guardedsleep.app.ui.common.SetupConfirmSeconds
import org.guardedsleep.app.ui.common.kioskAttention
import org.guardedsleep.app.ui.common.kioskBrightness
import org.guardedsleep.app.ui.common.kioskMarkShape
import org.guardedsleep.app.ui.common.setupConfirmRemaining
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.theme.KioskMarks
import org.guardedsleep.app.ui.theme.StateColors
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** The pure rules of the baby kiosk (`connection-status` and `device-setup` "Baby kiosk"). */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class KioskLogicTest {
    private fun contrast(a: Color, b: Color): Double {
        val la = a.luminance().toDouble()
        val lb = b.luminance().toDouble()
        return (maxOf(la, lb) + 0.05) / (minOf(la, lb) + 0.05)
    }

    @Test
    fun noKioskMarkIsBrighterThanTheCap() {
        assertEquals(Color(0xFF5A1414), KioskMarks.Cap)
        for (c in KioskMarks.all) {
            assertTrue("$c is brighter than the cap", c.luminance() <= KioskMarks.Cap.luminance())
        }
    }

    @Test
    fun theStatusLineIsReadableOnBlackAtTheKioskBrightness() {
        assertTrue(contrast(StateColors.KioskText, StateColors.KioskBlack) >= 4.5)
    }

    @Test
    fun theMarkTellsTheStateByShape() {
        fun shape(phase: LinkPhase, level: Int = 0) = kioskMarkShape(link(phase, level))
        assertEquals(KioskMarkShape.BAR, shape(LinkPhase.CONNECTED))
        assertEquals(KioskMarkShape.NONE, shape(LinkPhase.CONNECTING))
        assertEquals(KioskMarkShape.NONE, shape(LinkPhase.RECONNECTING, 0))
        assertEquals(KioskMarkShape.DASHED, shape(LinkPhase.RECONNECTING, 1))
        assertEquals(KioskMarkShape.FRAME, shape(LinkPhase.RECONNECTING, 2))
        assertEquals(KioskMarkShape.FRAME, shape(LinkPhase.RECONNECTING, 3))
        assertEquals(KioskMarkShape.FRAME, shape(LinkPhase.FAILED_NEEDS_USER))
        assertEquals(KioskMarkShape.NONE, shape(LinkPhase.PEER_STOPPED))
    }

    @Test
    fun needsAttentionForALowBatteryOrAPulledCharger() {
        assertNull(kioskAttention(null, true))
        assertNull(kioskAttention(BatteryReading(64, true), true))
        assertNull(kioskAttention(BatteryReading(64, false), false))
        assertEquals(KioskAttention.NOT_CHARGING, kioskAttention(BatteryReading(64, false), true))
        assertEquals(KioskAttention.BATTERY_LOW, kioskAttention(BatteryReading(12, false), false))
        assertEquals(
            KioskAttention.BATTERY_CRITICAL,
            kioskAttention(BatteryReading(4, false), true),
        )
        assertNull(kioskAttention(BatteryReading(4, true), true))
    }

    @Test
    fun theConfirmationRunsForSixtySecondsFromTheFirstContact() {
        assertNull(setupConfirmRemaining(null, 100_000))
        assertEquals(SetupConfirmSeconds, setupConfirmRemaining(100_000, 100_000))
        assertEquals(45, setupConfirmRemaining(100_000, 115_000))
        assertEquals(1, setupConfirmRemaining(100_000, 159_000))
        assertNull(setupConfirmRemaining(100_000, 160_000))
        assertNull(setupConfirmRemaining(100_000, 400_000))
    }

    @Test
    fun brightnessIsTheMinimumUnlessAwakeAndNotTalking() {
        assertEquals(KioskMinBrightness, kioskBrightness(awake = false, talking = false), 0f)
        assertEquals(KioskWakeBrightness, kioskBrightness(awake = true, talking = false), 0f)
        assertEquals(KioskMinBrightness, kioskBrightness(awake = true, talking = true), 0f)
        assertTrue(KioskWakeBrightness <= 0.1f)
    }
}
