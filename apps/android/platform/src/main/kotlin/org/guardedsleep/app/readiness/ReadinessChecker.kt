package org.guardedsleep.app.readiness

import android.annotation.SuppressLint
import android.app.NotificationManager
import android.content.Context
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.os.Build
import androidx.core.app.NotificationManagerCompat
import org.guardedsleep.app.alerts.AlertChannels
import org.guardedsleep.app.link.Clock
import org.guardedsleep.app.link.ReadinessProvider
import org.guardedsleep.app.settings.SettingsRepository
import org.guardedsleep.app.ui.model.ReadinessIssue

/** Facts about the parent phone, separated from the rules so the rules are testable on the JVM. */
interface ReadinessEnv {
    fun notificationsEnabled(): Boolean

    /** Alarm stream volume, 0 = off. */
    fun alarmVolume(): Int

    fun fullScreenAllowed(): Boolean

    /** Policy access granted and the alarm channels may bypass Do Not Disturb. */
    fun dndBypassAllowed(): Boolean

    fun batteryOptimizationIgnored(): Boolean

    fun ownBatteryPct(): Int?

    fun ownCharging(): Boolean

    fun headphonesConnected(): Boolean
}

/** Parent readiness check (alpha-ux-flows §3.17); worst items first. */
class ReadinessChecker(
    private val env: ReadinessEnv,
    private val settings: SettingsRepository,
    private val clock: Clock,
) : ReadinessProvider {
    override fun ownBatteryPct(): Int? = env.ownBatteryPct()

    override fun check(): List<ReadinessIssue> {
        val issues = mutableListOf<ReadinessIssue>()
        if (!env.notificationsEnabled()) issues += ReadinessIssue.NOTIFICATIONS_BLOCKED
        if (env.alarmVolume() <= 0) issues += ReadinessIssue.ALARM_VOLUME_ZERO
        if (!env.fullScreenAllowed()) issues += ReadinessIssue.FULL_SCREEN_BLOCKED
        if (!env.dndBypassAllowed()) issues += ReadinessIssue.DND_BYPASS_MISSING
        if (!env.batteryOptimizationIgnored()) issues += ReadinessIssue.BATTERY_OPTIMIZED
        val battery = env.ownBatteryPct()
        if (battery != null && battery < LOW_BATTERY_PCT && !env.ownCharging())
            issues += ReadinessIssue.PARENT_BATTERY_LOW
        if (env.headphonesConnected()) issues += ReadinessIssue.HEADPHONES_CONNECTED
        val lastTest = settings.current.lastTestWallMs
        if (lastTest == null || clock.wallMs() - lastTest > TEST_MAX_AGE_MS)
            issues += ReadinessIssue.NEVER_TESTED
        return issues
    }

    companion object {
        const val LOW_BATTERY_PCT = 20
        const val TEST_MAX_AGE_MS = 30L * 24 * 60 * 60 * 1000
        const val RECHECK_MS = 5L * 60_000L
    }
}

class AndroidReadinessEnv(private val context: Context) : ReadinessEnv {
    private val nm = context.getSystemService(NotificationManager::class.java)
    private val audio = context.getSystemService(AudioManager::class.java)

    override fun notificationsEnabled(): Boolean {
        if (!NotificationManagerCompat.from(context).areNotificationsEnabled()) return false
        return listOf(AlertChannels.ALARM_NOISE, AlertChannels.ALARM_LINK).all {
            (nm.getNotificationChannel(it)?.importance ?: NotificationManager.IMPORTANCE_HIGH) !=
                NotificationManager.IMPORTANCE_NONE
        }
    }

    override fun alarmVolume(): Int = audio?.getStreamVolume(AudioManager.STREAM_ALARM) ?: 0

    override fun fullScreenAllowed(): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.UPSIDE_DOWN_CAKE || nm.canUseFullScreenIntent()

    override fun dndBypassAllowed(): Boolean =
        nm.isNotificationPolicyAccessGranted &&
            listOf(AlertChannels.ALARM_NOISE, AlertChannels.ALARM_LINK).all {
                nm.getNotificationChannel(it)?.canBypassDnd() == true
            }

    private val battery = BatteryReader(context)

    override fun batteryOptimizationIgnored(): Boolean = battery.optimizationIgnored()

    override fun ownBatteryPct(): Int? = battery.pct()

    override fun ownCharging(): Boolean = battery.charging()

    override fun headphonesConnected(): Boolean =
        audio?.getDevices(AudioManager.GET_DEVICES_OUTPUTS)?.any { it.type in HEADPHONE_TYPES } ==
            true

    private companion object {
        // TYPE_BLE_HEADSET (API 31) is an inlined int: on older systems no device reports it.
        @SuppressLint("InlinedApi")
        val HEADPHONE_TYPES =
            setOf(
                AudioDeviceInfo.TYPE_WIRED_HEADSET,
                AudioDeviceInfo.TYPE_WIRED_HEADPHONES,
                AudioDeviceInfo.TYPE_BLUETOOTH_A2DP,
                AudioDeviceInfo.TYPE_USB_HEADSET,
                AudioDeviceInfo.TYPE_BLE_HEADSET,
            )
    }
}
