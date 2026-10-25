package org.guardedsleep.app.alerts

import android.app.Notification
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationCompat
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.locale.localizedContext
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.LinkUi
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.wiring.EntryPoints

/** The foreground-service notification: ongoing, silent, text follows the link state. */
class StatusNotifier(
    private val baseContext: Context,
    private val entryPoints: EntryPoints,
    /** The stored app language (`SettingsState.language`), read when the notification is built. */
    private val language: () -> String?,
) {
    private var channelsLanguage: String? = language()

    init {
        AlertChannels.ensure(baseContext, channelsLanguage)
    }

    /** Resources in the stored app language (also on Android 8 to 12, see [localizedContext]). */
    private fun localized(): Context {
        val tag = language()
        if (tag != channelsLanguage) {
            channelsLanguage = tag
            AlertChannels.ensure(baseContext, tag)
        }
        return localizedContext(baseContext, tag)
    }

    fun build(role: Role, link: LinkUi): Notification {
        val context = localized()
        val open =
            PendingIntent.getActivity(
                context,
                0,
                Intent(context, entryPoints.mainActivity),
                PendingIntent.FLAG_IMMUTABLE,
            )
        val baby = role == Role.BABY
        return NotificationCompat.Builder(
                context,
                if (baby) AlertChannels.BABY_STATUS else AlertChannels.STATUS,
            )
            .setSmallIcon(android.R.drawable.ic_btn_speak_now)
            .setContentTitle(context.getString(R.string.native_notif_app_title))
            .setContentText(
                context.getString(
                    if (baby) R.string.native_notif_baby_monitoring else parentText(link)
                )
            )
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setPriority(
                if (baby) NotificationCompat.PRIORITY_MIN else NotificationCompat.PRIORITY_LOW
            )
            .setVisibility(
                if (baby) NotificationCompat.VISIBILITY_SECRET
                else NotificationCompat.VISIBILITY_PRIVATE
            )
            .setContentIntent(open)
            .build()
    }

    /**
     * Baby after the OS killed the process: a microphone service cannot start from the background.
     */
    fun resumePrompt(): Notification {
        val context = localized()
        val open =
            PendingIntent.getActivity(
                context,
                1,
                Intent(context, entryPoints.mainActivity),
                PendingIntent.FLAG_IMMUTABLE,
            )
        return NotificationCompat.Builder(context, AlertChannels.BABY_STATUS)
            .setSmallIcon(android.R.drawable.ic_btn_speak_now)
            .setContentTitle(context.getString(R.string.native_notif_app_title))
            .setContentText(context.getString(R.string.native_notif_baby_resume))
            .setAutoCancel(true)
            .setSilent(true)
            .setPriority(NotificationCompat.PRIORITY_MIN)
            .setContentIntent(open)
            .build()
    }

    companion object {
        const val ID_STATUS = 1
        const val ID_RESUME = 2

        fun parentText(link: LinkUi): Int =
            when (link.phase) {
                LinkPhase.CONNECTED -> R.string.native_notif_status_connected
                LinkPhase.CONNECTING ->
                    if (link.level >= 2) R.string.native_notif_status_alarm
                    else R.string.native_notif_status_connecting
                LinkPhase.RECONNECTING ->
                    when {
                        link.level >= 2 -> R.string.native_notif_status_alarm
                        link.level == 1 -> R.string.native_notif_status_trouble
                        else -> R.string.native_notif_status_reconnecting
                    }
                LinkPhase.PEER_STOPPED -> R.string.native_notif_status_peer_stopped
                LinkPhase.FAILED_NEEDS_USER -> R.string.native_notif_status_failed
                LinkPhase.IDLE,
                LinkPhase.STOPPED -> R.string.native_notif_status_connecting
            }
    }
}
