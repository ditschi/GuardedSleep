package org.guardedsleep.app.media.libwebrtc

import java.nio.ByteBuffer
import java.util.concurrent.atomic.AtomicBoolean
import org.guardedsleep.app.diag.PocLog
import org.guardedsleep.app.media.AttemptListener
import org.guardedsleep.app.media.AttemptPort
import org.guardedsleep.app.media.InboundAudioStats
import org.webrtc.AudioTrack
import org.webrtc.DataChannel
import org.webrtc.MediaConstraints
import org.webrtc.MediaStream
import org.webrtc.PeerConnection
import org.webrtc.PeerConnectionFactory
import org.webrtc.RtpReceiver
import org.webrtc.RtpTransceiver
import org.webrtc.SdpObserver
import org.webrtc.SessionDescription
import uniffi.guardedsleep_core.IceCandidate as CoreIce

/**
 * One PeerConnection + DataChannel for one core attempt handle. The baby sends a `sendonly`
 * audio m-line (the core refuses anything else) and creates the "guardedsleep" channel; the
 * parent only receives.
 */
internal class LibWebRtcAttemptPeer(
    factory: PeerConnectionFactory,
    rtcConfig: PeerConnection.RTCConfiguration,
    private val sendTrack: AudioTrack?,
    private val offerer: Boolean,
    private val listener: AttemptListener,
    private val onRemoteAudio: (AudioTrack) -> Unit,
) : AttemptPort {
    private val lock = Any()
    private val closed = AtomicBoolean(false)
    private val pendingRemoteIce = mutableListOf<org.webrtc.IceCandidate>()
    private var remoteSet = false
    private var answerRequested = false
    private var dataOpenSent = false
    private val fingerprintSent = AtomicBoolean(false)
    @Volatile private var channel: DataChannel? = null
    @Volatile private var hasAudioReceiver = false

    private val observer =
        object : PeerConnection.Observer {
            override fun onSignalingChange(state: PeerConnection.SignalingState) = Unit

            override fun onIceConnectionChange(state: PeerConnection.IceConnectionState) = Unit

            override fun onIceConnectionReceivingChange(receiving: Boolean) = Unit

            override fun onIceGatheringChange(state: PeerConnection.IceGatheringState) = Unit

            override fun onIceCandidate(c: org.webrtc.IceCandidate) {
                if (!closed.get()) listener.onLocalIce(CoreIce(c.sdp, c.sdpMid, c.sdpMLineIndex.toUInt()))
            }

            override fun onIceCandidatesRemoved(candidates: Array<out org.webrtc.IceCandidate>) = Unit

            override fun onAddStream(stream: MediaStream) = Unit

            override fun onRemoveStream(stream: MediaStream) = Unit

            override fun onDataChannel(dc: DataChannel) = attachChannel(dc)

            override fun onRenegotiationNeeded() = Unit

            override fun onAddTrack(receiver: RtpReceiver, streams: Array<out MediaStream>) {
                (receiver.track() as? AudioTrack)?.let {
                    hasAudioReceiver = true
                    onRemoteAudio(it)
                }
            }

            override fun onTrack(transceiver: RtpTransceiver) = Unit

            override fun onConnectionChange(newState: PeerConnection.PeerConnectionState) {
                if (closed.get()) return
                listener.onConnectionState(newState.name.lowercase())
                if (newState == PeerConnection.PeerConnectionState.CONNECTED) reportFingerprint(tries = FP_TRIES)
            }
        }

    private val pc: PeerConnection =
        requireNotNull(factory.createPeerConnection(rtcConfig, observer)) { "createPeerConnection failed" }

    init {
        if (sendTrack != null) {
            pc.addTransceiver(
                sendTrack,
                RtpTransceiver.RtpTransceiverInit(
                    RtpTransceiver.RtpTransceiverDirection.SEND_ONLY,
                    listOf("gs-stream"),
                ),
            )
        }
        if (offerer) {
            attachChannel(pc.createDataChannel("guardedsleep", DataChannel.Init().apply { ordered = true }))
        }
    }

    override fun createOffer() {
        pc.createOffer(
            object : SdpAdapter("createOffer") {
                override fun onCreateSuccess(desc: SessionDescription) = setLocal(desc)
            },
            MediaConstraints(),
        )
    }

    override fun createAnswer() {
        synchronized(lock) {
            if (!remoteSet) {
                answerRequested = true
                return
            }
        }
        pc.createAnswer(
            object : SdpAdapter("createAnswer") {
                override fun onCreateSuccess(desc: SessionDescription) = setLocal(desc)
            },
            MediaConstraints(),
        )
    }

    override fun setRemoteDescription(type: String, sdp: String) {
        val desc = SessionDescription(SessionDescription.Type.fromCanonicalForm(type), sdp)
        pc.setRemoteDescription(
            object : SdpAdapter("setRemote") {
                override fun onSetSuccess() {
                    val (queued, answer) =
                        synchronized(lock) {
                            remoteSet = true
                            val q = pendingRemoteIce.toList()
                            pendingRemoteIce.clear()
                            q to answerRequested.also { answerRequested = false }
                        }
                    if (closed.get()) return
                    queued.forEach { pc.addIceCandidate(it) }
                    if (answer) createAnswer()
                }
            },
            desc,
        )
    }

    override fun addRemoteIce(candidate: CoreIce) {
        val ice =
            org.webrtc.IceCandidate(candidate.sdpMid ?: "0", candidate.sdpMLineIndex?.toInt() ?: 0, candidate.candidate)
        synchronized(lock) {
            if (!remoteSet) {
                pendingRemoteIce += ice
                return
            }
        }
        if (!closed.get()) pc.addIceCandidate(ice)
    }

    override fun sendData(text: String): Boolean {
        val dc = channel ?: return false
        if (closed.get() || dc.state() != DataChannel.State.OPEN) return false
        return dc.send(DataChannel.Buffer(ByteBuffer.wrap(text.toByteArray(Charsets.UTF_8)), false))
    }

    override fun inboundAudioStats(callback: (InboundAudioStats?) -> Unit) {
        if (closed.get() || !hasAudioReceiver) return callback(null)
        pc.getStats { report ->
            val audio =
                report.statsMap.values.firstOrNull { it.type == "inbound-rtp" && it.members["kind"] == "audio" }
            callback(
                audio?.let {
                    InboundAudioStats(
                        (it.members["packetsReceived"] as? Number)?.toLong() ?: 0L,
                        (it.members["packetsLost"] as? Number)?.toLong() ?: 0L,
                    )
                }
            )
        }
    }

    override fun close() {
        if (!closed.compareAndSet(false, true)) return
        runCatching {
            channel?.let {
                it.unregisterObserver()
                it.close()
                it.dispose()
            }
            pc.close()
            pc.dispose()
        }
    }

    /** Reads the remote DTLS certificate fingerprint from the stats; the core cannot verify without it. */
    private fun reportFingerprint(tries: Int) {
        if (closed.get() || fingerprintSent.get()) return
        pc.getStats { report ->
            val stats = report.statsMap
            val certId =
                stats.values.firstNotNullOfOrNull { s ->
                    if (s.type == "transport") s.members["remoteCertificateId"] as? String else null
                }
            val cert = certId?.let { stats[it] }
            val fp = cert?.members?.get("fingerprint") as? String
            val algo = (cert?.members?.get("fingerprintAlgorithm") as? String)?.lowercase()
            if (fp != null && algo == "sha-256") {
                if (fingerprintSent.compareAndSet(false, true) && !closed.get()) listener.onDtlsFingerprint(fp)
            } else if (tries > 1) {
                retry.schedule({ reportFingerprint(tries - 1) }, FP_RETRY_MS, java.util.concurrent.TimeUnit.MILLISECONDS)
            }
        }
    }

    private fun setLocal(desc: SessionDescription) {
        pc.setLocalDescription(
            object : SdpAdapter("setLocal") {
                override fun onSetSuccess() {
                    if (closed.get()) return
                    val local = pc.localDescription ?: desc
                    listener.onLocalDescription(local.type.canonicalForm(), local.description)
                }
            },
            desc,
        )
    }

    private fun attachChannel(dc: DataChannel) {
        channel = dc
        dc.registerObserver(
            object : DataChannel.Observer {
                override fun onBufferedAmountChange(previousAmount: Long) = Unit

                override fun onStateChange() {
                    when (dc.state()) {
                        DataChannel.State.OPEN -> signalOpen()
                        DataChannel.State.CLOSED -> if (!closed.get()) listener.onDataClosed()
                        else -> Unit
                    }
                }

                override fun onMessage(buffer: DataChannel.Buffer) {
                    if (buffer.binary || closed.get()) return
                    val bytes = ByteArray(buffer.data.remaining())
                    buffer.data.get(bytes)
                    listener.onDataMessage(String(bytes, Charsets.UTF_8))
                }
            }
        )
        if (dc.state() == DataChannel.State.OPEN) signalOpen()
    }

    private fun signalOpen() {
        val first = synchronized(lock) { !dataOpenSent.also { dataOpenSent = true } }
        if (first && !closed.get()) listener.onDataOpen()
    }

    private abstract inner class SdpAdapter(private val op: String) : SdpObserver {
        override fun onCreateSuccess(desc: SessionDescription) = Unit

        override fun onSetSuccess() = Unit

        override fun onCreateFailure(error: String?) = fail(error)

        override fun onSetFailure(error: String?) = fail(error)

        private fun fail(error: String?) {
            PocLog.event("sdp_failure", "op" to op, "error" to error?.replace(' ', '_'))
            if (!closed.get()) listener.onConnectionState("failed")
        }
    }

    private companion object {
        const val FP_TRIES = 10
        const val FP_RETRY_MS = 300L
        val retry: java.util.concurrent.ScheduledExecutorService =
            java.util.concurrent.Executors.newSingleThreadScheduledExecutor { r -> Thread(r, "gs-fp").apply { isDaemon = true } }
    }
}
