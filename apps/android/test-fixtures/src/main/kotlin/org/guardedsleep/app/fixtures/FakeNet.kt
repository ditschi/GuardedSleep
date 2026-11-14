package org.guardedsleep.app.fixtures

import java.util.UUID
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.atomic.AtomicInteger
import org.guardedsleep.app.media.AttemptListener
import org.guardedsleep.app.media.AttemptPort
import org.guardedsleep.app.media.IceServerEntry
import org.guardedsleep.app.media.InboundAudioStats
import org.guardedsleep.app.media.MediaEngine
import org.guardedsleep.app.pairing.DefaultPairingMarker
import org.guardedsleep.app.pairing.PairingCipher
import org.guardedsleep.app.pairing.PairingStore
import org.guardedsleep.app.pairing.RolePairings
import org.guardedsleep.app.signaling.SignalingListener
import org.guardedsleep.app.signaling.SignalingTransport
import org.guardedsleep.app.silence.SilenceGateway
import org.guardedsleep.app.ui.model.Role
import org.json.JSONArray
import org.json.JSONObject
import uniffi.guardedsleep_core.DeviceStatus
import uniffi.guardedsleep_core.IceCandidate

/**
 * In-memory signaling relay (the v3 server protocol) and fake WebRTC, so two real cores talk to
 * each other.
 */
class FakeNet(private val scheduler: ManualScheduler) {
    val lock = Any()
    val members = LinkedHashMap<String, Pair<String, FakeTransport>>() // clientId -> role
    val rooms = HashMap<String, MutableList<String>>()
    val offers = ConcurrentHashMap<String, FakeAttempt>()
    /** Sees every DataChannel text before it is delivered (on the sender's thread). */
    @Volatile var onSend: ((text: String) -> Unit)? = null
    val attemptsCreated = AtomicInteger()
    /** The ICE list of every attempt the cores asked the media engine for. */
    val attemptIce = CopyOnWriteArrayList<List<IceServerEntry>>()
    /** Every address a mailbox socket was opened at. */
    val openedUrls = CopyOnWriteArrayList<String>()
    /** A relay `ice_servers` frame sent to each member right after the welcome. */
    @Volatile var iceFrame: String? = null
    val liveAttempts = CopyOnWriteArrayList<FakeAttempt>()

    /** Delivery is a task on the virtual scheduler, in order, at the current time. */
    fun later(block: () -> Unit) = scheduler.post(block)

    fun join(t: FakeTransport, role: String, room: String) =
        synchronized(lock) {
            val id = UUID.randomUUID().toString().take(8)
            members[id] = role to t
            t.clientId = id
            t.room = room
            rooms.getOrPut(room) { mutableListOf() } += id
            t.deliver(
                JSONObject()
                    .put("type", "welcome")
                    .put("clientId", id)
                    .put("roomId", room)
                    .toString()
            )
            iceFrame?.let { t.deliver(it) }
            broadcastMembers(room)
        }

    fun leave(t: FakeTransport) =
        synchronized(lock) {
            val id = t.clientId ?: return@synchronized
            members.remove(id)
            t.room?.let { r ->
                rooms[r]?.remove(id)
                broadcastMembers(r)
            }
        }

    private fun broadcastMembers(room: String) {
        val ids = rooms[room].orEmpty()
        val peers = JSONArray()
        ids.forEach { peers.put(JSONObject().put("clientId", it).put("role", members[it]!!.first)) }
        val msg = JSONObject().put("type", "members").put("peers", peers).toString()
        ids.forEach { members[it]!!.second.deliver(msg) }
    }

    fun handle(from: FakeTransport, text: String) =
        synchronized(lock) {
            val o = JSONObject(text)
            when (o.getString("type")) {
                "hello" ->
                    if (rejectProtocol)
                        from.deliver(
                            """{"type":"error","code":"UNSUPPORTED_PROTOCOL","message":"test"}"""
                        )
                    else join(from, o.getString("role"), o.getString("roomId"))
                "ping" -> from.deliver("""{"type":"pong"}""")
                "signal" -> {
                    val to = o.optString("to", "")
                    val msg =
                        JSONObject()
                            .put("type", "signal")
                            .put("from", from.clientId)
                            .put("payload", o.get("payload"))
                            .toString()
                    rooms[from.room]
                        .orEmpty()
                        .filter { it != from.clientId && (to.isEmpty() || it == to) }
                        .forEach { members[it]!!.second.deliver(msg) }
                }
            }
        }

    val answerers = ConcurrentHashMap<String, FakeAttempt>()
    val fakeAttempts = CopyOnWriteArrayList<FakeAttempt>()

    fun registerOffer(token: String, a: FakeAttempt) {
        offers[token] = a
    }

    fun registerAnswerer(token: String, a: FakeAttempt) {
        answerers[token] = a
    }

    /** While true the relay answers a hello with the protocol-not-supported error. */
    @Volatile var rejectProtocol = false

    /** While true no new connection is made (a long outage); see [breakAll]. */
    @Volatile var outage = false

    fun connect(token: String) {
        if (outage) return
        val offerer = offers[token]
        val answerer = answerers[token]
        if (offerer == null || answerer == null) return
        offerer.peer = answerer
        answerer.peer = offerer
        listOf(offerer to answerer, answerer to offerer).forEach { (a, other) ->
            liveAttempts += a
            later {
                a.listener.onConnectionState("connected")
                a.listener.onDtlsFingerprint(other.fpValue)
                a.listener.onDataOpen()
            }
        }
    }

