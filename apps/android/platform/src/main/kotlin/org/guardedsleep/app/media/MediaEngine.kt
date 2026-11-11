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

/** Baby: the talk receiver's inbound counters and decoded level (stats; no audio sink). */
data class TalkReceiverStats(val packetsReceived: Long, val audioLevel: Double)

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

    /**
     * Parent: attach the talk microphone to the talk sender (`true`) or detach and dispose it
     * (`false`). False when there is no talk sender or no capture could be opened.
     */
    fun setTalkMic(attached: Boolean): Boolean = false

    /** Baby: enable or disable the talk receiver's track (disabled before any frame by default). */
    fun setTalkReceiverEnabled(enabled: Boolean) {}

    /** Baby: gain of the talk receiver's track (`AudioTrack.setVolume`). */
    fun setTalkReceiverGain(gain: Double) {}

    /** Baby: counters and level of the talk receiver; null while no receiver exists. */
    fun talkReceiverStats(callback: (TalkReceiverStats?) -> Unit) = callback(null)

    fun close()
}

/** Microphone/playout side of a link: one factory and audio device module per monitoring run. */
interface MediaEngine {
    /**
     * [iceServers] is the core's list for this attempt (`CreateAttempt.ice_servers`), used as is;
     * the engine adds only the user-entered TURN of its [IceConfig].
     */
    fun createAttempt(
        offerer: Boolean,
        listener: AttemptListener,
        iceServers: List<IceServerEntry>,
    ): AttemptPort

    /** Baby: gate the outgoing track (the core opens it for noise, listen and probe). */
    fun setMicEnabled(enabled: Boolean)

    /** Baby: mix a synthetic test tone into the outgoing stream; never played locally. */
    fun startProbeTone(durationMs: Long)

    /** Parent: playout muted ("Stop sound") without stopping decode or the meter. */
    fun setPlaybackMuted(muted: Boolean)

    /** Parent: alert gain in percent (25..150). */
    fun setPlaybackGainPct(pct: Int)

    fun setSpeakerOnly(speakerOnly: Boolean)

    /** Parent: mute only the baby's audio while the talk button is held (half duplex). */
    fun setPlaybackDucked(ducked: Boolean) {}

    /** Parent: open the capture locally at the press (not yet attached to the sender). */
    fun prepareTalkCapture() {}

    /** Parent: called when the microphone fails while a talk may run. */
    fun setTalkMicFailureListener(listener: () -> Unit) {}

    /** Baby: the ADM speaker; the engine starts muted and the lease opens it. */
    fun setTalkSpeakerMuted(muted: Boolean) {}

    /** Baby: plays the generated chime on its own alarm-stream track. */
    fun playTalkChime(gain: Double, onDone: () -> Unit) = onDone()

    fun stopTalkChime() {}

    /** Baby: the alarm stream volume control; null when the engine cannot play a talk. */
    fun talkVolume(): AlarmVolume? = null

    /** RMS of the microphone since the previous call; null when no audio was recorded (baby). */
    fun drainMicRms(): Double?

    /** RMS of the received baby audio since the previous call; null when none (parent). */
    fun drainRemoteRms(): Double?

    fun release()
}

/**
 * One ICE server entry of an attempt, exactly as the core handed it over. The credential is
 * short-lived, never logged and never persisted.
 */
data class IceServerEntry(
    val urls: List<String>,
    val username: String? = null,
    val credential: String? = null,
) {
    override fun toString(): String =
        "IceServerEntry(urls=$urls, userSet=${username != null}, credential=<redacted>)"
}

/** Engine-wide ICE settings: the user-entered TURN server and the debug relay-only gate. */
data class IceConfig(
    val turnUrl: String? = null,
    val turnUser: String? = null,
    val turnCredential: String? = null,
    /** Debug gate only: the emulators reach each other through TURN. */
    val relayOnly: Boolean = false,
) {
    // The TURN password must never reach logs through string templates.
    override fun toString(): String =
        "IceConfig(relayOnly=$relayOnly, turnUrlSet=${turnUrl != null}, " +
            "turnUserSet=${turnUser != null}, turnCredential=<redacted>)"
}
