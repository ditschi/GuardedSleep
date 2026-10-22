package org.guardedsleep.app.media

import uniffi.guardedsleep_core.IceCandidate

/**
 * Callbacks of one peer connection. Called on WebRTC threads; the receiver must hop to its
 * executor.
 */
interface AttemptListener {
    fun onLocalDescription(type: String, sdp: String)

    fun onLocalIce(candidate: IceCandidate)

    fun onDataOpen()

    fun onDataMessage(text: String)

    fun onDataClosed()

    /**
     * Lowercase `RTCPeerConnectionState`: new, connecting, connected, disconnected, failed, closed.
     */
    fun onConnectionState(state: String)

    /** SHA-256 fingerprint of the remote DTLS certificate (colon-separated hex), once connected. */
    fun onDtlsFingerprint(fingerprint: String)
}

data class InboundAudioStats(val packetsReceived: Long, val packetsLost: Long)

/** One RTCPeerConnection + DataChannel lifetime (an "attempt" of the core). */
interface AttemptPort {
    fun createOffer()

    fun setRemoteDescription(type: String, sdp: String)

    /** Runs after the remote description is applied, even if requested earlier. */
    fun createAnswer()

    fun addRemoteIce(candidate: IceCandidate)

    fun sendData(text: String): Boolean

    /** Parent: inbound audio counters; null when no audio receiver exists yet. */
    fun inboundAudioStats(callback: (InboundAudioStats?) -> Unit)

    fun close()
}

/** Microphone/playout side of a link: one factory and audio device module per monitoring run. */
interface MediaEngine {
    fun createAttempt(offerer: Boolean, listener: AttemptListener): AttemptPort

    /** Baby: gate the outgoing track (the core opens it for noise, listen and probe). */
    fun setMicEnabled(enabled: Boolean)

    /** Baby: mix a synthetic test tone into the outgoing stream; never played locally. */
    fun startProbeTone(durationMs: Long)

    /** Parent: playout muted ("Stop sound") without stopping decode or the meter. */
    fun setPlaybackMuted(muted: Boolean)

    /** Parent: alert gain in percent (25..150). */
    fun setPlaybackGainPct(pct: Int)

    fun setSpeakerOnly(speakerOnly: Boolean)

    /** RMS of the microphone since the previous call; null when no audio was recorded (baby). */
    fun drainMicRms(): Double?

    /** RMS of the received baby audio since the previous call; null when none (parent). */
    fun drainRemoteRms(): Double?

    fun release()
}

data class IceConfig(
    val stunUrls: List<String>,
    val turnUrl: String? = null,
    val turnUser: String? = null,
    val turnCredential: String? = null,
    /** Debug gate only: the emulators reach each other through TURN. */
    val relayOnly: Boolean = false,
)
