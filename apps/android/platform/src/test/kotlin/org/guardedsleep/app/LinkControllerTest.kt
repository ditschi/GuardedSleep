package org.guardedsleep.app

import java.security.SecureRandom
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicInteger
import org.guardedsleep.app.config.PublicLists
import org.guardedsleep.app.fixtures.FakeCipher
import org.guardedsleep.app.fixtures.FakeMedia
import org.guardedsleep.app.fixtures.FakeNet
import org.guardedsleep.app.fixtures.FakeSilence
import org.guardedsleep.app.fixtures.FakeTransport
import org.guardedsleep.app.fixtures.ManualScheduler
import org.guardedsleep.app.fixtures.RecordingSink
import org.guardedsleep.app.fixtures.rolePairings
import org.guardedsleep.app.fixtures.testSettings
import org.guardedsleep.app.link.Clock
import org.guardedsleep.app.link.LinkController
import org.guardedsleep.app.link.MonitorHost
import org.guardedsleep.app.link.NetworkWatcher
import org.guardedsleep.app.link.TalkGate
import org.guardedsleep.app.link.buildIceConfig
import org.guardedsleep.app.pairing.PairingCipher
import org.guardedsleep.app.pairing.PairingPayload
import org.guardedsleep.app.pairing.PairingStore
import org.guardedsleep.app.pairing.RolePairings
import org.guardedsleep.app.pairing.StoredMailbox
import org.guardedsleep.app.pairing.StoredPairing
import org.guardedsleep.app.pairing.TurnCredentials
import org.guardedsleep.app.pairing.TurnStore
import org.guardedsleep.app.readiness.BatterySnapshot
import org.guardedsleep.app.readiness.DeviceCheckEnv
import org.guardedsleep.app.readiness.DeviceChecker
import org.guardedsleep.app.settings.SettingsRepository
import org.guardedsleep.app.signaling.SignalingFactory
import org.guardedsleep.app.ui.model.DeviceCheck
import org.guardedsleep.app.ui.model.DeviceCheckReason
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.MailboxReach
import org.guardedsleep.app.ui.model.PairingPhase
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.TalkPermission
import org.guardedsleep.app.ui.model.TalkUi
import org.guardedsleep.app.ui.model.TalkUnavailable
import org.guardedsleep.app.ui.model.TurnSaveResult
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

/** A healthy, charging phone with a microphone that hears something; tests change one fact. */
private class FakeDeviceEnv : DeviceCheckEnv {
    var reachable = true
    var reachableByCall: List<Boolean>? = null
    var gate: CountDownLatch? = null
    val probed = CopyOnWriteArrayList<String>()
    val micSamples = AtomicInteger()

    override fun micGranted() = true

    override fun battery() = BatterySnapshot(80, charging = true, unhealthy = false)

    override fun batteryOptimizationIgnored() = true

    override fun dndAccessGranted() = true

    /** Blocks in the sample until cancelled when set, like a long AudioRecord read loop. */
    var micBlocks = false
    val micStarted = CountDownLatch(1)
    val micCancelled = CountDownLatch(1)

    override fun sampleMicRms(ms: Long, cancelled: () -> Boolean): Double? {
        micSamples.incrementAndGet()
        micStarted.countDown()
        if (micBlocks) {
            val end = System.currentTimeMillis() + 10_000
            while (System.currentTimeMillis() < end) {
                if (cancelled()) {
                    micCancelled.countDown()
                    return null
                }
                Thread.sleep(10)
            }
        }
        return 0.01
    }

    override fun probeMailbox(url: String, timeoutMs: Long): Boolean {
        val call = probed.size
        probed += url
        gate?.await()
        return reachableByCall?.getOrNull(call) ?: reachable
    }
}

private val projectUrl = "wss://${PublicLists.projectServer.host}/signaling"

private const val PIN_TYPE = "\"parent_pin\""

