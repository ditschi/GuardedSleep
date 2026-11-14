package org.guardedsleep.app.readiness

import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.BatteryManager
import android.os.PowerManager

/** The battery as one read saw it; [pct] is null when the system gives none. */
data class BatterySnapshot(val pct: Int?, val charging: Boolean, val unhealthy: Boolean)

/** Battery and power-saving facts of this phone, shared by the readiness and device checks. */
internal class BatteryReader(private val context: Context) {
    /** The sticky battery broadcast; null when the system gives none. */
    private fun intent(): Intent? =
        context.registerReceiver(null, IntentFilter(Intent.ACTION_BATTERY_CHANGED))

    /** One read of the sticky broadcast: percentage, charging and health from the same instant. */
    fun snapshot(): BatterySnapshot {
        val i = intent() ?: return BatterySnapshot(null, false, false)
        val level = i.getIntExtra(BatteryManager.EXTRA_LEVEL, -1)
        val scale = i.getIntExtra(BatteryManager.EXTRA_SCALE, -1)
        val health = i.getIntExtra(BatteryManager.EXTRA_HEALTH, BatteryManager.BATTERY_HEALTH_GOOD)
        return BatterySnapshot(
            pct = if (level >= 0 && scale > 0) level * 100 / scale else null,
            charging = i.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0) != 0,
            // Dead, overheated, over-voltage or failing: the phone should not run all night.
            unhealthy =
                health == BatteryManager.BATTERY_HEALTH_DEAD ||
                    health == BatteryManager.BATTERY_HEALTH_OVERHEAT ||
                    health == BatteryManager.BATTERY_HEALTH_OVER_VOLTAGE ||
                    health == BatteryManager.BATTERY_HEALTH_UNSPECIFIED_FAILURE,
        )
    }

    fun pct(): Int? = snapshot().pct

    fun charging(): Boolean = snapshot().charging

    fun optimizationIgnored(): Boolean =
        context
            .getSystemService(PowerManager::class.java)
            ?.isIgnoringBatteryOptimizations(context.packageName) == true
}
