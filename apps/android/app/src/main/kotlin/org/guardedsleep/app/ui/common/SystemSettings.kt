package org.guardedsleep.app.ui.common

import android.app.NotificationManager
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.provider.Settings
import androidx.core.app.NotificationManagerCompat

/** Intents into system settings, built here so the checklist rows stay a pure UI concern. */
object SystemSettings {
    private fun pkgUri(ctx: Context) = Uri.parse("package:" + ctx.packageName)

    fun appDetails(ctx: Context) =
        Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, pkgUri(ctx))

    fun notificationSettings(ctx: Context) =
        Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, ctx.packageName)

    fun fullScreenIntentSettings(ctx: Context): Intent =
        if (Build.VERSION.SDK_INT >= 34) {
            Intent(Settings.ACTION_MANAGE_APP_USE_FULL_SCREEN_INTENT, pkgUri(ctx))
        } else {
            notificationSettings(ctx)
        }

    fun dndAccess() = Intent(Settings.ACTION_NOTIFICATION_POLICY_ACCESS_SETTINGS)

    fun batteryOptimization(ctx: Context) =
        Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, pkgUri(ctx))

    /** Starts [intent]; falls back to the app's details page when no activity handles it. */
    fun launch(ctx: Context, intent: Intent) {
        val i = Intent(intent).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        try {
            ctx.startActivity(i)
        } catch (_: ActivityNotFoundException) {
            runCatching { ctx.startActivity(appDetails(ctx).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
        } catch (_: SecurityException) {
            runCatching { ctx.startActivity(Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
        }
    }

    fun notificationsEnabled(ctx: Context) = NotificationManagerCompat.from(ctx).areNotificationsEnabled()

    fun fullScreenAllowed(ctx: Context): Boolean =
        Build.VERSION.SDK_INT < 34 ||
            (ctx.getSystemService(NotificationManager::class.java)?.canUseFullScreenIntent() ?: true)

    fun dndAccessGranted(ctx: Context) =
        ctx.getSystemService(NotificationManager::class.java)?.isNotificationPolicyAccessGranted ?: false

    fun batteryUnrestricted(ctx: Context) =
        ctx.getSystemService(PowerManager::class.java)?.isIgnoringBatteryOptimizations(ctx.packageName) ?: false
}
