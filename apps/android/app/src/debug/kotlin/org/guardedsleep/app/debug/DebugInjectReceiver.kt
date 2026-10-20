package org.guardedsleep.app.debug

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import org.guardedsleep.app.diag.PocLog
import org.guardedsleep.app.media.InjectedAudio

/**
 * `adb shell am broadcast -a org.guardedsleep.app.alpha.debug.INJECT_AUDIO -n org.guardedsleep.app.alpha/org.guardedsleep.app.debug.DebugInjectReceiver --ef rms 0.6 --el duration_ms 3000`
 */
class DebugInjectReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val rms = intent.getFloatExtra("rms", 0.6f).toDouble()
        val duration = intent.getLongExtra("duration_ms", 3_000L)
        InjectedAudio.inject(rms, duration)
        PocLog.event("inject", "rms" to rms, "duration_ms" to duration)
    }
}
