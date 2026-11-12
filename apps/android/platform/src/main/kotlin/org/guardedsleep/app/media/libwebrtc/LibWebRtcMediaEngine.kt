package org.guardedsleep.app.media.libwebrtc

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioManager
import java.util.concurrent.CopyOnWriteArrayList
import org.guardedsleep.app.diag.PocLog
import org.guardedsleep.app.media.AlarmVolume
import org.guardedsleep.app.media.AndroidStreamVolume
import org.guardedsleep.app.media.AttemptListener
import org.guardedsleep.app.media.AttemptPort
import org.guardedsleep.app.media.CaptureTap
import org.guardedsleep.app.media.ChimePlayer
import org.guardedsleep.app.media.IceConfig
import org.guardedsleep.app.media.IceServerEntry
import org.guardedsleep.app.media.MediaEngine
import org.guardedsleep.app.media.Pcm
import org.guardedsleep.app.media.PrefsVolumeMemory
import org.guardedsleep.app.media.ProbeTone
import org.guardedsleep.app.media.RmsWindow
import org.guardedsleep.app.ui.model.Role
import org.webrtc.AudioSource
import org.webrtc.AudioTrack
import org.webrtc.AudioTrackSink
import org.webrtc.MediaConstraints
import org.webrtc.PeerConnection
import org.webrtc.PeerConnectionFactory
import org.webrtc.audio.JavaAudioDeviceModule

/**
 * libwebrtc for one monitoring run: one factory and audio device module, one send track (baby).
 * Playout goes to the alarm stream so the parent's alarm volume and DND-alarm rules apply.
 *
 * The record callback sees the PCM that is about to be encoded: it (a) measures the microphone
 * level for the core, (b) mixes the debug cry injection before measuring so injection exercises the
 * real path, and (c) mixes the probe tone after measuring so the tone cannot trigger the gate.
 */
