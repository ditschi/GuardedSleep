package org.guardedsleep.app.media

import android.content.Context
import android.content.SharedPreferences
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioTrack
import androidx.core.content.edit

/** The generated chime on its own alarm-stream track (never the ADM, never a file). */
class ChimePlayer {
    @Volatile private var track: AudioTrack? = null
    @Volatile private var cancelled = false

    /** Plays [gain]-scaled PCM; [onDone] runs once, also when playback fails or is stopped. */
    fun play(gain: Double, onDone: () -> Unit) {
        stop()
        cancelled = false
        val pcm = ChimeSynth.generate(gain)
        Thread(
                {
                    try {
                        val t =
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
                                        .setSampleRate(ChimeSynth.RATE)
                                        .setChannelMask(AudioFormat.CHANNEL_OUT_MONO)
                                        .build()
                                )
                                .setBufferSizeInBytes(pcm.size * 2)
                                .setTransferMode(AudioTrack.MODE_STATIC)
                                .build()
                        track = t
                        t.write(pcm, 0, pcm.size)
                        if (!cancelled) {
                            t.play()
                            val deadline = System.nanoTime() + 1_200_000_000L
                            while (
                                !cancelled &&
                                    t.playbackHeadPosition < pcm.size &&
                                    System.nanoTime() < deadline
                            ) {
                                Thread.sleep(20)
                            }
                        }
                        runCatching { t.stop() }
                        t.release()
                    } catch (_: Exception) {
                        // A chime that cannot play ends at once; the core also has a timeout.
                    } finally {
                        track = null
                        onDone()
                    }
                },
                "gs-chime",
            )
            .apply { isDaemon = true }
            .start()
    }

    fun stop() {
        cancelled = true
        runCatching { track?.stop() }
    }
}

class AndroidStreamVolume(context: Context) : StreamVolume {
    private val am = context.getSystemService(AudioManager::class.java)
    override val max: Int
        get() = am.getStreamMaxVolume(AudioManager.STREAM_ALARM)

    override val fixed: Boolean
        get() = am.isVolumeFixed

    override var index: Int
        get() = am.getStreamVolume(AudioManager.STREAM_ALARM)
        set(value) = am.setStreamVolume(AudioManager.STREAM_ALARM, value, 0)
}

/** Two integers, no secret: plain preferences are enough. */
class PrefsVolumeMemory(context: Context) : VolumeMemory {
    private val prefs: SharedPreferences =
        context.getSharedPreferences("talk_volume", Context.MODE_PRIVATE)

    // `commit = true` on purpose: the remembered raise must be on disk before the volume is
    // raised, or a crash right after would leave it unknown.
    override fun save(previous: Int, set: Int) {
        prefs.edit(commit = true) {
            putInt("previous", previous)
            putInt("set", set)
        }
    }

    override fun load(): Pair<Int, Int>? =
        if (prefs.contains("previous")) prefs.getInt("previous", 0) to prefs.getInt("set", 0)
        else null

    override fun clear() {
        prefs.edit(commit = true) { clear() }
    }
}

/** The settings' "Play chime": the chime at the chosen level, the volume restored afterwards. */
class AndroidTalkChimePreview(context: Context) : org.guardedsleep.app.link.TalkChimePreview {
    private val volume = AlarmVolume(AndroidStreamVolume(context), PrefsVolumeMemory(context))
    private val player = ChimePlayer()

    override fun play(level: org.guardedsleep.app.ui.model.TalkVolume) {
        val core =
            when (level) {
                org.guardedsleep.app.ui.model.TalkVolume.LOW ->
                    uniffi.guardedsleep_core.TalkLevel.LOW
                org.guardedsleep.app.ui.model.TalkVolume.MEDIUM ->
                    uniffi.guardedsleep_core.TalkLevel.MEDIUM
                org.guardedsleep.app.ui.model.TalkVolume.HIGH ->
                    uniffi.guardedsleep_core.TalkLevel.HIGH
            }
        val gain = volume.prepare(core) ?: return
        player.play(gain) { volume.restore() }
    }
}
