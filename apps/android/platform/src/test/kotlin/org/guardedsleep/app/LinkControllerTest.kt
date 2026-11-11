package org.guardedsleep.app

import java.security.SecureRandom
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicInteger
import org.guardedsleep.app.config.PublicLists
import org.guardedsleep.app.link.Clock
import org.guardedsleep.app.link.LinkController
import org.guardedsleep.app.link.MonitorHost
import org.guardedsleep.app.link.NetworkWatcher
import org.guardedsleep.app.link.buildIceConfig
import org.guardedsleep.app.media.AttemptListener
import org.guardedsleep.app.media.AttemptPort
import org.guardedsleep.app.media.IceServerEntry
import org.guardedsleep.app.media.InboundAudioStats
import org.guardedsleep.app.media.MediaEngine
import org.guardedsleep.app.pairing.PairingCipher
import org.guardedsleep.app.pairing.PairingPayload
import org.guardedsleep.app.pairing.PairingStore
import org.guardedsleep.app.pairing.RolePairings
import org.guardedsleep.app.pairing.StoredMailbox
import org.guardedsleep.app.pairing.StoredPairing
import org.guardedsleep.app.pairing.TurnCredentials
import org.guardedsleep.app.pairing.TurnStore
import org.guardedsleep.app.settings.SettingsRepository
import org.guardedsleep.app.signaling.SignalingFactory
import org.guardedsleep.app.signaling.SignalingListener
import org.guardedsleep.app.signaling.SignalingTransport
import org.guardedsleep.app.silence.SilenceGateway
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.PairingPhase
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.TurnSaveResult
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import uniffi.guardedsleep_core.DeviceStatus
import uniffi.guardedsleep_core.IceCandidate

/**
 * In-memory signaling relay (the v3 server protocol) and fake WebRTC, so two real cores talk to
 * each other.
 */
private class FakeNet {
    val delivery = Executors.newSingleThreadExecutor { Thread(it, "fake-net") }
    private val lock = Any()
    private val members = LinkedHashMap<String, Pair<String, FakeTransport>>() // clientId -> role
    private val rooms = HashMap<String, MutableList<String>>()
    private val offers = ConcurrentHashMap<String, FakeAttempt>()
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

    fun later(block: () -> Unit) = delivery.execute { runCatching(block) }

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
                "hello" -> join(from, o.getString("role"), o.getString("roomId"))
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

    private val answerers = ConcurrentHashMap<String, FakeAttempt>()

    fun registerOffer(token: String, a: FakeAttempt) {
        offers[token] = a
    }

    fun registerAnswerer(token: String, a: FakeAttempt) {
        answerers[token] = a
    }