class LibWebRtcMediaEngine(context: Context, private val role: Role, private val ice: IceConfig) :
    MediaEngine {
    private val appContext = context.applicationContext
    private val micWindow = RmsWindow()
    private val remoteWindow = RmsWindow()
    private val probe = ProbeTone()
    private val remoteTracks = CopyOnWriteArrayList<AudioTrack>()
    @Volatile private var gain = 1.0
    @Volatile private var speakerMuted = false
    @Volatile private var ducked = false
    @Volatile private var talkMicFailed: () -> Unit = {}
    private val chime = ChimePlayer()
    private val talkVolume: AlarmVolume? =
        if (role == Role.BABY) AlarmVolume(AndroidStreamVolume(context), PrefsVolumeMemory(context))
        else null
    private var talkSource: AudioSource? = null
    private var talkTrack: AudioTrack? = null
    private val adm: JavaAudioDeviceModule
    private val factory: PeerConnectionFactory
    private var source: AudioSource? = null
    private var track: AudioTrack? = null

    private val remoteSink = AudioTrackSink { data, bitsPerSample, _, _, _, _ ->
        if (bitsPerSample == 16) {
            val (sum, n) = Pcm.sumSquares(data)
            remoteWindow.add(sum, n)
        }
    }

    private val captureTap = CaptureTap(micWindow, probe)

    private val recordCallback =
        JavaAudioDeviceModule.AudioBufferCallback {
            buffer,
            _,
            channels,
            rate,
            bytesRead,
            captureTimeNs ->
            captureTap.onBuffer(buffer, channels, rate, bytesRead, captureTimeNs)
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
                        override fun onWebRtcAudioTrackInitError(errorMessage: String?) =
                            trackError(errorMessage)

                        override fun onWebRtcAudioTrackStartError(
                            errorCode: JavaAudioDeviceModule.AudioTrackStartErrorCode?,
                            errorMessage: String?,
                        ) = trackError(errorMessage)

                        override fun onWebRtcAudioTrackError(errorMessage: String?) =
                            trackError(errorMessage)
                    }
                )
        if (role == Role.BABY) builder.setAudioBufferCallback(recordCallback)
        else {
            // The parent's capture is the talk microphone: voice processing on (design §4), and
            // no record callback, so nothing can read the PCM. A capture error ends the talk.
            builder
                .setUseHardwareAcousticEchoCanceler(true)
                .setUseHardwareNoiseSuppressor(true)
                .setAudioRecordErrorCallback(
                    object : JavaAudioDeviceModule.AudioRecordErrorCallback {
                        override fun onWebRtcAudioRecordInitError(errorMessage: String?) =
                            talkMicFailed()

                        override fun onWebRtcAudioRecordStartError(
                            errorCode: JavaAudioDeviceModule.AudioRecordStartErrorCode?,
                            errorMessage: String?,
                        ) = talkMicFailed()

                        override fun onWebRtcAudioRecordError(errorMessage: String?) =
                            talkMicFailed()
                    }
                )
        }
        adm = builder.createAudioDeviceModule()
        // The baby never plays audio (ADR-018 §3): belt and braces on top of the sendonly SDP.
        if (role == Role.BABY) adm.setSpeakerMute(true)
        factory =
            PeerConnectionFactory.builder().setAudioDeviceModule(adm).createPeerConnectionFactory()
        if (role == Role.BABY) {
            val src = factory.createAudioSource(MediaConstraints())
            source = src
            track = factory.createAudioTrack("gs-audio", src).apply { setEnabled(false) }
        }
    }

    override fun createAttempt(
        offerer: Boolean,
        listener: AttemptListener,
        iceServers: List<IceServerEntry>,
    ): AttemptPort =
        LibWebRtcAttemptPeer(
            factory,
            rtcConfig(iceServers),
            track,
            offerer,
            listener,
            ::onRemoteAudio,
            if (role == Role.PARENT) talkCapture else null,
        )

    /** Opened at the press (warm) or at the attach, disposed on every detach. */
    private val talkCapture =
        object : TalkCapture {
            override fun open(): AudioTrack? = openTalkTrack()

            override fun close() = disposeTalkTrack()
        }

    @Synchronized
    private fun openTalkTrack(): AudioTrack? {
        talkTrack?.let {
            it.setEnabled(true)
            return it
        }
        return runCatching {
            // Voice processing on: echo cancellation, noise suppression, auto gain.
            val constraints =
                MediaConstraints().apply {
                    for (k in
                        listOf(
                            "googEchoCancellation",
                            "googNoiseSuppression",
                            "googAutoGainControl",
                        )) {
                        mandatory.add(MediaConstraints.KeyValuePair(k, "true"))
                    }
                }
            val src = factory.createAudioSource(constraints)
            val t = factory.createAudioTrack("gs-talk", src)
            talkSource = src
            talkTrack = t
            t
        }
            .getOrNull()
    }

    @Synchronized
    private fun disposeTalkTrack() {
        runCatching {
            talkTrack?.setEnabled(false)
            talkTrack?.dispose()
            talkSource?.dispose()
        }
        talkTrack = null
        talkSource = null
    }

    override fun prepareTalkCapture() {
        // Created but not attached: nothing is sent and the track is disabled until the attach.
        openTalkTrack()?.setEnabled(false)
    }

    override fun setTalkMicFailureListener(listener: () -> Unit) {
        talkMicFailed = listener
    }

    override fun setPlaybackDucked(ducked: Boolean) {
        this.ducked = ducked
        if (role == Role.PARENT) adm.setSpeakerMute(speakerMuted || ducked)
    }

    override fun setTalkSpeakerMuted(muted: Boolean) {
        if (role == Role.BABY) adm.setSpeakerMute(muted)
    }

    override fun playTalkChime(gain: Double, onDone: () -> Unit) = chime.play(gain, onDone)

    override fun stopTalkChime() = chime.stop()

    override fun talkVolume(): AlarmVolume? = talkVolume

    override fun setMicEnabled(enabled: Boolean) {
        track?.setEnabled(enabled)
    }

    override fun startProbeTone(durationMs: Long) = probe.request(durationMs)

    override fun setPlaybackMuted(muted: Boolean) {
        speakerMuted = muted
        if (role == Role.PARENT) adm.setSpeakerMute(muted || ducked)
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
        chime.stop()
        runCatching {
            remoteTracks.forEach { it.removeSink(remoteSink) }
            remoteTracks.clear()
            disposeTalkTrack()
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

    private fun rtcConfig(attemptServers: List<IceServerEntry>): PeerConnection.RTCConfiguration =
        PeerConnection.RTCConfiguration(iceServers(attemptServers)).apply {
            sdpSemantics = PeerConnection.SdpSemantics.UNIFIED_PLAN
            continualGatheringPolicy = PeerConnection.ContinualGatheringPolicy.GATHER_CONTINUALLY
            iceTransportsType =
                if (ice.relayOnly) PeerConnection.IceTransportsType.RELAY
                else PeerConnection.IceTransportsType.ALL
        }

    private fun iceServers(attemptServers: List<IceServerEntry>): List<PeerConnection.IceServer> =
        buildList {
            // The core's list, unchanged: URLs, user name and credential as it computed them.
            attemptServers.forEach { e ->
                val b = PeerConnection.IceServer.builder(e.urls)
                e.username?.let(b::setUsername)
                e.credential?.let(b::setPassword)
                add(b.createIceServer())
            }
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
                    PeerConnectionFactory.InitializationOptions.builder(context)
                        .createInitializationOptions()
                )
                initialized = true
            }
        }
    }
}
