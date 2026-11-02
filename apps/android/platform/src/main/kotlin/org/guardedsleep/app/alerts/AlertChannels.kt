package org.guardedsleep.app.alerts

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.media.AudioAttributes
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.locale.localizedContext

/**
 * Notification channels. The two alarm channels carry no sound or vibration of their own: the app
 * plays the repeating patterns itself (distinct for noise and connection), so a channel only
 * decides visibility and Do Not Disturb. The baby channel is minimum importance and fully silent
 * (ADR-018 §3.1).
 */
object AlertChannels {
    const val ALARM_NOISE = "alarm_noise"
    const val ALARM_LINK = "alarm_link"
    const val STATUS = "status"
    const val NOTICE = "notice"
    const val INFO = "info"
    const val BABY_STATUS = "baby_status"

    private val alarmAttributes =
        AudioAttributes.Builder()
            .setUsage(AudioAttributes.USAGE_ALARM)
            .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
            .build()

    /**
     * Creates the channels, or renames them when the language changed (an existing channel keeps
     * its other settings). Names follow the stored app language [storedTag] on every API level.
     */
    fun ensure(base: Context, storedTag: String?) {
        val context = localizedContext(base, storedTag)
        val nm = context.getSystemService(NotificationManager::class.java)
        fun create(
            id: String,
            name: Int,
            desc: Int,
            importance: Int,
            block: NotificationChannel.() -> Unit = {},
        ) {
            nm.createNotificationChannel(
                NotificationChannel(id, context.getString(name), importance).apply {
                    description = context.getString(desc)
                    setSound(null, null)
                    enableVibration(false)
                    enableLights(false)
                    setShowBadge(false)
                    block()
                }
            )
        }
        create(
            ALARM_NOISE,
            R.string.native_notif_channel_noise,
            R.string.native_notif_channel_noise_desc,
            NotificationManager.IMPORTANCE_HIGH,
        ) {
            setBypassDnd(true)
            setSound(null, alarmAttributes)
            lockscreenVisibility = Notification.VISIBILITY_PUBLIC
        }
        create(
            ALARM_LINK,
            R.string.native_notif_channel_link,
            R.string.native_notif_channel_link_desc,
            NotificationManager.IMPORTANCE_HIGH,
        ) {
            setBypassDnd(true)
            setSound(null, alarmAttributes)
            lockscreenVisibility = Notification.VISIBILITY_PUBLIC
        }
        create(
            STATUS,
            R.string.native_notif_channel_status,
            R.string.native_notif_channel_status_desc,
            NotificationManager.IMPORTANCE_LOW,
        )
        create(
            NOTICE,
            R.string.native_notif_channel_notice,
            R.string.native_notif_channel_notice_desc,
            NotificationManager.IMPORTANCE_DEFAULT,
        ) {
            // One short haptic, never a sound.
            enableVibration(true)
            vibrationPattern = longArrayOf(0, 150)
        }
        create(
            INFO,
            R.string.native_notif_channel_info,
            R.string.native_notif_channel_info_desc,
            NotificationManager.IMPORTANCE_LOW,
        )
        create(
            BABY_STATUS,
            R.string.native_notif_channel_baby,
            R.string.native_notif_channel_baby_desc,
            NotificationManager.IMPORTANCE_MIN,
        ) {
            lockscreenVisibility = Notification.VISIBILITY_SECRET
        }
    }
}