    fun connect(token: String) {
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

private class FakeTransport(private val net: FakeNet, private val listener: SignalingListener) :
    SignalingTransport {
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

private class FakeAttempt(val net: FakeNet, val listener: AttemptListener, val baby: Boolean) :
    AttemptPort {
    var token: String? = null
    val fpValue = (0 until 32).joinToString(":") { "%02X".format(java.util.Random().nextInt(256)) }
    private val fingerprint = "a=fingerprint:sha-256 $fpValue"
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

    override fun close() {
        closed = true
        peer?.let { p -> net.later { if (!p.closed) p.listener.onDataClosed() } }
        net.liveAttempts.remove(this)
    }
}

private class FakeMedia(val net: FakeNet, val baby: Boolean) : MediaEngine {
    val mic = CopyOnWriteArrayList<Boolean>()

    override fun createAttempt(
        offerer: Boolean,
        listener: AttemptListener,
        iceServers: List<IceServerEntry>,
    ): AttemptPort {
        net.attemptIce += iceServers
        net.attemptsCreated.incrementAndGet()
        return FakeAttempt(net, listener, baby)
    }

    override fun setMicEnabled(enabled: Boolean) {
        mic += enabled
    }

    override fun startProbeTone(durationMs: Long) = Unit

    override fun setPlaybackMuted(muted: Boolean) = Unit

    override fun setPlaybackGainPct(pct: Int) = Unit

    override fun setSpeakerOnly(speakerOnly: Boolean) = Unit

    override fun drainMicRms(): Double? = null

    override fun drainRemoteRms(): Double? = null

    override fun release() = Unit
}

private class FakeSilence : SilenceGateway {
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

private class RealClock : Clock {
    private val t0 = System.nanoTime()

    override fun elapsedMs() = 1_000 + (System.nanoTime() - t0) / 1_000_000

    override fun wallMs() = System.currentTimeMillis()
}

private val projectUrl = "wss://${PublicLists.projectServer.host}/signaling"

/** Both slots in [dir]; [cipher] seals the slot of [role], the other one uses a fake. */
private fun rolePairings(
    dir: java.io.File,
    role: Role,
    cipher: PairingCipher = FakeCipher(),
    legacy: PairingStore? = null,
) =
    RolePairings(
        Role.entries.associateWith {
            PairingStore(
                java.io.File(dir, "pairing-${it.name.lowercase()}.bin"),
                if (it == role) cipher else FakeCipher(),
            )
        },
        legacy,
    )

private const val PIN_TYPE = "\"parent_pin\""

class LinkControllerTest {
    @get:Rule val tmp = TemporaryFolder()
    private val net = FakeNet()
    private val controllers = mutableListOf<LinkController>()

    private class Unit2(
        val controller: LinkController,
        val store: PairingStore,
        val settings: SettingsRepository,
        val pairings: RolePairings,
        val sink: RecordingSink,
        val silence: FakeSilence,
        val turn: TurnStore,
    )

    /**
     * A phone that chose [role]; with a null role the app starts as it would after a restart
     * ([settings] and [pairings] of an earlier [Unit2] simulate that).
     */
    private fun unit(
        name: String,
        role: Role?,
        hostThrows: Boolean = false,
        deviceName: String = "",
        pairingCipher: PairingCipher = FakeCipher(),
        settings: SettingsRepository? = null,
        pairings: RolePairings? = null,
    ): Unit2 {
        val dir = tmp.newFolder(name)
        val settings = settings ?: testSettings(dir)
        val pairings = pairings ?: rolePairings(dir, role ?: Role.BABY, pairingCipher)
        val store = pairings[role ?: Role.BABY]
        val turnStore = TurnStore(java.io.File(dir, "turn.bin"), FakeCipher())
        val sink = RecordingSink()
        val silence = FakeSilence()
        lateinit var controller: LinkController
        val host =
            object : MonitorHost {
                override fun start() {
                    if (hostThrows) throw IllegalStateException("fgs not allowed")
                    controller.ensureRun()
                }

                override fun stop() = Unit
            }
        controller =
            LinkController(
                settingsRepo = settings,
                pairings = pairings,
                turnStore = turnStore,
                signaling =
                    SignalingFactory { url, l ->
                        net.openedUrls += url
                        FakeTransport(net, l)
                    },
                mediaFactory = { r, _ -> FakeMedia(net, r == Role.BABY) },
                silence = silence,
                alerts = sink,
                readiness = { emptyList() },
                host = host,
                network =
                    object : NetworkWatcher {
                        override fun start(onAvailable: () -> Unit) = Unit

                        override fun stop() = Unit
                    },
                clock = RealClock(),
                random = SecureRandom(),
                deviceName = { deviceName },
            )
        controller.updateSettings { it.copy(signalingUrls = listOf("wss://fake.example")) }
        if (role != null) controller.chooseRole(role)
        controllers += controller
        return Unit2(controller, store, settings, pairings, sink, silence, turnStore)
    }

    @Test
    fun turnServerIsSealedAndMirroredWithoutPassword() {
        val u = unit("turn1", Role.PARENT)
        val c = u.controller
        assertEquals(
            TurnSaveResult.SAVED,
            c.setTurnServer(" turn:h:3478 ", "user", "turn-pw-test-only"),
        )
        assertEquals("turn:h:3478", c.settings.value.turnUrl)
        assertEquals("user", c.settings.value.turnUsername)
        assertTrue(c.settings.value.turnPasswordSaved)
        assertFalse(c.settings.value.toString().contains("turn-pw-test-only"))
        await("sealed") { u.turn.load()?.credential == "turn-pw-test-only" }

        // Empty password with unchanged address and user name keeps the saved one.
        assertEquals(TurnSaveResult.SAVED, c.setTurnServer("turn:h:3478", "user", ""))
        // A changed address needs the password again.
        assertEquals(TurnSaveResult.INVALID_CREDENTIALS, c.setTurnServer("turn:other", "user", ""))
        assertTrue(c.settings.value.turnPasswordSaved)
        assertEquals(TurnSaveResult.INVALID_ADDRESS, c.setTurnServer("stun:x", "", ""))

        // updateSettings cannot write the TURN fields.
        c.updateSettings { it.copy(turnUrl = "turn:evil", turnPasswordSaved = false) }
        assertEquals("turn:h:3478", c.settings.value.turnUrl)
        assertTrue(c.settings.value.turnPasswordSaved)

        assertEquals(TurnSaveResult.CLEARED, c.setTurnServer("", "", ""))
        assertNull(c.settings.value.turnUrl)
        assertFalse(c.settings.value.turnPasswordSaved)
        await("cleared") { u.turn.load() == null }
    }

    @Test
    fun clearTurnServerRemovesTheEntry() {
        val u = unit("turn2", Role.PARENT)
        u.controller.setTurnServer("turn:h", "", "")
        assertEquals("turn:h", u.controller.settings.value.turnUrl)
        u.controller.clearTurnServer()
        assertNull(u.controller.settings.value.turnUrl)
    }

    @Test
    fun iceConfigPrefersDebugLaunchEntryOverStoredOne() {
        val stored = TurnCredentials("turn:stored", "su", "stored-secret-test-only")
        val pending = TurnCredentials("turn:debug", "du", "dp")
        val fromStore = buildIceConfig(null, stored, false)
        assertEquals("turn:stored", fromStore.turnUrl)
        assertEquals("su", fromStore.turnUser)
        assertEquals("stored-secret-test-only", fromStore.turnCredential)
        assertFalse(fromStore.toString().contains("stored-secret-test-only"))
        assertEquals("turn:debug", buildIceConfig(pending, stored, true).turnUrl)
        val bare = buildIceConfig(null, TurnCredentials("turn:h", "", ""), false)
        assertNull(bare.turnUser)
        assertNull(bare.turnCredential)
        assertNull(buildIceConfig(null, null, false).turnUrl)
    }

    @Test
    fun theAttemptsIceListReachesTheMediaEngineUnchanged() {
        net.iceFrame =
            """{"type":"ice_servers","urls":["turn:fake.example:3478?transport=udp"],""" +
                """"username":"1999999999:abc","credential":"turn-cred-test-only","ttl":600}"""
        pair()
        await("an attempt was created") { net.attemptIce.isNotEmpty() }
        val turn = net.attemptIce.flatten().firstOrNull { it.credential != null }
        assertNotNull("the relay's TURN set reaches the engine", turn)
        assertEquals(listOf("turn:fake.example:3478?transport=udp"), turn!!.urls)
        assertEquals("1999999999:abc", turn.username)
        assertEquals("turn-cred-test-only", turn.credential)
        assertFalse(turn.toString().contains("turn-cred-test-only"))
        // The core adds the relay's built-in STUN for a mailbox that sent none; a credential never
        // belongs to a STUN entry.
        assertTrue(net.attemptIce.flatten().all { e -> e.credential != null || e.username == null })
    }

    private fun seedRecord(u: Unit2, mailboxes: List<StoredMailbox>) {
        u.store.save(
            StoredPairing(
                PairingPayload.b64(ByteArray(32) { 7 }),
                // A legacy URL list that must never be opened without a stored set.
                listOf("wss://legacy.example"),
                true,
                null,
                null,
                mailboxes,
                null,
            )
        )
    }

    @Test
    fun aRecordWithoutAMailboxSetOpensNoMailboxAndFallsBackToNothing() {
        val parent = unit("nofb", Role.PARENT)
        // Settings and defaults name relays; the record does not, so none is opened.
        parent.controller.updateSettings {
            it.copy(advancedEnabled = true, signalingUrls = listOf("wss://settings.example"))
        }
        seedRecord(parent, emptyList())
        parent.controller.startMonitoring()
        await("the run started") { parent.controller.app.value.monitoring }
        Thread.sleep(600)
        assertEquals(emptyList<String>(), net.openedUrls.toList())
    }

    @Test
    fun theOffSwitchKeepsTheProjectServerOutOfTheOpenedMailboxes() {
        val parent = unit("onlylocal", Role.PARENT)
        parent.controller.updateSettings { it.copy(onlyLocal = true) }
        seedRecord(
            parent,
            listOf(StoredMailbox("relay", projectUrl), StoredMailbox("relay", "wss://own.example")),
        )
        parent.controller.startMonitoring()
        await("the own relay was opened") { net.openedUrls.contains("wss://own.example") }
        Thread.sleep(300)
        assertEquals(listOf("wss://own.example"), net.openedUrls.toList())
    }

    @Test
    fun theOffSwitchOffOpensTheWholeSet() {
        val parent = unit("switchoff", Role.PARENT)
        seedRecord(
            parent,
            listOf(StoredMailbox("relay", projectUrl), StoredMailbox("relay", "wss://own.example")),
        )
        parent.controller.startMonitoring()
        await("both opened") { net.openedUrls.containsAll(listOf(projectUrl, "wss://own.example")) }
    }

    @Test
    fun aProjectEntryInTheInviteAndInTheMailboxUpdateIsIgnoredWithTheOffSwitch() {
        val baby = unit("upd-baby", Role.BABY)
        val parent = unit("upd-parent", Role.PARENT)
        baby.controller.updateSettings {
            it.copy(signalingUrls = listOf(projectUrl, "wss://fake.example"))
        }
        parent.controller.updateSettings { it.copy(onlyLocal = true) }
        baby.controller.startPairing()
        parent.controller.submitPairingCode(baby.controller.pairing.value!!.codePayload!!)
        await("both paired", 8_000) {
            baby.controller.pairing.value?.phase == PairingPhase.PAIRED &&
                parent.controller.pairing.value?.phase == PairingPhase.PAIRED
        }
        await("the parent stores the update") { parent.store.load()!!.mailboxes.isNotEmpty() }
        assertEquals(
            listOf(StoredMailbox("relay", "wss://fake.example")),
            parent.store.load()!!.mailboxes,
        )
        // The baby (switch off) opened the projectUrl mailbox; the parent never did: one open of
        // the
        // projectUrl address is the baby's.
        assertEquals(1, net.openedUrls.count { it == projectUrl })
    }

    @Test
    fun theOffSwitchLeavesTheProjectServerOutOfANewInvite() {
        val baby = unit("invite", Role.BABY)
        baby.controller.updateSettings {
            it.copy(signalingUrls = listOf(projectUrl, "wss://own.example"), onlyLocal = true)
        }
        baby.controller.startPairing()
        val code = baby.controller.pairing.value!!.codePayload!!
        val parsed =
            PairingPayload.parse(code, System.currentTimeMillis())
                as PairingPayload.Companion.Result.Ok
        assertEquals(listOf("wss://own.example"), parsed.payload.signalingUrls)
        baby.controller.updateSettings { it.copy(onlyLocal = false) }
        baby.controller.startPairing()
        val all =
            PairingPayload.parse(
                baby.controller.pairing.value!!.codePayload!!,
                System.currentTimeMillis(),
            ) as PairingPayload.Companion.Result.Ok
        assertEquals(listOf(projectUrl, "wss://own.example"), all.payload.signalingUrls)
    }

    @After
    fun tearDown() {
        controllers.forEach { it.stopMonitoring() }
        net.delivery.shutdownNow()
    }

    // ---- session role, start mode and per-role pairings --------------------------------------

    private fun pairing(psk: String) = StoredPairing(psk, listOf("wss://fake.example"), false)

    @Test
    fun startWithoutADefaultShowsTheRoleScreen() {
        val u = unit("nodefault", null)
        assertNull(u.controller.app.value.role)
        assertNull(u.controller.app.value.defaultRole)
    }

    @Test
    fun startWithADefaultOpensThatRole() {
        val first = unit("withdefault1", Role.PARENT)
        first.controller.chooseRole(Role.PARENT, makeDefault = true)
        // A second start of the same phone (same stores), role screen state left behind.
        first.controller.switchMode()
        val second =
            unit("withdefault2", null, settings = first.settings, pairings = first.pairings)
        assertEquals(Role.PARENT, second.controller.app.value.role)
        assertEquals(Role.PARENT, second.controller.app.value.defaultRole)
    }

    @Test
    fun restartDuringMonitoringResumesTheSessionRole() {
        val first = unit("resume1", Role.BABY)
        first.controller.setDefaultRole(Role.PARENT)
        first.settings.update { it.copy(monitoringActive = true) }
        val second = unit("resume2", null, settings = first.settings, pairings = first.pairings)
        assertEquals(Role.BABY, second.controller.app.value.role)
        assertEquals(Role.PARENT, second.controller.app.value.defaultRole)
        assertTrue(second.controller.app.value.monitoring)
    }

    @Test
    fun switchingKeepsBothPairingsAndTheDefault() {
        val u = unit("switch", Role.PARENT)
        val c = u.controller
        u.pairings[Role.PARENT].save(pairing("parent-psk"))
        c.chooseRole(Role.PARENT, makeDefault = true)
        assertTrue(c.app.value.paired)

        c.switchMode()
        assertNull(c.app.value.role)
        assertEquals(Role.PARENT, c.app.value.defaultRole)
        assertFalse(c.app.value.paired)

        c.chooseRole(Role.BABY)
        assertEquals(Role.BABY, c.app.value.role)
        assertEquals(Role.PARENT, c.app.value.defaultRole)
        assertFalse("the baby slot is empty", c.app.value.paired)
        u.pairings[Role.BABY].save(pairing("baby-psk"))
        c.switchMode()

        c.chooseRole(Role.PARENT)
        assertTrue(c.app.value.paired)
        assertEquals("parent-psk", u.pairings[Role.PARENT].load()?.pskB64url)
        assertEquals("baby-psk", u.pairings[Role.BABY].load()?.pskB64url)
    }

    @Test
    fun forgetDeletesOnlyThePairingOfTheCurrentRole() {
        val u = unit("forget", Role.PARENT)
        u.pairings[Role.PARENT].save(pairing("parent-psk"))
        u.pairings[Role.BABY].save(pairing("baby-psk"))
        u.controller.forgetPairing()
        assertNull(u.pairings[Role.PARENT].load())
        assertEquals("baby-psk", u.pairings[Role.BABY].load()?.pskB64url)
        assertFalse(u.controller.app.value.paired)
    }

    @Test
    fun switchModeCancelsAnOpenInviteAndIsIgnoredWhileMonitoring() {
        val u = unit("invite", Role.BABY)
        val c = u.controller
        c.startPairing()
        assertNotNull(c.pairing.value)
        c.switchMode()
        assertNull(c.pairing.value)
        assertNull(c.app.value.role)

        c.chooseRole(Role.BABY)
        u.settings.update { it.copy(monitoringActive = true) }
        c.switchMode()
        assertEquals(Role.BABY, c.app.value.role)
    }

    @Test
    fun theSnoozeAndPeerNameBelongToTheirRole() {
        val u = unit("perrole", Role.PARENT)
        u.settings.update { it.copy(peerName = "Lena", snoozeUntilWallMs = 42L) }
        u.controller.switchMode()
        u.controller.chooseRole(Role.BABY)
        assertNull(u.controller.app.value.peerName)
        u.controller.switchMode()
        u.controller.chooseRole(Role.PARENT)
        assertEquals("Lena", u.controller.app.value.peerName)
        assertEquals(42L, u.settings.current.snoozeUntilWallMs)
    }

    private fun await(what: String, timeoutMs: Long = 15_000, cond: () -> Boolean) {
        val end = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < end) {
            if (cond()) return
            Thread.sleep(25)
        }
        throw AssertionError("timeout waiting for $what")
    }

    private fun pair(parentName: String = ""): Pair<Unit2, Unit2> {
        val baby = unit("baby", Role.BABY)
        val parent = unit("parent", Role.PARENT, deviceName = parentName)
        baby.controller.startPairing()
        val payload = baby.controller.pairing.value!!.codePayload!!
        parent.controller.submitPairingCode(payload)
        try {
            await("both paired", 8_000) {
                baby.controller.pairing.value?.phase == PairingPhase.PAIRED &&
                    parent.controller.pairing.value?.phase == PairingPhase.PAIRED
            }
        } catch (e: AssertionError) {
            throw AssertionError(
                "baby pairing=${baby.controller.pairing.value?.phase} link=${baby.controller.baby.value.link}; " +
                    "parent pairing=${parent.controller.pairing.value} link=${parent.controller.parent.value.link}",
                e,
            )
        }
        await("parent connected") {
            parent.controller.parent.value.link.phase == LinkPhase.CONNECTED
        }
        await("baby connected") { baby.controller.baby.value.link.phase == LinkPhase.CONNECTED }
        return baby to parent
    }

    @Test
    fun happyPathPairsAndMonitors() {
        val (baby, parent) = pair()
        assertTrue(baby.store.load()!!.pairedBefore)
        assertTrue(parent.store.load()!!.pairedBefore)
        assertEquals(baby.store.load()!!.pskB64url, parent.store.load()!!.pskB64url)
        assertTrue(baby.controller.app.value.monitoring)
        assertTrue(parent.controller.app.value.paired)
        assertTrue(baby.silence.active)
        assertNotNull(parent.controller.parent.value.quietSinceWallMs)
        assertEquals(0, parent.controller.parent.value.link.level)
        await("baby status reaches parent") { parent.controller.parent.value.peer.batteryPct == 80 }
        assertTrue(parent.sink.lost.isEmpty())
    }

    @Test
    fun pairingPinsTheParentAndKeepsTheSecretOnBothDevices() {
        val (baby, parent) = pair("Pixel 8")
        await("both keep the secret") {
            baby.store.load()?.parentSecretB64url != null &&
                parent.store.load()?.parentSecretB64url != null
        }
        assertEquals(
            baby.store.load()!!.parentSecretB64url,
            parent.store.load()!!.parentSecretB64url,
        )
        assertEquals(43, baby.store.load()!!.parentSecretB64url!!.length)
        // The baby shows who it is connected to; nothing but the name the parent chose.
        await("the baby shows the parent's name") {
            baby.controller.baby.value.parentName == "Pixel 8"
        }
        assertEquals(false, baby.controller.baby.value.securityWarning)
        // The parent stores the mailbox set the baby sent, in the same encrypted record.
        await("the parent stores the mailbox set") { parent.store.load()!!.mailboxes.isNotEmpty() }
        assertEquals(
            listOf(StoredMailbox("relay", "wss://fake.example")),
            parent.store.load()!!.mailboxes,
        )
        assertEquals(
            listOf(StoredMailbox("relay", "wss://fake.example")),
            baby.store.load()!!.mailboxes,
        )
        assertTrue(!baby.store.load().toString().contains(baby.store.load()!!.parentSecretB64url!!))
    }

    @Test
    fun theParentSecretIsOnDiskBeforeTheMessageThatCarriesItIsSent() {
        val baby = unit("baby", Role.BABY)
        val parent = unit("parent", Role.PARENT, deviceName = "Pixel 8")
        val checks = CopyOnWriteArrayList<Pair<Boolean, Boolean>>()
        net.onSend = { text ->
            if (text.contains(PIN_TYPE)) {
                val stored = parent.store.load()?.parentSecretB64url
                // The message carries the secret as hex, the record as base64url.
                val hex =
                    stored
                        ?.let { org.guardedsleep.app.pairing.PairingPayload.decodePsk(it) }
                        ?.joinToString("") { "%02x".format(it) }
                checks += (stored != null) to (hex != null && text.contains(hex))
            }
        }
        baby.controller.startPairing()
        parent.controller.submitPairingCode(baby.controller.pairing.value!!.codePayload!!)
        await("the secret was sent") { checks.isNotEmpty() }
        assertTrue("persisted before the send: $checks", checks.all { it.first && it.second })
        await("both paired") {
            baby.controller.pairing.value?.phase == PairingPhase.PAIRED &&
                parent.controller.pairing.value?.phase == PairingPhase.PAIRED
        }
        // The baby's pairing is complete at the pin: its record holds the same secret.
        await("the baby stores the pin") { baby.store.load()?.parentSecretB64url != null }
        assertEquals(
            baby.store.load()!!.parentSecretB64url,
            parent.store.load()!!.parentSecretB64url,
        )
    }

    @Test
    fun aSecretThatCannotBePersistedIsNotSent() {
        val baby = unit("baby", Role.BABY)
        val parent = unit("parent", Role.PARENT, deviceName = "Pixel 8")
        val sent = CopyOnWriteArrayList<String>()
        net.onSend = { text -> if (text.contains(PIN_TYPE)) sent += text }
        // The first-pairing record cannot be written: a directory sits where the temp file goes.
        java.io.File(tmp.root, "parent/pairing-parent.bin.tmp").mkdirs()
        baby.controller.startPairing()
        parent.controller.submitPairingCode(baby.controller.pairing.value!!.codePayload!!)
        Thread.sleep(3_000)
        assertTrue("nothing was sent: $sent", sent.isEmpty())
        assertNull(parent.store.load())
        assertNull(baby.store.load())
    }

    @Test
    fun aRestartedPairProvesTheSecretAndConnectsAgain() {
        val (baby, parent) = pair("Pixel 8")
        await("secret kept") { parent.store.load()?.parentSecretB64url != null }
        parent.controller.stopMonitoring()
        await("baby sees stop") { baby.controller.baby.value.link.phase == LinkPhase.PEER_STOPPED }
        parent.controller.startMonitoring()
        await("parent connected again") {
            parent.controller.parent.value.link.phase == LinkPhase.CONNECTED &&
                baby.controller.baby.value.link.phase == LinkPhase.CONNECTED
        }
        await("name shown again") { baby.controller.baby.value.parentName == "Pixel 8" }
        assertEquals(false, baby.controller.baby.value.securityWarning)
    }

    @Test
    fun aParentThatLostTheSecretIsRefusedAndTheBabyWarns() {
        val (baby, parent) = pair("Pixel 8")
        await("secret kept") { parent.store.load()?.parentSecretB64url != null }
        parent.controller.stopMonitoring()
        await("baby sees stop") { baby.controller.baby.value.link.phase == LinkPhase.PEER_STOPPED }
        parent.store.save(parent.store.load()!!.copy(parentSecretB64url = null))
        parent.controller.startMonitoring()
        await("baby warns") { baby.controller.baby.value.securityWarning }
        Thread.sleep(2_000)
        assertTrue(baby.controller.baby.value.link.phase != LinkPhase.CONNECTED)
        assertNull(baby.controller.baby.value.parentName)
        assertTrue(parent.controller.parent.value.link.phase != LinkPhase.CONNECTED)
    }

    @Test
    fun recoversFromBrokenConnection() {
        val (_, parent) = pair()
        val quietBefore = parent.controller.parent.value.quietSinceWallMs
        val created = net.attemptsCreated.get()
        net.breakAll()
        await("new attempts") { net.attemptsCreated.get() > created }
        await("parent connected again") {
            parent.controller.parent.value.link.phase == LinkPhase.CONNECTED &&
                net.liveAttempts.size == 2
        }
        // The baby's summary says nothing was missed, so "quiet since" returns to the earlier time.
        await("quiet restored") { parent.controller.parent.value.quietSinceWallMs == quietBefore }
        assertTrue(parent.sink.lost.isEmpty())
    }

    @Test
    fun parentStopIsNotAnOutageOnTheBaby() {
        val (baby, parent) = pair()
        parent.controller.stopMonitoring()
        await("baby sees stop") { baby.controller.baby.value.link.phase == LinkPhase.PEER_STOPPED }
        assertNull(baby.controller.baby.value.link.failureKey)
        assertEquals(LinkPhase.IDLE, parent.controller.parent.value.link.phase)
    }

    @Test
    fun wrongCodeFailsPairingWithoutSaving() {
        val baby = unit("baby", Role.BABY)
        val parent = unit("parent", Role.PARENT)
        baby.controller.startPairing()
        val other = JSONObject(baby.controller.pairing.value!!.codePayload!!)
        other.put("k", org.guardedsleep.app.pairing.PairingPayload.b64(ByteArray(32) { 7 }))
        parent.controller.submitPairingCode(other.toString())
        // Different PSK means a different room: nobody to talk to, nothing saved, nothing verified.
        Thread.sleep(1500)
        assertTrue(!parent.store.exists())
        assertEquals(PairingPhase.CONNECTING, parent.controller.pairing.value?.phase)
    }

    @Test
    fun refusedForegroundServiceIsVisibleAndAlarmsTheParent() {
        val parent = unit("parent", Role.PARENT, hostThrows = true)
        parent.store.save(
            org.guardedsleep.app.pairing.StoredPairing(
                org.guardedsleep.app.pairing.PairingPayload.b64(ByteArray(32) { 3 }),
                listOf("wss://fake.example"),
                true,
            )
        )
        assertTrue(parent.controller.app.value.backgroundProtection)
        parent.controller.startMonitoring()
        assertEquals(false, parent.controller.app.value.backgroundProtection)
        assertEquals(true, parent.sink.fgsLost.last())
        // A later successful start clears it.
        parent.controller.onServiceRunning(true)
        assertTrue(parent.controller.app.value.backgroundProtection)
        assertEquals(false, parent.sink.fgsLost.last())
    }

    @Test
    fun systemStoppingTheServiceTurnsRed() {
        val (baby, parent) = pair()
        parent.controller.onServiceRunning(false)
        assertEquals(false, parent.controller.app.value.backgroundProtection)
        assertEquals(true, parent.sink.fgsLost.last())
        baby.controller.onServiceRunning(false)
        assertEquals(false, baby.controller.app.value.backgroundProtection)
        // The baby unit never sounds an alarm.
        assertTrue(baby.sink.fgsLost.isEmpty())
        parent.controller.stopMonitoring()
        assertTrue(parent.controller.app.value.backgroundProtection)
    }

    private fun storedPairing() =
        org.guardedsleep.app.pairing.StoredPairing(
            org.guardedsleep.app.pairing.PairingPayload.b64(ByteArray(32) { 3 }),
            listOf("wss://fake.example"),
            true,
        )

    @Test
    fun missingPairingWhileMonitoringIsReportedNotSilentlySkipped() {
        val parent = unit("parent", Role.PARENT)
        parent.controller.startMonitoring()
        assertEquals(LinkController.EnsureRun.NO_PAIRING, parent.controller.ensureRun())
        // The service reports it through the same platform path as a lost foreground service.
        parent.controller.onServiceRunning(false)
        assertEquals(false, parent.controller.app.value.backgroundProtection)
        assertEquals(true, parent.sink.fgsLost.last())
    }

    @Test
    fun transientlyUnreadablePairingIsKeptAndTheRetryResumesTheRun() {
        val cipher = FlakyCipher(1) { java.security.KeyStoreException("keystore busy") }
        val parent = unit("parent", Role.PARENT, pairingCipher = cipher)
        parent.store.save(storedPairing())
        assertEquals(LinkController.EnsureRun.UNREADABLE, parent.controller.ensureRun())
        assertTrue(parent.store.exists())
        assertEquals(LinkController.EnsureRun.RUNNING, parent.controller.ensureRun())
    }

    @Test
    fun unrecoverablePairingIsClearedAndReportedAsMissing() {
        val cipher = FlakyCipher(1) { javax.crypto.AEADBadTagException() }
        val parent = unit("parent", Role.PARENT, pairingCipher = cipher)
        parent.store.save(storedPairing())
        assertEquals(LinkController.EnsureRun.NO_PAIRING, parent.controller.ensureRun())
        assertFalse(parent.store.exists())
    }
}
