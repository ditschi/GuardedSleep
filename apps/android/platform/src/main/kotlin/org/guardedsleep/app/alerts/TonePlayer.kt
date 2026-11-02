package org.guardedsleep.app.alerts

import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioTrack
import org.guardedsleep.app.media.PcmSynth

/**
 * Plays generated tones on the alarm stream, so the user's alarm volume and DND-alarm rules apply.
 */
class TonePlayer {
    private val rate = 16_000
    private var loop: AudioTrack? = null
    private val once = mutableListOf<AudioTrack>()

    @Synchronized fun playSoft() = playOnce(PcmSynth.softTone(rate))

    /** The level-3 alarm: loops until [stop]. */
    @Synchronized
    fun startAlarm() {
        if (loop != null) return
        val samples = PcmSynth.alarmCycle(rate)
        loop =
            track(samples)?.also {
                it.setLoopPoints(0, samples.size, -1)
                it.play()
            }
    }

    @Synchronized
    fun stop() {
        loop?.let {
            runCatching { it.stop() }
            it.release()
        }
        loop = null
        once.forEach {
            runCatching { it.stop() }
            it.release()
        }
        once.clear()
    }

    @Synchronized
    private fun playOnce(samples: ShortArray) {
        val t = track(samples) ?: return
        once += t
        t.setNotificationMarkerPosition(samples.size - 1)
        t.setPlaybackPositionUpdateListener(
            object : AudioTrack.OnPlaybackPositionUpdateListener {
                override fun onMarkerReached(track: AudioTrack) {
                    synchronized(this@TonePlayer) { once.remove(track) }
                    track.release()
                }

                override fun onPeriodicNotification(track: AudioTrack) = Unit
            }
        )
        t.play()
    }

    private fun track(samples: ShortArray): AudioTrack? = runCatching {
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
        .getOrNull()
}