class LinkControllerTest {
    @get:Rule val tmp = TemporaryFolder()
    private val scheduler = ManualScheduler()
    private val net = FakeNet(scheduler)
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
        talkGate: TalkGate = TalkGate.Open,
        hostStarts: java.util.concurrent.atomic.AtomicInteger? = null,
        deviceChecker: DeviceChecker? = null,
        clock: Clock = scheduler,
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
                    hostStarts?.incrementAndGet()
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
                clock = clock,
                newExecutor = { scheduler.executor() },
                turnIo = scheduler.executor(),
                deviceCheckIo =
                    // The device-check doubles block on real latches: those tests keep a thread.
                    if (deviceChecker == null) scheduler.executor()
                    else Executors.newSingleThreadExecutor { r -> Thread(r, "test-device-check") },
                random = SecureRandom(),
                deviceName = { deviceName },
                talkGate = talkGate,
                deviceChecker = deviceChecker,
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
        scheduler.advanceBy(600)
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
        scheduler.advanceBy(300)
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
            PairingPayload.parse(code, scheduler.wallMs()) as PairingPayload.Companion.Result.Ok
        assertEquals(listOf("wss://own.example"), parsed.payload.signalingUrls)
        baby.controller.updateSettings { it.copy(onlyLocal = false) }
        baby.controller.startPairing()
        val all =
            PairingPayload.parse(
                baby.controller.pairing.value!!.codePayload!!,
                scheduler.wallMs(),
            ) as PairingPayload.Companion.Result.Ok
        assertEquals(listOf(projectUrl, "wss://own.example"), all.payload.signalingUrls)
    }

    @After
    fun tearDown() {
        controllers.forEach { it.stopMonitoring() }
        scheduler.runUntilIdle()
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
    fun twoSavedPairingsListTheDefaultPreselectedAndOneChoicePerSession() {
        val u = unit("choice", Role.PARENT)
        val c = u.controller
        u.pairings.store(Role.PARENT, "1").save(pairing("home-psk"))
        u.pairings.store(Role.PARENT, "2").save(pairing("travel-psk"))
        u.pairings.rename(Role.PARENT, "1", "Home")
        u.pairings.rename(Role.PARENT, "2", "Travel")
        c.chooseRole(Role.PARENT)

        val app = c.app.value
        assertEquals(listOf("1", "2"), app.connections.map { it.id })
        assertEquals("1", app.connections.single { it.selected }.id)
        assertEquals("Home", app.peerName)
        assertFalse("at the cap", app.canAddPairing)

        c.selectConnection("2")
        assertEquals("Travel", c.app.value.peerName)
        assertTrue(u.pairings.store(Role.PARENT, "1").exists())
        assertEquals("the default is unchanged", "1", u.pairings.defaultId(Role.PARENT))

        c.startMonitoring()
        c.selectConnection("1")
        assertEquals("not while monitoring", "2", c.app.value.connections.single { it.selected }.id)
        c.stopMonitoring()
        assertEquals(
            "the next start preselects the default",
            "1",
            c.app.value.connections.single { it.selected }.id,
        )
    }

    @Test
    fun theParentStartPairingOpensTheScanScreenUntilTheCap() {
        val u = unit("parent-scan", Role.PARENT)
        val c = u.controller
        c.chooseRole(Role.PARENT)
        c.startPairing()
        assertEquals(PairingPhase.SCANNING, c.pairing.value?.phase)
        assertFalse("no run starts before a code", c.app.value.monitoring)
        c.cancelPairing()
        assertNull(c.pairing.value)

        u.pairings.store(Role.PARENT, "1").save(pairing("one"))
        u.pairings.store(Role.PARENT, "2").save(pairing("two"))
        c.startPairing()
        assertNull("at the cap nothing opens", c.pairing.value)
    }

    @Test
    fun theParentStartPairingStopsAnActiveMonitoringFirst() {
        val u = unit("parent-scan-stop", Role.PARENT)
        val c = u.controller
        u.pairings.store(Role.PARENT, "1").save(pairing("one"))
        c.chooseRole(Role.PARENT)
        c.startMonitoring()
        assertTrue(c.app.value.monitoring)
        c.startPairing()
        assertFalse(c.app.value.monitoring)
        assertEquals(PairingPhase.SCANNING, c.pairing.value?.phase)

        // At the cap nothing changes, monitoring included.
        u.pairings.store(Role.PARENT, "2").save(pairing("two"))
        c.cancelPairing()
        c.startMonitoring()
        c.startPairing()
        assertTrue(c.app.value.monitoring)
        assertNull(c.pairing.value)
    }

    @Test
    fun aSubmittedCodeAfterTheScanStillPairs() {
        val u = unit("parent-scan-submit", Role.PARENT)
        val c = u.controller
        c.chooseRole(Role.PARENT)
        c.startPairing()
        c.submitPairingCode("not a code")
        assertEquals(PairingPhase.FAILED, c.pairing.value?.phase)
    }

    /**
     * A parent phone with two pairings, the second marked, onboarding done. [startsAsParent] is the
     * start mode "Parent"; false is "Ask every time" (a cold start shows the role screen).
     */
    private fun autoStartParent(
        name: String,
        mark: Boolean = true,
        startsAsParent: Boolean = true,
    ): Unit2 {
        val u = unit(name, Role.PARENT)
        u.pairings.store(Role.PARENT, "1").save(pairing("one"))
        u.pairings.store(Role.PARENT, "2").save(pairing("two"))
        if (mark) u.pairings.setDefault(Role.PARENT, "2")
        u.controller.completeOnboarding()
        if (startsAsParent) u.controller.setDefaultRole(Role.PARENT)
        return u
    }

    /** The same phone after a cold start; [role] null keeps whatever the settings persisted. */
    private fun restart(
        name: String,
        of: Unit2,
        starts: java.util.concurrent.atomic.AtomicInteger,
    ) = unit(name, null, settings = of.settings, pairings = of.pairings, hostStarts = starts)

    @Test
    fun aMarkedParentDefaultStartsOnceAndNotAgainAfterStopOrRotation() {
        val starts = java.util.concurrent.atomic.AtomicInteger()
        val c = restart("auto-once2", autoStartParent("auto-once"), starts).controller
        c.onForeground()
        assertTrue(c.app.value.monitoring)
        assertEquals("2", c.app.value.connections.single { it.selected }.id)
        assertEquals("one host start, no resume start on top", 1, starts.get())
        c.stopMonitoring()
        c.onForeground()
        assertFalse("rotation or resume after a stop", c.app.value.monitoring)
        assertEquals(1, starts.get())
    }

    @Test
    fun askEveryTimeStartsOnTheFirstParentChoiceOnly() {
        val first = autoStartParent("ask", startsAsParent = false)
        val starts = java.util.concurrent.atomic.AtomicInteger()
        val c = restart("ask2", first, starts).controller
        c.onForeground()
        assertFalse("the role screen waits", c.app.value.monitoring)
        c.chooseRole(Role.PARENT)
        assertTrue(c.app.value.monitoring)
        assertEquals(1, starts.get())
        c.stopMonitoring()
        c.onForeground()
        assertFalse(c.app.value.monitoring)
        assertEquals(1, starts.get())
    }

    @Test
    fun aBabyStartModeSwitchedToParentWithoutABabyRunStartsOnce() {
        val first = autoStartParent("baby-mode", startsAsParent = false)
        first.controller.setDefaultRole(Role.BABY)
        val starts = java.util.concurrent.atomic.AtomicInteger()
        val c = restart("baby-mode2", first, starts).controller
        c.onForeground()
        assertEquals(Role.BABY, c.app.value.role)
        assertFalse(c.app.value.monitoring)
        c.switchMode()
        c.chooseRole(Role.PARENT)
        assertTrue(c.app.value.monitoring)
        assertTrue(c.app.value.autoStarted)
        assertEquals(1, starts.get())
        c.stopMonitoring()
        assertFalse(c.app.value.autoStarted)
        c.switchMode()
        c.chooseRole(Role.PARENT)
        c.onForeground()
        assertFalse("once per process", c.app.value.monitoring)
        assertEquals(1, starts.get())
    }

    @Test
    fun anInviteShownByTheBabyFirstUsesUpTheAutomaticStart() {
        val first = autoStartParent("baby-first", startsAsParent = false)
        first.controller.setDefaultRole(Role.BABY)
        val starts = java.util.concurrent.atomic.AtomicInteger()
        val c = restart("baby-first2", first, starts).controller
        c.onForeground()
        c.startPairing()
        assertNotNull(c.pairing.value)
        c.cancelPairing()
        c.switchMode()
        c.chooseRole(Role.PARENT)
        assertFalse(c.app.value.monitoring)
        assertEquals(0, starts.get())
    }

    @Test
    fun chooseAnotherConnectionStopsAnAutomaticStart() {
        val first = autoStartParent("choose-other")
        val c =
            restart("choose-other2", first, java.util.concurrent.atomic.AtomicInteger()).controller
        c.onForeground()
        assertTrue(c.app.value.autoStarted)
        c.chooseAnotherConnection()
        assertFalse(c.app.value.monitoring)
        assertFalse(c.app.value.autoStarted)
        c.selectConnection("1")
        c.startMonitoring()
        assertTrue(c.app.value.monitoring)
        assertFalse("a manual start is not automatic", c.app.value.autoStarted)
    }

    @Test
    fun noAutomaticStartWithoutMarkOrOnboardingOrForTheBabyOrWhilePairing() {
        val noMark =
            restart(
                "auto-nomark2",
                autoStartParent("auto-nomark", mark = false),
                java.util.concurrent.atomic.AtomicInteger(),
            )
        noMark.controller.onForeground()
        assertFalse(noMark.controller.app.value.monitoring)

        val fresh = unit("auto-fresh", Role.PARENT)
        fresh.pairings.store(Role.PARENT, "1").save(pairing("one"))
        fresh.pairings.setDefault(Role.PARENT, "1")
        fresh.controller.setDefaultRole(Role.PARENT)
        val freshRestart =
            restart("auto-fresh2", fresh, java.util.concurrent.atomic.AtomicInteger())
        freshRestart.controller.onForeground()
        assertFalse(freshRestart.controller.app.value.monitoring)

        val baby = unit("auto-baby", Role.BABY)
        baby.pairings.store(Role.BABY, "1").save(pairing("one"))
        baby.pairings.setDefault(Role.BABY, "1")
        baby.controller.completeOnboarding()
        baby.controller.setDefaultRole(Role.BABY)
        val babyRestart = restart("auto-baby2", baby, java.util.concurrent.atomic.AtomicInteger())
        babyRestart.controller.onForeground()
        assertFalse(babyRestart.controller.app.value.monitoring)

        val pairing = autoStartParent("auto-pairing")
        pairing.pairings.forget(Role.PARENT, "1")
        val pr = restart("auto-pairing2", pairing, java.util.concurrent.atomic.AtomicInteger())
        pr.controller.startPairing()
        pr.controller.onForeground()
        assertFalse(pr.controller.app.value.monitoring)
    }

    @Test
    fun clearingTheDefaultStopsTheAutomaticStart() {
        val u =
            restart(
                "auto-clear2",
                autoStartParent("auto-clear"),
                java.util.concurrent.atomic.AtomicInteger(),
            )
        u.controller.clearDefaultConnection()
        assertTrue(u.controller.app.value.connections.none { it.isDefault })
        u.controller.onForeground()
        assertFalse(u.controller.app.value.monitoring)
    }

    @Test
    fun aRestartWhileMonitoringKeepsTheChosenConnection() {
        val first = unit("resume-choice", Role.PARENT)
        first.pairings.store(Role.PARENT, "1").save(pairing("home-psk"))
        first.pairings.store(Role.PARENT, "2").save(pairing("travel-psk"))
        first.controller.chooseRole(Role.PARENT, makeDefault = true)
        first.controller.selectConnection("2")
        first.controller.startMonitoring()

        val second =
            unit("resume-choice2", null, settings = first.settings, pairings = first.pairings)
        assertEquals("2", second.controller.app.value.connections.single { it.selected }.id)
        second.controller.stopMonitoring()

        val third =
            unit("resume-choice3", null, settings = first.settings, pairings = first.pairings)
        assertEquals("1", third.controller.app.value.connections.single { it.selected }.id)
    }

    @Test
    fun renameAndDefaultAndForgetWorkPerPairing() {
        val u = unit("manage", Role.PARENT)
        val c = u.controller
        u.pairings.store(Role.PARENT, "1").save(pairing("one"))
        u.pairings.store(Role.PARENT, "2").save(pairing("two"))
        c.chooseRole(Role.PARENT)

        assertTrue(c.renameConnection("2", "Travel"))
        assertFalse(c.renameConnection("2", ""))
        c.setDefaultConnection("2")
        assertEquals("2", c.app.value.connections.single { it.isDefault }.id)

        // The selected one is the default now; forgetting the other keeps monitoring untouched.
        c.forgetConnection("1")
        assertEquals(listOf("2"), c.app.value.connections.map { it.id })
        assertEquals("two", u.pairings.store(Role.PARENT, "2").load()?.pskB64url)
        assertTrue(c.app.value.canAddPairing)

        c.forgetConnection("2")
        assertFalse(c.app.value.paired)
    }

    @Test
    fun forgettingTheActiveConnectionStopsMonitoringFirst() {
        val u = unit("forget-active", Role.PARENT)
        val c = u.controller
        u.pairings.store(Role.PARENT, "1").save(pairing("one"))
        u.pairings.store(Role.PARENT, "2").save(pairing("two"))
        c.chooseRole(Role.PARENT)
        // Not the default: the one in use is deleted, never the default by mistake.
        c.selectConnection("2")
        c.startMonitoring()
        c.forgetConnection("2")
        assertFalse(c.app.value.monitoring)
        assertEquals(listOf("1"), c.app.value.connections.map { it.id })
        assertEquals("one", u.pairings.store(Role.PARENT, "1").load()?.pskB64url)
    }

    @Test
    fun aBabyWithTwoParentPairingsUsesTheDefaultOne() {
        val u = unit("baby-two", Role.BABY)
        u.pairings.store(Role.BABY, "1").save(pairing("one"))
        u.pairings.store(Role.BABY, "2").save(pairing("two"))
        u.pairings.setDefault(Role.BABY, "2")
        u.controller.chooseRole(Role.BABY)
        assertEquals("2", u.controller.app.value.connections.single { it.selected }.id)
        assertEquals(LinkController.EnsureRun.RUNNING, u.controller.ensureRun())
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

    // ---- baby device check ---------------------------------------------------------------------

    @Test
    fun theDeviceCheckIsUnknownWhileRunningThenPublishesTheResult() {
        val env = FakeDeviceEnv().apply { gate = CountDownLatch(1) }
        val baby = unit("dc-run", Role.BABY, deviceChecker = DeviceChecker(env))
        baby.controller.runDeviceCheck()
        assertEquals(DeviceCheck.UNKNOWN, baby.controller.app.value.deviceCheck)
        env.gate!!.countDown()
        awaitReal("the result") { baby.controller.app.value.deviceCheck != DeviceCheck.UNKNOWN }
        val app = baby.controller.app.value
        assertEquals(DeviceCheck.OK, app.deviceCheck)
        // The invite set (settings list) is probed; only the host reaches the state.
        assertEquals(listOf("wss://fake.example"), env.probed.toList())
        assertEquals(listOf(MailboxReach("fake.example", true)), app.deviceCheckMailboxes)
    }

    @Test
    fun anUnreachableMailboxIsNotRecommendedAndStillAllowsMonitoring() {
        val env = FakeDeviceEnv().apply { reachable = false }
        val baby = unit("dc-none", Role.BABY, deviceChecker = DeviceChecker(env))
        baby.controller.runDeviceCheck()
        awaitReal("the result") { baby.controller.app.value.deviceCheck != DeviceCheck.UNKNOWN }
        assertEquals(DeviceCheck.NOT_RECOMMENDED, baby.controller.app.value.deviceCheck)
        assertEquals(
            listOf(DeviceCheckReason.NO_MAILBOX_REACHABLE),
            baby.controller.app.value.deviceCheckReasons,
        )
    }

    @Test
    fun aStaleDeviceCheckResultIsDropped() {
        val env = FakeDeviceEnv().apply { gate = CountDownLatch(1) }
        val baby = unit("dc-stale", Role.BABY, deviceChecker = DeviceChecker(env))
        // The first run blocks on the gate and would say "unreachable"; the second is newer.
        env.reachableByCall = listOf(false, true)
        baby.controller.runDeviceCheck()
        awaitReal("first probe started") { env.probed.size == 1 }
        baby.controller.runDeviceCheck(force = true)
        env.gate!!.countDown()
        // The one worker ran the dropped first check before the second, so this is the second's.
        awaitReal("the newer result") {
            baby.controller.app.value.deviceCheck != DeviceCheck.UNKNOWN
        }
        assertEquals(DeviceCheck.OK, baby.controller.app.value.deviceCheck)
        assertEquals(
            listOf(true),
            baby.controller.app.value.deviceCheckMailboxes.map { it.reachable },
        )
    }

    @Test
    fun theParentDoesNotRunADeviceCheck() {
        val env = FakeDeviceEnv()
        val parent = unit("dc-parent", Role.PARENT, deviceChecker = DeviceChecker(env))
        parent.controller.runDeviceCheck()
        assertEquals(0, env.probed.size)
        assertEquals(DeviceCheck.UNKNOWN, parent.controller.app.value.deviceCheck)
    }

    @Test
    fun theMicrophoneIsNotSampledWhileARunHoldsIt() {
        val env = FakeDeviceEnv()
        val baby = unit("dc-mic", Role.BABY, deviceChecker = DeviceChecker(env))
        baby.controller.runDeviceCheck()
        awaitReal("idle result") { baby.controller.app.value.deviceCheck != DeviceCheck.UNKNOWN }
        assertEquals(1, env.micSamples.get())

        // A saved record: its stored relay set is probed, not the settings list.
        seedRecord(baby, listOf(StoredMailbox("relay", "wss://own.example")))
        baby.controller.startMonitoring()
        awaitReal("the run started") { baby.controller.app.value.monitoring }
        env.probed.clear()
        baby.controller.runDeviceCheck()
        awaitReal("running result") { env.probed.isNotEmpty() }
        awaitReal("published") { baby.controller.app.value.deviceCheckMailboxes.size == 1 }
        assertEquals(1, env.micSamples.get())
        assertEquals(listOf("wss://own.example"), env.probed.toList())
        assertEquals("own.example", baby.controller.app.value.deviceCheckMailboxes[0].host)
    }

    @Test
    fun aFreshResultIsReusedAndCheckAgainForcesANewOne() {
        val env = FakeDeviceEnv()
        val clock = FakeClock()
        val baby = unit("dc-reuse", Role.BABY, deviceChecker = DeviceChecker(env), clock = clock)
        baby.controller.runDeviceCheck()
        awaitReal("the result") { baby.controller.app.value.deviceCheck != DeviceCheck.UNKNOWN }
        assertEquals(1, env.probed.size)

        // A resume right after: no new probes and no "Checking..." state.
        clock.advance(59_000)
        baby.controller.runDeviceCheck()
        assertEquals(DeviceCheck.OK, baby.controller.app.value.deviceCheck)
        assertEquals(1, env.probed.size)

        // "Check again" forces one.
        baby.controller.runDeviceCheck(force = true)
        awaitReal("the forced probe") { env.probed.size == 2 }
        awaitReal("the forced result") {
            baby.controller.app.value.deviceCheck != DeviceCheck.UNKNOWN
        }

        // An old result is checked anew; an invalidation (role change) clears it.
        clock.advance(61_000)
        baby.controller.runDeviceCheck()
        awaitReal("the aged probe") { env.probed.size == 3 }
        awaitReal("the aged result") {
            baby.controller.app.value.deviceCheck != DeviceCheck.UNKNOWN
        }
        baby.controller.chooseRole(Role.PARENT)
        baby.controller.chooseRole(Role.BABY)
        assertEquals(DeviceCheck.UNKNOWN, baby.controller.app.value.deviceCheck)
        baby.controller.runDeviceCheck()
        awaitReal("the check after the invalidation") { env.probed.size == 4 }
    }

    @Test
    fun startingARunCancelsTheMicSampleAndDropsTheResult() {
        val env =
            FakeDeviceEnv().apply {
                micBlocks = true
                reachable = false
            }
        val baby = unit("dc-cancel", Role.BABY, deviceChecker = DeviceChecker(env))
        seedRecord(baby, listOf(StoredMailbox("relay", "wss://own.example")))
        baby.controller.runDeviceCheck()
        awaitReal("the sample started") { env.micStarted.count == 0L }
        baby.controller.startMonitoring()
        awaitReal("the sample was cancelled") { env.micCancelled.count == 0L }
        // The start dropped the check's state at once; its late result must not come back.
        assertEquals(DeviceCheck.UNKNOWN, baby.controller.app.value.deviceCheck)
        // A later check (the run holds the mic, so it skips the sample) runs after the cancelled
        // one on the same worker and publishes its own result.
        env.reachable = true
        env.probed.clear()
        baby.controller.runDeviceCheck()
        awaitReal("the next result") {
            baby.controller.app.value.deviceCheck != DeviceCheck.UNKNOWN
        }
        assertEquals(1, env.micSamples.get())
        assertEquals(
            listOf(true),
            baby.controller.app.value.deviceCheckMailboxes.map { it.reachable },
        )
    }

    /** Steps virtual time until [cond] holds (no sleeping), at most [timeoutMs] of it. */
    private fun await(what: String, timeoutMs: Long = 15_000, cond: () -> Boolean) =
        scheduler.runUntil(what, timeoutMs, cond = cond)

    /**
     * Real-time wait for the device-check tests only: their doubles block on latches on a real
     * worker thread, so wall time (not the scheduler) has to pass. The scheduler is still pumped.
     */
    private fun awaitReal(what: String, timeoutMs: Long = 15_000, cond: () -> Boolean) {
        val end = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < end) {
            scheduler.runUntilIdle()
            if (cond()) return
            scheduler.advanceBy(5)
        }
        throw AssertionError("timeout waiting for $what")
    }

    private fun pair(
        parentName: String = "",
        parentGate: TalkGate = TalkGate.Open,
    ): Pair<Unit2, Unit2> {
        val baby = unit("baby", Role.BABY)
        val parent = unit("parent", Role.PARENT, deviceName = parentName, talkGate = parentGate)
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
    fun aPressWithoutPermissionSendsNothingAndOffersTheStep() {
        val gate =
            object : TalkGate {
                override fun micPermission() = false

                override fun deviceUnlocked() = true
            }
        val (_, parent) = pair(parentGate = gate)
        val sent = CopyOnWriteArrayList<String>()
        net.onSend = { sent += it }
        parent.controller.setTalk(true)
        assertEquals(TalkPermission.NEEDED, parent.controller.parent.value.talkPermission)
        scheduler.advanceBy(300)
        assertTrue(sent.none { it.contains("talk_request") })
        parent.controller.talkPermissionResult(false)
        assertEquals(TalkPermission.REFUSED, parent.controller.parent.value.talkPermission)
        assertTrue(net.fakeAttempts.all { it.talkMic.isEmpty() })
    }

    @Test
    fun aLockedDeviceSendsNothing() {
        val gate =
            object : TalkGate {
                override fun micPermission() = true

                override fun deviceUnlocked() = false
            }
        val (_, parent) = pair(parentGate = gate)
        val sent = CopyOnWriteArrayList<String>()
        net.onSend = { sent += it }
        parent.controller.setTalk(true)
        scheduler.advanceBy(300)
        assertTrue(sent.none { it.contains("talk_request") })
    }

    @Test
    fun aBabyThatCannotSetTheVolumeSaysMutedAndTheParentShowsItBeforeAPress() {
        val (_, parent) = pair()
        await("the parent learns the baby cannot play") {
            parent.controller.parent.value.talk == TalkUi.Unavailable(TalkUnavailable.MUTED)
        }
        assertTrue(net.fakeAttempts.all { it.talkMic.isEmpty() })
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
        scheduler.advanceBy(3_000)
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
        scheduler.advanceBy(2_000)
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
        scheduler.advanceBy(1500)
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
