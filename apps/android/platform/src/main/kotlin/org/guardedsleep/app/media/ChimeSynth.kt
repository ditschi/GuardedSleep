package org.guardedsleep.app.media

import kotlin.math.PI
import kotlin.math.sin
import org.guardedsleep.app.config.TalkPolicy

/**
 * The talk chime, generated (no asset): two soft sine tones, each with a slow raised-cosine attack
 * and release, about 600 ms in all. Pure so a JVM test can check length, peak and envelope.
 */
object ChimeSynth {
    const val RATE = 16_000
    private const val NOTE_HZ_1 = 523.25
    private const val NOTE_HZ_2 = 659.25

    /** [gain] is the talk track's attenuation; the peak never falls below the audible floor. */
    fun peak(gain: Double): Double =
        (TalkPolicy.CHIME_PEAK * gain).coerceIn(TalkPolicy.CHIME_FLOOR, 1.0)

    fun generate(gain: Double): ShortArray {
        val total = RATE * TalkPolicy.CHIME_MS / 1000
        val half = total / 2
        val amp = peak(gain) * Short.MAX_VALUE
        val out = ShortArray(total)
        for (i in 0 until total) {
            val inNote = if (i < half) i else i - half
            val hz = if (i < half) NOTE_HZ_1 else NOTE_HZ_2
            val attack = (RATE * 0.06).toInt()
            val release = half - attack
            val env =
                when {
                    inNote < attack -> 0.5 - 0.5 * kotlin.math.cos(PI * inNote / attack)
                    else -> 0.5 + 0.5 * kotlin.math.cos(PI * (inNote - attack) / release)
                }
            out[i] = (amp * env * sin(2 * PI * hz * i / RATE)).toInt().toShort()
        }
        return out
    }
}
