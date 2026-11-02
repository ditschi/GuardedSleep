package org.guardedsleep.app.silence

import android.app.AlarmManager
import android.app.AutomaticZenRule
import android.app.NotificationManager
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.media.AudioManager
import android.net.Uri
import android.os.BatteryManager
import android.os.Build
import android.service.notification.Condition
import android.service.notification.ZenPolicy
import androidx.annotation.RequiresApi
import androidx.core.net.toUri
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.diag.PocLog
import org.guardedsleep.app.locale.localizedContext
import org.guardedsleep.app.settings.SettingsRepository
import org.guardedsleep.app.settings.SilenceMarker
import org.guardedsleep.app.wiring.EntryPoints
import uniffi.guardedsleep_core.DeviceStatus
import uniffi.guardedsleep_core.Dnd
import uniffi.guardedsleep_core.Ringer
import uniffi.guardedsleep_core.UnitWarning

/** What the link needs from the baby phone's silence handling; faked in JVM tests. */
interface SilenceGateway {
    fun hasAccess(): Boolean

    /** Turns "alarms only, no visual effects" on. False without notification-policy access. */
    fun activate(): Boolean

    fun deactivate()

    /** At app start: undoes a rule a crash left behind. True when something was restored. */
    fun restoreLeftover(): Boolean

    fun readDeviceStatus(): DeviceStatus
}

/**
 * Baby-side Do Not Disturb (ADR-018 §3): an AutomaticZenRule whose ZenPolicy allows alarms only and
 * hides every visual effect; API 26-28 fall back to the global "alarms only" filter. The user's own
 * mode returns when the rule is switched off or removed.
 */
