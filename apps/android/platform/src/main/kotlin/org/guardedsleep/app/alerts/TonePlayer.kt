package org.guardedsleep.app.alerts

import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioTrack
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import org.guardedsleep.app.media.PcmSynth
import org.guardedsleep.app.ui.model.AlarmTonePlayback
import uniffi.guardedsleep_core.AlarmTone
import uniffi.guardedsleep_core.ToneSegment

/**
 * One buffer of samples being played. A seam so the JVM tests need no audio device; the real one is
 * [AudioTrackVoices].
 */
interface ToneVoice {
    /**
     * Plays the samples once ([loop] false; [onEnd] runs after the last sample) or until [stop].
     */
    fun start(loop: Boolean, onEnd: () -> Unit)

    /** Stops and frees the voice; safe to call twice. */
    fun stop()
}

fun interface ToneVoiceFactory {
    fun create(samples: ShortArray, rate: Int): ToneVoice?
}

/** Plays on the alarm stream, so the user's alarm volume and DND-alarm rules apply. */
object AudioTrackVoices : ToneVoiceFactory {
    override fun create(samples: ShortArray, rate: Int): ToneVoice? {
        val track =
            runCatching {
                AudioTrack.Builder()
                    .setAudioAttributes(
                        AudioAttributes.Builder()
                            .setUsage(AudioAttributes.USAGE_ALARM)
                            .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
                            .build()
                    )
                    .setAudioFormat(
                        AudioFormat.Builder()
                            .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                            .setSampleRate(rate)
                            .setChannelMask(AudioFormat.CHANNEL_OUT_MONO)
                            .build()
                    )
                    .setBufferSizeInBytes(samples.size * 2)
                    .setTransferMode(AudioTrack.MODE_STATIC)
                    .build()
                    .also { it.write(samples, 0, samples.size) }
            }
                .getOrNull() ?: return null
        return object : ToneVoice {
            private var released = false

            override fun start(loop: Boolean, onEnd: () -> Unit) {
                if (loop) {
                    track.setLoopPoints(0, samples.size, -1)
                } else {
                    track.setNotificationMarkerPosition(samples.size - 1)
                    track.setPlaybackPositionUpdateListener(
                        object : AudioTrack.OnPlaybackPositionUpdateListener {
                            override fun onMarkerReached(t: AudioTrack) {
                                onEnd()
                                stop()
                            }

                            override fun onPeriodicNotification(t: AudioTrack) = Unit
                        }
                    )
                }
                track.play()
            }

            @Synchronized
            override fun stop() {
                if (released) return
                released = true
                runCatching { track.stop() }
                track.release()
            }
        }
    }
}

/**
 * Plays the connection alarm tones (recipes from the core, samples from [PcmSynth]). A real alert
 * ([playSoft], [startAlarm]) always stops a preview or test first.
 */
class TonePlayer(
    private val rate: Int = SAMPLE_RATE,
    private val voices: ToneVoiceFactory = AudioTrackVoices,
) {
    private var loop: ToneVoice? = null
    private var loopTone: AlarmTone? = null
    private var loopStartedMs = 0L
    private var pendingTone: AlarmTone? = null
    private val once = mutableListOf<ToneVoice>()
    private var demo: ToneVoice? = null
    private val handler = Handler(Looper.getMainLooper())
    private val switchLoop = Runnable { switchToPending() }
    private val _playback = MutableStateFlow<AlarmTonePlayback>(AlarmTonePlayback.Idle)

    /** What the picker plays right now. */
    val playback: StateFlow<AlarmTonePlayback> = _playback

    @Synchronized
    fun playSoft(tone: AlarmTone) {
        stopDemo()
        val voice = voices.create(PcmSynth.render(tone.soft, rate), rate) ?: return
        once += voice
        voice.start(loop = false) { synchronized(this) { once.remove(voice) } }
    }

    /**
     * The level-3 alarm: loops until [stop]. A running alarm of another tone switches to [tone] at
     * the start of its next cycle.
     */
    @Synchronized
    fun startAlarm(tone: AlarmTone) {
        stopDemo()
        val running = loopTone
        if (loop == null || running == null) {
            startLoop(tone)
        } else if (running.id == tone.id) {
            handler.removeCallbacks(switchLoop)
            pendingTone = null
        } else {
            pendingTone = tone
            handler.removeCallbacks(switchLoop)
            handler.postDelayed(switchLoop, untilNextCycleMs(running))
        }
    }

    /** Ends a running alarm and any soft tone; a preview or test is left alone ([stopDemo]). */
    @Synchronized
    fun stop() {
        handler.removeCallbacks(switchLoop)
        pendingTone = null
        loop?.stop()
        loop = null
        loopTone = null
        once.forEach { it.stop() }
        once.clear()
    }

    /** Preview: the soft tone, then two alarm cycles, at reduced volume (about half). */
    @Synchronized
    fun preview(tone: AlarmTone) {
        if (loop != null) return
        val samples =
            PcmSynth.render(demoSegments(tone, cycles = PREVIEW_CYCLES), rate, PREVIEW_GAIN)
        startDemo(samples, AlarmTonePlayback.Preview(tone.id))
    }

    /** Test: the soft tone, then the alarm for about 8 seconds, at full volume. */
    @Synchronized
    fun test(tone: AlarmTone) {
        if (loop != null) return
        val cycleMs = tone.alarm.sumOf { it.ms.toInt() }.coerceAtLeast(1)
        val soft = tone.soft.sumOf { it.ms.toInt() } + DEMO_PAUSE_MS
        val cycles = ((TEST_MS - soft + cycleMs - 1) / cycleMs).coerceAtLeast(1)
        startDemo(
            PcmSynth.render(demoSegments(tone, cycles), rate),
            AlarmTonePlayback.Test(tone.id),
        )
    }

    /** Stops a running preview or test at once. */
    @Synchronized
    fun stopDemo() {
        demo?.stop()
        demo = null
        _playback.value = AlarmTonePlayback.Idle
    }

    private fun demoSegments(tone: AlarmTone, cycles: Int) =
        tone.soft +
            ToneSegment(emptyList(), DEMO_PAUSE_MS.toUInt(), 0u) +
            List(cycles) { tone.alarm }.flatten()

    private fun startLoop(tone: AlarmTone) {
        val voice = voices.create(PcmSynth.render(tone.alarm, rate), rate)
        loopTone = tone
        loopStartedMs = SystemClock.elapsedRealtime()
        loop = voice?.also { it.start(loop = true) {} }
    }

    private fun untilNextCycleMs(tone: AlarmTone): Long {
        val cycle = tone.alarm.sumOf { it.ms.toLong() }.coerceAtLeast(1)
        return cycle - (SystemClock.elapsedRealtime() - loopStartedMs) % cycle
    }

    @Synchronized
    private fun switchToPending() {
        val next = pendingTone ?: return
        pendingTone = null
        loop?.stop()
        loop = null
        startLoop(next)
    }

    private fun startDemo(samples: ShortArray, state: AlarmTonePlayback) {
        stopDemo()
        val voice = voices.create(samples, rate) ?: return
        demo = voice
        _playback.value = state
        voice.start(loop = false) {
            synchronized(this) {
                if (demo === voice) {
                    demo = null
                    _playback.value = AlarmTonePlayback.Idle
                }
            }
        }
    }

    companion object {
        const val SAMPLE_RATE = 16_000

        /** The preview plays at about half of the alarm volume so it does not wake the house. */
        const val PREVIEW_GAIN = 0.5
        const val PREVIEW_CYCLES = 2
        const val TEST_MS = 8_000
        private const val DEMO_PAUSE_MS = 300
    }
}
