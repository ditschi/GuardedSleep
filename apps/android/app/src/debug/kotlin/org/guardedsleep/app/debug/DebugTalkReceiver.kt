package org.guardedsleep.app.debug

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.media.AudioManager
import android.os.Build
import org.guardedsleep.app.diag.PocLog
import org.guardedsleep.app.graph

/**
 * Gate hooks for talk-back (`scripts/poc/gate.sh`, AC-11).
 * - `adb shell am broadcast -a org.guardedsleep.app.alpha.debug.TALK -n
 *   org.guardedsleep.app.alpha/org.guardedsleep.app.debug.DebugTalkReceiver --ez pressed true`
 *   presses (or releases) the parent's talk button.
 * - `-a org.guardedsleep.app.alpha.debug.AUDIO_STATE --es tag before` logs the device's audio mode,
 *   alarm stream index and speaker route, so the gate can compare them before and after a talk.
 */
class DebugTalkReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action?.endsWith(".AUDIO_STATE") == true) {
            logAudio(context, intent.getStringExtra("tag").orEmpty())
        } else {
            val pressed = intent.getBooleanExtra("pressed", false)
            PocLog.event("talk_button", "pressed" to pressed)
            context.graph().controller.setTalk(pressed)
        }
    }

    private fun logAudio(context: Context, tag: String) {
        val am = context.getSystemService(AudioManager::class.java) ?: return
        val route =
            if (Build.VERSION.SDK_INT >= 31) am.communicationDevice?.type?.toString() ?: "none"
            else "n/a"
        PocLog.event(
            "audio_state",
            "tag" to tag.filter { it.isLetterOrDigit() || it == '_' },
            "mode" to am.mode,
            "alarm" to am.getStreamVolume(AudioManager.STREAM_ALARM),
            "speakerphone" to am.isSpeakerphoneOn,
            "route" to route,
        )
    }
}
