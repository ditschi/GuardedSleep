package org.guardedsleep.app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/** Notification buttons: Stop sound, Snooze 10 min, I'm checking. Not exported. */
class AlertActionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val controller = context.graph().controller
        when (intent.action) {
            ACTION_STOP_SOUND -> controller.stopAlertSound()
            ACTION_SNOOZE_10 -> controller.snooze(10)
            ACTION_ACK -> controller.acknowledgeAlarm()
        }
    }

    companion object {
        const val ACTION_STOP_SOUND = "org.guardedsleep.app.action.STOP_SOUND"
        const val ACTION_SNOOZE_10 = "org.guardedsleep.app.action.SNOOZE_10"
        const val ACTION_ACK = "org.guardedsleep.app.action.ACK"
    }
}
