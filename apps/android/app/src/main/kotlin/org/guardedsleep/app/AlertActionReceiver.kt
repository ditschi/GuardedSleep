package org.guardedsleep.app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import org.guardedsleep.app.ui.model.ListenMode
import org.guardedsleep.app.wiring.AlertIntents

/**
 * Notification buttons: Stop sound, Snooze 10 min, Listen (discreet style), I'm checking. Not
 * exported.
 */
class AlertActionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val controller = context.graph().controller
        when (intent.action) {
            AlertIntents.ACTION_STOP_SOUND -> controller.stopAlertSound()
            AlertIntents.ACTION_SNOOZE_10 -> controller.snooze(10)
            AlertIntents.ACTION_LISTEN -> controller.setListen(ListenMode.LOCKED)
            AlertIntents.ACTION_ACK -> controller.acknowledgeAlarm()
        }
    }
}
