package org.guardedsleep.app.alerts

import android.content.Context
import android.media.AudioAttributes
import android.os.Build
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager

/**
 * Distinct patterns so a parent can tell the alerts apart without looking (alpha-ux-flows §1.2).
 */
enum class HapticPattern(val timings: LongArray) {
    /** Long-long-longer: noise. */
    NOISE(longArrayOf(0, 600, 250, 600, 250, 1000)),

    /** Short pulses: connection. */
    CONNECTION(longArrayOf(0, 150, 100, 150, 100, 150, 100, 150)),
    SHORT(longArrayOf(0, 150)),
}

class Haptics(context: Context) {
    private val vibrator: Vibrator? =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            context.getSystemService(VibratorManager::class.java)?.defaultVibrator
        } else {
            @Suppress("DEPRECATION") context.getSystemService(Vibrator::class.java)
        }

    private val attributes =
        AudioAttributes.Builder()
            .setUsage(AudioAttributes.USAGE_ALARM)
            .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
            .build()

    @Suppress("DEPRECATION")
    fun play(pattern: HapticPattern) {
        val v = vibrator ?: return
        if (!v.hasVibrator()) return
        runCatching { v.vibrate(VibrationEffect.createWaveform(pattern.timings, -1), attributes) }
    }

    fun cancel() {
        runCatching { vibrator?.cancel() }
    }
}
