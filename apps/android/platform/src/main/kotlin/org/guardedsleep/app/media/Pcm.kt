package org.guardedsleep.app.media

import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.PI
import kotlin.math.sin
import kotlin.math.sqrt
import org.guardedsleep.app.platform.BuildConfig
import uniffi.guardedsleep_core.ToneSegment

/** Little-endian PCM16 helpers; buffers are the ones libwebrtc hands to the record callback. */
object Pcm {
    const val PCM16_BYTES = 2

    /** Sum of squares of the normalized samples between position and limit. */
    fun sumSquares(buffer: ByteBuffer): Pair<Double, Int> {
        val b = buffer.duplicate().order(ByteOrder.LITTLE_ENDIAN)
        var sum = 0.0
        var n = 0
        var i = b.position()
        while (i + 1 < b.limit()) {
            val x = b.getShort(i) / 32768.0
            sum += x * x
            n++
            i += PCM16_BYTES
        }
        return sum to n
    }

    fun rms(buffer: ByteBuffer): Double {
        val (sum, n) = sumSquares(buffer)
        return if (n == 0) 0.0 else sqrt(sum / n)
    }

    fun rms(samples: ShortArray): Double {
        if (samples.isEmpty()) return 0.0
        var sum = 0.0
        for (s in samples) {
            val x = s / 32768.0
            sum += x * x
        }
        return sqrt(sum / samples.size)
    }

    /** Adds [source] to every channel of every frame (saturating). */
    inline fun mixInPlace(buffer: ByteBuffer, channels: Int, source: (frame: Int) -> Int) {
        val b = buffer.duplicate().order(ByteOrder.LITTLE_ENDIAN)
        val ch = channels.coerceAtLeast(1)
        var i = b.position()
        var frame = 0
        while (i + PCM16_BYTES * ch <= b.limit()) {
            val add = source(frame)
            for (c in 0 until ch) {
                val at = i + c * PCM16_BYTES
                b.putShort(at, (b.getShort(at) + add).coerceIn(-32768, 32767).toShort())
            }
            i += PCM16_BYTES * ch
            frame++
        }
    }
}

/** RMS over everything added since the last [drain]; written by audio threads, read at ~10 Hz. */
class RmsWindow {
    private var sum = 0.0
    private var count = 0L

    @Synchronized
    fun add(sumSquares: Double, samples: Int) {
        sum += sumSquares
        count += samples
    }

    /** Null when no audio arrived since the previous call (so the core is not fed zeros). */
    @Synchronized
    fun drain(): Double? {
        if (count == 0L) return null
        val rms = sqrt(sum / count)
        sum = 0.0
        count = 0
        return rms
    }
}

/**
 * Synthesizes the tone recipes of the core (`alarm_tone`): sine partials with a short fade in and
 * out so a tone never clicks. A segment without partials is silence.
 */
object PcmSynth {
    /** Renders [segments] one after another into 16-bit mono samples. */
    fun render(segments: List<ToneSegment>, sampleRate: Int): ShortArray =
        segments.map { segment(it, sampleRate) }.fold(ShortArray(0)) { all, part -> all + part }

    /** The same samples as [render], scaled by [gain] (0..1); used by the quiet preview. */
    fun render(segments: List<ToneSegment>, sampleRate: Int, gain: Double): ShortArray =
        render(segments, sampleRate).also { samples ->
            for (i in samples.indices) samples[i] = (samples[i] * gain).toInt().toShort()
        }

    private fun segment(seg: ToneSegment, sampleRate: Int): ShortArray {
        val n = sampleRate * seg.ms.toInt() / 1000
        if (seg.partials.isEmpty()) return ShortArray(n)
        val fade = (sampleRate * seg.fadeMs.toInt() / 1000).coerceAtLeast(1)
        return ShortArray(n) { i ->
            val env =
                minOf(1.0, i / fade.toDouble(), (n - 1 - i) / fade.toDouble()).coerceAtLeast(0.0)
            var sum = 0.0
            for (p in seg.partials) sum += sin(2 * PI * p.freqHz * i / sampleRate) * p.amplitude
            (sum * env * Short.MAX_VALUE).toInt().coerceIn(-32768, 32767).toShort()
        }
    }
}

/**
 * Probe tone for the test alert (ADR-018 "Test alert"): mixed into the outgoing microphone buffer
 * only. The baby never plays it, and the core's level measurement is taken before mixing.
 */
class ProbeTone(private val freqHz: Double = 1000.0, private val amplitude: Double = 0.4) {
    private var remaining = 0L
    private var phase = 0.0
    private var queuedMs = 0L

    @Synchronized
    fun request(durationMs: Long) {
        queuedMs = durationMs
    }

    @Synchronized
    fun mix(buffer: ByteBuffer, channels: Int, sampleRate: Int) {
        if (queuedMs > 0) {
            remaining = queuedMs * sampleRate / 1000
            queuedMs = 0
        }
        if (remaining <= 0) return
        val step = 2 * PI * freqHz / sampleRate
        Pcm.mixInPlace(buffer, channels) {
            if (remaining <= 0) {
                0
            } else {
                remaining--
                phase += step
                (sin(phase) * amplitude * Short.MAX_VALUE).toInt()
            }
        }
    }
}

/**
 * Debug-only synthetic "cry" (gate scripts). The emulator microphone is silent, so the debug
 * `INJECT_AUDIO` broadcast adds noise of a given RMS to the real record buffer, which exercises the
 * normal AudioRecord → core path. Release builds never call [inject].
 */
object InjectedAudio {
    @Volatile private var rms = 0.0
    @Volatile private var untilMs = 0L
    private val random = java.util.Random(7)

    fun inject(level: Double, durationMs: Long, nowMs: Long = System.currentTimeMillis()) {
        rms = level.coerceIn(0.0, 1.0)
        untilMs = nowMs + durationMs
    }

    fun active(nowMs: Long = System.currentTimeMillis()): Boolean =
        BuildConfig.DEBUG && nowMs < untilMs

    /** The injected level while active, for a fallback when the record callback is not running. */
    fun level(nowMs: Long = System.currentTimeMillis()): Double? = if (active(nowMs)) rms else null

    fun mix(buffer: ByteBuffer, channels: Int) {
        if (!active()) return
        // Uniform noise in [-a, a] has RMS a / sqrt(3).
        val a = rms * sqrt(3.0) * Short.MAX_VALUE
        synchronized(random) {
            Pcm.mixInPlace(buffer, channels) { ((random.nextDouble() * 2 - 1) * a).toInt() }
        }
    }
}
