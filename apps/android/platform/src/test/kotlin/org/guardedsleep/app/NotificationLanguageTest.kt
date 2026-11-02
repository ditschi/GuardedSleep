package org.guardedsleep.app

import android.app.Activity
import android.app.Application
import android.app.Notification
import android.app.NotificationManager
import android.content.BroadcastReceiver
import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.guardedsleep.app.alerts.AlertChannels
import org.guardedsleep.app.alerts.AlertController
import org.guardedsleep.app.alerts.NoiseKind
import org.guardedsleep.app.alerts.StatusNotifier
import org.guardedsleep.app.locale.localizedContext
import org.guardedsleep.app.ui.model.LinkUi
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.wiring.EntryPoints
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertSame
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

/**
 * Notifications, channel names and the foreground-service text follow the stored app language, also
 * on Android 8 to 12 where the per-app language only reaches the activity (`localizedContext`).
 */
@RunWith(RobolectricTestRunner::class)
class NotificationLanguageTest {
    private val app: Application = ApplicationProvider.getApplicationContext()
    private val entryPoints =
        object : EntryPoints {
            override val mainActivity = Activity::class.java
            override val alertActivity = Activity::class.java
            override val alertActionReceiver = BroadcastReceiver::class.java
        }

    private fun title(n: Notification) = n.extras.getString(Notification.EXTRA_TITLE)

    private fun text(n: Notification) = n.extras.getString(Notification.EXTRA_TEXT)

    private fun postedNoiseAlarm(language: String?): Notification {
        AlertController(app, entryPoints) { language }
            .noiseAlert(NoiseKind.NEW, probe = false, snoozed = false)
        val nm = app.getSystemService(NotificationManager::class.java)
        return shadowOf(nm).allNotifications.single()
    }

    @Test
    @Config(sdk = [30], qualifiers = "en")
    fun api30StoredGermanDrivesAlarmServiceAndChannels() {
        assertEquals("Geräusch erkannt", title(postedNoiseAlarm("de")))

        val service = StatusNotifier(app, entryPoints) { "de" }.build(Role.BABY, LinkUi())
        assertEquals("GuardedSleep überwacht", text(service))

        val nm = app.getSystemService(NotificationManager::class.java)
        val german = nm.getNotificationChannel(AlertChannels.ALARM_NOISE).name.toString()
        // A later language change renames the channel.
        AlertChannels.ensure(app, null)
        val english = nm.getNotificationChannel(AlertChannels.ALARM_NOISE).name.toString()
        assertNotEquals(german, english)
    }

    @Test
    @Config(sdk = [30], qualifiers = "en")
    fun api30WithoutStoredLanguageFollowsTheSystem() {
        assertEquals("Sound detected", title(postedNoiseAlarm(null)))
    }

    @Test
    @Config(sdk = [34], qualifiers = "de")
    fun api34UsesTheNormalContextBecauseLocaleManagerApplies() {
        val context: Context = app
        assertSame(context, localizedContext(context, "de"))
        assertEquals("Geräusch erkannt", title(postedNoiseAlarm("de")))
        val service = StatusNotifier(app, entryPoints) { "de" }.build(Role.BABY, LinkUi())
        assertEquals("GuardedSleep überwacht", text(service))
    }
}
