package org.guardedsleep.app.ui

import org.guardedsleep.app.ui.screens.qrBrightness
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class QrBrightnessTest {
    @Test
    fun noOverrideUntilTheCodeIsMadeBrighter() {
        // null leaves the window at BRIGHTNESS_OVERRIDE_NONE (WindowBrightness does not touch it).
        assertNull(qrBrightness(bright = false))
        assertEquals(0.9f, qrBrightness(bright = true)!!, 0f)
    }
}
