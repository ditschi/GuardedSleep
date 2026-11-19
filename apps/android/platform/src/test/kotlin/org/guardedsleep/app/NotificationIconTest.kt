package org.guardedsleep.app

import android.app.Application
import android.app.Notification
import android.app.NotificationManager
import androidx.test.core.app.ApplicationProvider
import org.guardedsleep.app.alerts.AlertController
import org.guardedsleep.app.alerts.NoiseKind
import org.guardedsleep.app.alerts.StatusNotifier
import org.guardedsleep.app.platform.R
import org.guardedsleep.app.ui.model.LinkUi
import org.guardedsleep.app.ui.model.Role
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

/**
 * The small icon of a notification follows the role the device runs as: the crescent for the baby
 * device, the crescent with listening bars for the parent device (`adopt-night-crescent-logo`),
 * never a system icon.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class NotificationIconTest {
    private val app: Application = ApplicationProvider.getApplicationContext()

    private fun iconOf(n: Notification) = n.smallIcon.resId

    private fun posted(): List<Notification> =
        shadowOf(app.getSystemService(NotificationManager::class.java)).allNotifications

    @Test
    fun babyMonitoringShowsTheCrescent() {
        val n = StatusNotifier(app, FakeEntryPoints) { null }.build(Role.BABY, LinkUi())
        assertEquals(R.drawable.ic_stat_baby, iconOf(n))
    }

    @Test
    fun babyResumePromptShowsTheCrescent() {
        val n = StatusNotifier(app, FakeEntryPoints) { null }.resumePrompt()
        assertEquals(R.drawable.ic_stat_baby, iconOf(n))
    }

    @Test
    fun parentStatusShowsTheCrescentWithBars() {
        val n = StatusNotifier(app, FakeEntryPoints) { null }.build(Role.PARENT, LinkUi())
        assertEquals(R.drawable.ic_stat_parent, iconOf(n))
    }

    @Test
    fun parentNoiseAlertShowsTheCrescentWithBars() {
        AlertController(app, FakeEntryPoints, role = { Role.PARENT })
            .noiseAlert(NoiseKind.NEW, probe = false, snoozed = false)
        assertEquals(R.drawable.ic_stat_parent, iconOf(posted().single()))
    }

    @Test
    fun parentLinkLossAlertShowsTheCrescentWithBars() {
        AlertController(app, FakeEntryPoints, role = { Role.PARENT }).linkLost(downMs = 60_000)
        val all = posted()
        assertTrue(all.isNotEmpty())
        all.forEach { assertEquals(R.drawable.ic_stat_parent, iconOf(it)) }
    }

    @Test
    fun babyNoiseAlertShowsTheCrescent() {
        AlertController(app, FakeEntryPoints, role = { Role.BABY })
            .noiseAlert(NoiseKind.NEW, probe = false, snoozed = false)
        assertEquals(R.drawable.ic_stat_baby, iconOf(posted().single()))
    }

    @Test
    fun noRoleFallsBackToTheBabyCrescent() {
        AlertController(app, FakeEntryPoints)
            .noiseAlert(NoiseKind.NEW, probe = false, snoozed = false)
        assertEquals(R.drawable.ic_stat_baby, iconOf(posted().single()))
    }
}
