package org.guardedsleep.app

import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.guardedsleep.app.readiness.ReadinessChecker
import org.guardedsleep.app.readiness.ReadinessEnv
import org.guardedsleep.app.settings.SettingsRepository
import org.guardedsleep.app.ui.model.ReadinessIssue
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

fun testSettings(dir: File, name: String = "s"): SettingsRepository {
    val scope = CoroutineScope(Dispatchers.Unconfined)
    return SettingsRepository(PreferenceDataStoreFactory.create(scope = scope) { File(dir, "$name.preferences_pb") }, scope)
}

class ReadinessTest {
    @get:Rule val tmp = TemporaryFolder()

    private class Env : ReadinessEnv {
        var notifications = true
        var volume = 5
        var fullScreen = true
        var bypass = true
        var batteryOpt = true
        var battery: Int? = 80
        var charging = false
        var headphones = false

        override fun notificationsEnabled() = notifications
        override fun alarmVolume() = volume
        override fun fullScreenAllowed() = fullScreen
        override fun dndBypassAllowed() = bypass
        override fun batteryOptimizationIgnored() = batteryOpt
        override fun ownBatteryPct() = battery
        override fun ownCharging() = charging
        override fun headphonesConnected() = headphones
    }

    @Test fun allGoodExceptNeverTested() {
        val clock = FakeClock()
        val settings = testSettings(tmp.root)
        val checker = ReadinessChecker(Env(), settings, clock)
        assertEquals(listOf(ReadinessIssue.NEVER_TESTED), checker.check())
        settings.update { it.copy(lastTestWallMs = clock.wall - 1000) }
        assertEquals(emptyList<ReadinessIssue>(), checker.check())
        clock.advance(31L * 24 * 3600 * 1000)
        assertEquals(listOf(ReadinessIssue.NEVER_TESTED), checker.check())
    }

    @Test fun everythingWrongIsReportedWorstFirst() {
        val env = Env().apply {
            notifications = false; volume = 0; fullScreen = false; bypass = false
            batteryOpt = false; battery = 12; headphones = true
        }
        val settings = testSettings(tmp.root)
        val issues = ReadinessChecker(env, settings, FakeClock()).check()
        assertEquals(
            listOf(
                ReadinessIssue.NOTIFICATIONS_BLOCKED, ReadinessIssue.ALARM_VOLUME_ZERO, ReadinessIssue.FULL_SCREEN_BLOCKED,
                ReadinessIssue.DND_BYPASS_MISSING, ReadinessIssue.BATTERY_OPTIMIZED, ReadinessIssue.PARENT_BATTERY_LOW,
                ReadinessIssue.HEADPHONES_CONNECTED, ReadinessIssue.NEVER_TESTED,
            ),
            issues,
        )
    }

    @Test fun lowBatteryWhileChargingIsFine() {
        val env = Env().apply { battery = 10; charging = true }
        val settings = testSettings(tmp.root)
        settings.update { it.copy(lastTestWallMs = Long.MAX_VALUE / 2) }
        assertEquals(emptyList<ReadinessIssue>(), ReadinessChecker(env, settings, FakeClock()).check())
    }
}