    /** Both ends of every live connection fail, like a Wi-Fi outage. */
    fun breakAll() {
        liveAttempts.forEach { a ->
            later {
                a.listener.onDataClosed()
                a.listener.onConnectionState("failed")
            }
        }
        liveAttempts.clear()
    }
}

class FakeTransport(val net: FakeNet, val listener: SignalingListener) : SignalingTransport {
    var clientId: String? = null
    var room: String? = null
    @Volatile var closed = false

    fun deliver(text: String) = net.later { if (!closed) listener.onMessage(text) }

    override fun open() = net.later { if (!closed) listener.onOpened() }

    override fun send(text: String): Boolean {
        net.later { if (!closed) net.handle(this, text) }
        return true
    }

    override fun close() {
        closed = true
        net.leave(this)
    }
}

class FakeAttempt(val net: FakeNet, val listener: AttemptListener, val baby: Boolean) :
    AttemptPort {
    var token: String? = null
    val fpValue = (0 until 32).joinToString(":") { "%02X".format(java.util.Random().nextInt(256)) }
    val fingerprint = "a=fingerprint:sha-256 $fpValue"
    var peer: FakeAttempt? = null
    @Volatile var closed = false

    override fun createOffer() {
        token = UUID.randomUUID().toString()
        net.registerOffer(token!!, this)
        val sdp =
            "v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=sendonly\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=recvonly\r\n$fingerprint\r\na=gs-token:$token\r\n"
        net.later { listener.onLocalDescription("offer", sdp) }
    }

    override fun setRemoteDescription(type: String, sdp: String) {
        val t = Regex("a=gs-token:(\\S+)").find(sdp)!!.groupValues[1]
        token = t
        if (type == "offer") net.registerAnswerer(t, this) else net.connect(t)
    }

    override fun createAnswer() {
        val sdp =
            "v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=recvonly\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=sendonly\r\n$fingerprint\r\na=gs-token:$token\r\n"
        net.later { listener.onLocalDescription("answer", sdp) }
    }

    override fun addRemoteIce(candidate: IceCandidate) = Unit

    override fun sendData(text: String): Boolean {
        net.onSend?.invoke(text)
        val p = peer ?: return false
        if (closed) return false
        net.later { if (!p.closed) p.listener.onDataMessage(text) }
        return true
    }

    override fun inboundAudioStats(callback: (InboundAudioStats?) -> Unit) = callback(null)

    val talkMic = CopyOnWriteArrayList<Boolean>()

    override fun setTalkMic(attached: Boolean): Boolean {
        talkMic += attached
        return true
    }

    override fun close() {
        closed = true
        peer?.let { p -> net.later { if (!p.closed) p.listener.onDataClosed() } }
        net.liveAttempts.remove(this)
    }
}

class FakeMedia(val net: FakeNet, val baby: Boolean) : MediaEngine {
    val mic = CopyOnWriteArrayList<Boolean>()

    /** What the microphone hears next ([TestPhones.injectNoise] sets it); null is silence. */
    @Volatile var micRms: Double? = null

    /** What the parent's playback measures of the baby's audio ([TestPhones.injectNoise]). */
    @Volatile var remoteRms: Double? = null

    override fun createAttempt(
        offerer: Boolean,
        listener: AttemptListener,
        iceServers: List<IceServerEntry>,
    ): AttemptPort {
        net.attemptIce += iceServers
        net.attemptsCreated.incrementAndGet()
        return FakeAttempt(net, listener, baby).also { net.fakeAttempts += it }
    }

    override fun setMicEnabled(enabled: Boolean) {
        mic += enabled
    }

    override fun startProbeTone(durationMs: Long) = Unit

    override fun setPlaybackMuted(muted: Boolean) = Unit

    override fun setPlaybackGainPct(pct: Int) = Unit

    override fun setSpeakerOnly(speakerOnly: Boolean) = Unit

    override fun drainMicRms(): Double? = micRms

    override fun drainRemoteRms(): Double? = remoteRms

    override fun release() = Unit
}

class FakeSilence : SilenceGateway {
    var active = false

    override fun hasAccess() = true

    override fun activate(): Boolean {
        active = true
        return true
    }

    override fun deactivate() {
        active = false
    }

    override fun restoreLeftover() = false

    override fun readDeviceStatus() =
        DeviceStatus(
            80u,
            true,
            uniffi.guardedsleep_core.Ringer.NORMAL,
            uniffi.guardedsleep_core.Dnd.OFF,
            true,
            true,
            active,
            null,
            null,
            null,
            emptyList(),
            null,
            null,
            null,
            null,
            null,
        )
}

/** Both slots in [dir]; [cipher] seals the slot of [role], the other one uses a fake. */
fun rolePairings(
    dir: java.io.File,
    role: Role,
    cipher: PairingCipher = FakeCipher(),
    legacy: PairingStore? = null,
    defaults: DefaultPairingMarker = DefaultPairingMarker.InMemory(),
) =
    RolePairings(
        Role.entries.associateWith {
            listOf("", "-2").map { suffix ->
                PairingStore(
                    java.io.File(dir, "pairing-${it.name.lowercase()}$suffix.bin"),
                    if (it == role && suffix.isEmpty()) cipher else FakeCipher(),
                )
            }
        },
        legacy,
        defaults,
    )
