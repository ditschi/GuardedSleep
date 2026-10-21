package org.guardedsleep.app.media

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioManager
import java.nio.ByteBuffer
import java.util.concurrent.CopyOnWriteArrayList
import org.guardedsleep.app.diag.PocLog
import org.webrtc.AudioSource
import org.webrtc.AudioTrack
import org.webrtc.AudioTrackSink
import org.webrtc.MediaConstraints
import org.webrtc.PeerConnection
import org.webrtc.PeerConnectionFactory
import org.webrtc.audio.JavaAudioDeviceModule
import uniffi.guardedsleep_core.Role

/**
 * libwebrtc for one monitoring run: one factory and audio device module, one send track (baby).
 * Playout goes to the alarm stream so the parent's alarm volume and DND-alarm rules apply.
 *
 * The record callback sees the PCM that is about to be encoded: it (a) measures the microphone
 * level for the core, (b) mixes the debug cry injection before measuring so injection exercises
 * the real path, and (c) mixes the probe tone after measuring so the tone cannot trigger the gate.
 */
class LibWebRtcMediaEngine(context: Context, private val role: Role, private val ice: IceConfig) : MediaEngine {
    private val appContext = context.applicationContext
    private val micWindow = RmsWindow()
    private val remoteWindow = RmsWindow()
    private val probe = ProbeTone()
    private val remoteTracks = CopyOnWriteArrayList<AudioTrack>()
    @Volatile private var gain = 1.0
    @Volatile private var speakerMuted = false
    private val adm: JavaAudioDeviceModule
    private val factory: PeerConnectionFactory
    private var source: AudioSource? = null
    private var track: AudioTrack? = null

    private val remoteSink =
        AudioTrackSink { data, bitsPerSample, _, _, _, _ ->
            if (bitsPerSample == 16) {
                val (sum, n) = Pcm.sumSquares(data)
                remoteWindow.add(sum, n)
            }
        }

    private val recordCallback =
        JavaAudioDeviceModule.AudioBufferCallback { buffer, _, channels, rate, bytesRead, _ ->
            // The valid PCM is [0, bytesRead); the helpers read position..limit of a view.
            val pcm: ByteBuffer = buffer.duplicate().clear().limit(bytesRead) as ByteBuffer
            InjectedAudio.mix(pcm, channels)
            val (sum, n) = Pcm.sumSquares(pcm)
            micWindow.add(sum, n)
            probe.mix(pcm, channels, rate)
            // -1: keep the capture timestamp libwebrtc computed.
            -1L
        }

    init {
        ensureInitialized(appContext)
        val attributes =
            AudioAttributes.Builder()
                .setUsage(AudioAttributes.USAGE_ALARM)
                .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                .build()
        val builder =
            JavaAudioDeviceModule.builder(appContext)
                .setAudioAttributes(attributes)
                .setAudioTrackErrorCallback(
                    object : JavaAudioDeviceModule.AudioTrackErrorCallback {
                        override fun onWebRtcAudioTrackInitError(errorMessage: String?) = trackError(errorMessage)

                        override fun onWebRtcAudioTrackStartError(
                            errorCode: JavaAudioDeviceModule.AudioTrackStartErrorCode?,
                            errorMessage: String?,
                        ) = trackError(errorMessage)

                        override fun onWebRtcAudioTrackError(errorMessage: String?) = trackError(errorMessage)
                    }
                )
        if (role == Role.BABY) builder.setAudioBufferCallback(recordCallback)
        adm = builder.createAudioDeviceModule()
        // The baby never plays audio (ADR-018 §3): belt and braces on top of the sendonly SDP.
        if (role == Role.BABY) adm.setSpeakerMute(true)
        factory = PeerConnectionFactory.builder().setAudioDeviceModule(adm).createPeerConnectionFactory()
        if (role == Role.BABY) {
            val src = factory.createAudioSource(MediaConstraints())
            source = src
            track = factory.createAudioTrack("gs-audio", src).apply { setEnabled(false) }
        }
    }

    override fun createAttempt(offerer: Boolean, listener: AttemptListener): AttemptPort =
        AttemptPeer(factory, rtcConfig(), track, offerer, listener, ::onRemoteAudio)

    override fun setMicEnabled(enabled: Boolean) {
        track?.setEnabled(enabled)
    }

    override fun startProbeTone(durationMs: Long) = probe.request(durationMs)

    override fun setPlaybackMuted(muted: Boolean) {
        speakerMuted = muted
        if (role == Role.PARENT) adm.setSpeakerMute(muted)
    }

    override fun setPlaybackGainPct(pct: Int) {
        gain = pct.coerceIn(0, 200) / 100.0
        remoteTracks.forEach { it.setVolume(gain) }
    }

    @Suppress("DEPRECATION")
    override fun setSpeakerOnly(speakerOnly: Boolean) {
        // Best effort: alarm-stream playback already uses the speaker on most phones.
        appContext.getSystemService(AudioManager::class.java)?.isSpeakerphoneOn = speakerOnly
    }

    override fun drainMicRms(): Double? = micWindow.drain()

    override fun drainRemoteRms(): Double? = remoteWindow.drain()

    override fun release() {
        runCatching {
            remoteTracks.forEach { it.removeSink(remoteSink) }
            remoteTracks.clear()
            track?.dispose()
            source?.dispose()
            factory.dispose()
            adm.release()
        }
    }

    private fun onRemoteAudio(t: AudioTrack) {
        t.setVolume(gain)
        t.addSink(remoteSink)
        remoteTracks += t
    }

    private fun trackError(message: String?) {
        PocLog.event("audio_track_error", "message" to message?.replace(' ', '_'))
    }

    private fun rtcConfig(): PeerConnection.RTCConfiguration =
        PeerConnection.RTCConfiguration(iceServers()).apply {
            sdpSemantics = PeerConnection.SdpSemantics.UNIFIED_PLAN
            continualGatheringPolicy = PeerConnection.ContinualGatheringPolicy.GATHER_CONTINUALLY
            iceTransportsType =
                if (ice.relayOnly) PeerConnection.IceTransportsType.RELAY else PeerConnection.IceTransportsType.ALL
        }

    private fun iceServers(): List<PeerConnection.IceServer> = buildList {
        ice.stunUrls.forEach { add(PeerConnection.IceServer.builder(it).createIceServer()) }
        ice.turnUrl?.let { url ->
            add(
                PeerConnection.IceServer.builder(url)
                    .setUsername(ice.turnUser ?: "")
                    .setPassword(ice.turnCredential ?: "")
                    .createIceServer()
            )
        }
    }

    private companion object {
        @Volatile private var initialized = false

        fun ensureInitialized(context: Context) {
            if (initialized) return
            synchronized(this) {
                if (initialized) return
                PeerConnectionFactory.initialize(
                    PeerConnectionFactory.InitializationOptions.builder(context).createInitializationOptions()
                )
                initialized = true
            }
        }
    }
}