class SilenceController(
    private val context: Context,
    private val settings: SettingsRepository,
    private val entryPoints: EntryPoints,
) : SilenceGateway {
    private val nm = context.getSystemService(NotificationManager::class.java)

    override fun hasAccess(): Boolean = nm.isNotificationPolicyAccessGranted

    override fun activate(): Boolean {
        if (!hasAccess()) return false
        val st = settings.current.silence
        if (st.active) return true
        return runCatching {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                val id = nm.addAutomaticZenRule(rule())
                nm.setAutomaticZenRuleState(id, condition(Condition.STATE_TRUE))
                settings.update { it.copy(silence = SilenceMarker(active = true, ruleId = id)) }
            } else {
                val previous = nm.currentInterruptionFilter
                nm.setInterruptionFilter(NotificationManager.INTERRUPTION_FILTER_ALARMS)
                settings.update {
                    it.copy(silence = SilenceMarker(active = true, previousFilter = previous))
                }
            }
            PocLog.event("silence", "state" to "on")
            true
        }
            .getOrElse {
                PocLog.event("failed", "reason" to "silence_activate")
                false
            }
    }

    override fun deactivate() {
        val st = settings.current.silence
        if (!st.active) return
        runCatching {
            if (hasAccess()) {
                if (st.ruleId != null) nm.removeAutomaticZenRule(st.ruleId)
                st.previousFilter?.let { nm.setInterruptionFilter(it) }
            }
        }
        settings.update { it.copy(silence = SilenceMarker()) }
        PocLog.event("silence", "state" to "off")
    }

    override fun restoreLeftover(): Boolean {
        val marked = settings.current.silence.active
        if (marked) deactivate()
        // Also remove rules of ours that a crash left without a saved marker.
        val orphans = runCatching {
            if (!hasAccess()) 0
            else
                nm.automaticZenRules.entries
                    .filter { it.value.conditionId == CONDITION_ID }
                    .onEach { nm.removeAutomaticZenRule(it.key) }
                    .size
        }
            .getOrDefault(0)
        return marked || orphans > 0
    }

    override fun readDeviceStatus(): DeviceStatus {
        val audio = context.getSystemService(AudioManager::class.java)
        val ringer =
            when (audio?.ringerMode) {
                AudioManager.RINGER_MODE_NORMAL -> Ringer.NORMAL
                AudioManager.RINGER_MODE_VIBRATE -> Ringer.VIBRATE
                AudioManager.RINGER_MODE_SILENT -> Ringer.SILENT
                else -> Ringer.UNKNOWN
            }
        val dnd =
            when (nm.currentInterruptionFilter) {
                NotificationManager.INTERRUPTION_FILTER_ALL -> Dnd.OFF
                NotificationManager.INTERRUPTION_FILTER_PRIORITY -> Dnd.PRIORITY
                NotificationManager.INTERRUPTION_FILTER_ALARMS -> Dnd.ALARMS_ONLY
                NotificationManager.INTERRUPTION_FILTER_NONE -> Dnd.TOTAL
                else -> Dnd.UNKNOWN
            }
        val battery = context.registerReceiver(null, IntentFilter(Intent.ACTION_BATTERY_CHANGED))
        val level = battery?.getIntExtra(BatteryManager.EXTRA_LEVEL, -1) ?: -1
        val scale = battery?.getIntExtra(BatteryManager.EXTRA_SCALE, -1) ?: -1
        val status = battery?.getIntExtra(BatteryManager.EXTRA_STATUS, -1) ?: -1
        val pct = if (level >= 0 && scale > 0) (level * 100 / scale).coerceIn(0, 100) else null
        val nextAlarm =
            context.getSystemService(AlarmManager::class.java)?.nextAlarmClock?.triggerTime?.let {
                ((it - System.currentTimeMillis()) / 60_000L)
                    .coerceAtLeast(0)
                    .coerceAtMost(MAX_MINUTES)
                    .toUInt()
            }
        return DeviceStatus(
            batteryPct = pct?.toUByte(),
            charging =
                status
                    .takeIf { it >= 0 }
                    ?.let {
                        it == BatteryManager.BATTERY_STATUS_CHARGING ||
                            it == BatteryManager.BATTERY_STATUS_FULL
                    },
            ringer = ringer,
            dnd = dnd,
            alarmsAllowed = dnd != Dnd.TOTAL,
            dndAccess = hasAccess(),
            quietActive = settings.current.silence.active,
            nextAlarmMin = nextAlarm,
            micSilent = null,
            noiseThreshold = null,
            unitWarnings = warnings(status, battery),
        )
    }

    private fun warnings(batteryStatus: Int, battery: Intent?): List<UnitWarning> = buildList {
        val plugged = (battery?.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0) ?: 0) != 0
        if (battery != null && !plugged && batteryStatus >= 0) add(UnitWarning.NOT_CHARGING)
        if (
            context.checkSelfPermission(android.Manifest.permission.RECORD_AUDIO) !=
                android.content.pm.PackageManager.PERMISSION_GRANTED
        ) {
            add(UnitWarning.MIC_PERMISSION_LOST)
        }
    }

    @RequiresApi(Build.VERSION_CODES.Q)
    private fun rule(): AutomaticZenRule {
        val policy =
            ZenPolicy.Builder()
                .allowAlarms(true)
                .allowMedia(false)
                .allowSystem(false)
                .allowReminders(false)
                .allowEvents(false)
                .allowCalls(ZenPolicy.PEOPLE_TYPE_NONE)
                .allowMessages(ZenPolicy.PEOPLE_TYPE_NONE)
                .allowRepeatCallers(false)
                .hideAllVisualEffects()
                .build()
        return AutomaticZenRule(
            ruleName(),
            null,
            ComponentName(context, entryPoints.mainActivity),
            CONDITION_ID,
            policy,
            NotificationManager.INTERRUPTION_FILTER_PRIORITY,
            true,
        )
    }

    private fun ruleName() =
        localizedContext(context, settings.current.settings.language)
            .getString(R.string.native_notif_silence_rule_name)

    private fun condition(state: Int) = Condition(CONDITION_ID, ruleName(), state)

    private companion object {
        const val MAX_MINUTES = 1440L
        val CONDITION_ID: Uri = "condition://org.guardedsleep.app/monitoring".toUri()
    }
}
