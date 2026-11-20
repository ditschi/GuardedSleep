package org.guardedsleep.app

import android.app.Application
import android.app.Notification
import android.app.NotificationManager
import androidx.test.core.app.ApplicationProvider
import org.guardedsleep.app.alerts.AlertController
import org.guardedsleep.app.alerts.NoiseKind
import org.guardedsleep.app.ui.model.Role
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

/**
 * A test alert runs the real alert chain (`parent-alerts` "Test alert"): the same channel and the
 * same full-screen path as a real noise alert; only the copy differs.
 */
@RunWith(RobolectricTestRunner::class)
// API 33: before Android 14's full-screen-intent permission, so the path is open without a grant.
@Config(sdk = [33])
class TestAlertTest {
    private val app: Application = ApplicationProvider.getApplicationContext()
    private val manager = app.getSystemService(NotificationManager::class.java)
    private val nm = shadowOf(manager)

    private fun raise(probe: Boolean): Notification {
        manager.cancelAll()
        AlertController(app, FakeEntryPoints, role = { Role.PARENT })
            .noiseAlert(NoiseKind.NEW, probe = probe, snoozed = false)
        return nm.allNotifications.single()
    }

    @Test
    fun theProbeUsesTheRealChannelAndFullScreenPath() {
        val real = raise(probe = false)
        val test = raise(probe = true)
        assertEquals(real.channelId, test.channelId)
        assertNotNull(real.fullScreenIntent)
        assertNotNull(test.fullScreenIntent)
        assertEquals(real.category, test.category)
    }
}
