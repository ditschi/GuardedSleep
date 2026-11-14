package org.guardedsleep.app.fixtures

import java.io.File
import java.security.SecureRandom
import java.util.Base64
import java.util.concurrent.CopyOnWriteArrayList
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.guardedsleep.app.link.LinkController
import org.guardedsleep.app.link.MonitorHost
import org.guardedsleep.app.link.NetworkWatcher
import org.guardedsleep.app.link.ReadinessProvider
import org.guardedsleep.app.pairing.DefaultPairingMarker
import org.guardedsleep.app.pairing.RolePairings
import org.guardedsleep.app.pairing.StoredPairing
import org.guardedsleep.app.pairing.TurnStore
import org.guardedsleep.app.pairing.storedMailboxesFor
import org.guardedsleep.app.readiness.DeviceChecker
import org.guardedsleep.app.signaling.SignalingFactory
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.ReadinessIssue
import org.guardedsleep.app.ui.model.Role

/**
 * A baby phone and a parent phone, each a real [LinkController] with its real core, joined by an
 * in-memory relay and fake media on one [ManualScheduler]. Only signaling, media, audio, time and
 * system services are replaced. The public surface is contract types only, so UI tests built on it
 * never see a UniFFI type.
 *
 * Nothing moves until the test thread calls [advance].
 */
class TestPhones private constructor(dir: File) {
    private val scheduler = ManualScheduler()
    private val net = FakeNet(scheduler)
    private val babyMedia = CopyOnWriteArrayList<FakeMedia>()
    private val parentMedia = CopyOnWriteArrayList<FakeMedia>()
    @Volatile private var noiseRms: Double? = null

    private inner class Phone(val role: Role, val dir: File, seed: Long) {
        val random = RecordingRandom(seed)
        val sink = RecordingSink()
        val silence = FakeSilence()
        val deviceEnv = FakeDeviceCheckEnv()
        private var scope = CoroutineScope(Dispatchers.Unconfined + SupervisorJob())
        lateinit var controller: LinkController
            private set

        lateinit var pairings: RolePairings
            private set

        /** An app start: a new controller over the files the last one left, then `onForeground`. */
        fun boot(first: Boolean) {
            if (!first) scope.cancel()
            scope = CoroutineScope(Dispatchers.Unconfined + SupervisorJob())
            controller = build()
            if (first) {
                // The relay every pairing uses is the fake one; a public default never connects.
                controller.updateSettings { it.copy(signalingUrls = listOf("wss://fake.example")) }
            }
            // What MainActivity does on start.
            controller.onForeground()
        }

        private fun build(): LinkController {
            lateinit var c: LinkController
            pairings = rolePairings(dir, role, defaults = DefaultPairingMarker.Files(dir))
            c =
                LinkController(
                    settingsRepo = testSettings(dir, scope = scope),
                    // The default marker is a file, so it survives an app restart like on the
                    // phone.
                    pairings = pairings,
                    turnStore = TurnStore(File(dir, "turn.bin"), FakeCipher()),
                    signaling =
                        SignalingFactory { url, l ->
                            net.openedUrls += url
                            FakeTransport(net, l)
                        },
                    mediaFactory = { r, _ ->
                        FakeMedia(net, r == Role.BABY).also {
                            if (r == Role.BABY) {
                                it.micRms = noiseRms
                                babyMedia += it
                            } else {
                                it.remoteRms = noiseRms
                                parentMedia += it
                            }
                        }
                    },
                    silence = silence,
                    alerts = sink,
                    readiness =
                        object : ReadinessProvider {
                            // Something to show: the test alert was never run, the battery is half.
                            override fun check() = listOf(ReadinessIssue.NEVER_TESTED)

                            override fun ownBatteryPct() = OWN_BATTERY_PCT
                        },
                    deviceName = { "Test ${role.name.lowercase()} phone" },
                    host =
                        object : MonitorHost {
                            override fun start() {
                                c.ensureRun()
                            }

                            override fun stop() = Unit
                        },
                    network =
                        object : NetworkWatcher {
                            override fun start(onAvailable: () -> Unit) = Unit

                            override fun stop() = Unit
                        },
                    clock = scheduler,
                    newExecutor = { scheduler.executor() },
                    random = random,
                    deviceChecker = DeviceChecker(deviceEnv, InlineExecutorService()),
                    deviceCheckIo = scheduler.executor(),
                    turnIo = scheduler.executor(),
                )
            return c
        }
    }

    private val babyPhone = Phone(Role.BABY, File(dir, "baby").apply { mkdirs() }, BABY_SEED)
    private val parentPhone =
        Phone(Role.PARENT, File(dir, "parent").apply { mkdirs() }, PARENT_SEED)

    init {
        babyPhone.boot(first = true)
        parentPhone.boot(first = true)
    }

    val baby: AppController
        get() = babyPhone.controller

    val parent: AppController
        get() = parentPhone.controller

    /** The facts the baby's device check sees; change one and run the check again. */
    val babyDevice: FakeDeviceCheckEnv
        get() = babyPhone.deviceEnv

    /**
     * Every form a pairing secret of this run could take in a log line: raw 32-byte draws of the
     * baby's random source as Base64URL (with and without padding), standard Base64 and hex, in
     * both cases. The harness fails a run that finds one of them in the logs.
     */
    val pskForms: List<String>
        get() =
            (babyPhone.random.drawn.filter { it.size == PSK_BYTES } + seededPsks).flatMap { b ->
                val url = Base64.getUrlEncoder()
                val hex = b.joinToString("") { "%02x".format(it) }
                listOf(
                    url.withoutPadding().encodeToString(b),
                    url.encodeToString(b),
                    Base64.getEncoder().encodeToString(b),
                    hex,
                    hex.uppercase(),
                )
            }

    private val seededPsks = CopyOnWriteArrayList<ByteArray>()

    /**
     * Saves the same pairing on both phones as one that never verified its peer (the state of a
     * first connection attempt). Only a link without a verified past can fail terminally, which is
     * what "Pair again" answers. Restart both apps afterwards to load it.
     */
    fun seedUnverifiedPairing() {
        val psk = ByteArray(PSK_BYTES).also(java.util.Random(SEEDED_PSK)::nextBytes)
        seededPsks += psk
        val b64 = Base64.getUrlEncoder().withoutPadding().encodeToString(psk)
        listOf(babyPhone, parentPhone).forEach {
            val id = it.pairings.ids(it.role).first()
            it.pairings
                .store(it.role, id)
                .save(
                    StoredPairing(
                        b64,
                        listOf("wss://fake.example"),
                        false,
                        mailboxes = storedMailboxesFor(listOf("wss://fake.example")),
                    )
                )
        }
    }

    /** Noise alerts the parent phone raised (the full-screen notification would open). */
    val parentNoiseAlerts: Int
        get() = parentPhone.sink.noise.size

    /** Times the parent phone's alert sound was stopped. */
    val parentSoundStops: Int
        get() = parentPhone.sink.soundStopped

    /** Link-loss alarms the parent phone raised. */
    val parentLinkLostAlarms: Int
        get() = parentPhone.sink.lost.size

    /** Every address a mailbox socket was opened at (for failure messages). */
    val openedUrls: List<String>
        get() = net.openedUrls.toList()

    /** The virtual monotonic clock (what the contract's elapsed timestamps are measured on). */
    fun elapsedMs(): Long = scheduler.elapsedMs()

    /** The virtual wall clock. */
    fun wallMs(): Long = scheduler.wallMs()

    /** Virtual time moves by [ms]; everything due runs in order. */
    fun advance(ms: Long) = scheduler.advanceBy(ms)

    /** The baby's microphone hears [rms] (0.0 to 1.0) until [stopNoise]. */
    fun injectNoise(rms: Double = NOISE_RMS) {
        babyMedia.forEach { it.micRms = rms }
        parentMedia.forEach { it.remoteRms = rms }
        noiseRms = rms
    }

    fun stopNoise() {
        babyMedia.forEach { it.micRms = null }
        parentMedia.forEach { it.remoteRms = null }
        noiseRms = null
    }

    /** Both ends of every live connection fail at once and none is made until [restoreNetwork]. */
    fun dropNetwork() {
        net.outage = true
        net.breakAll()
    }

    /**
     * The relay refuses every new join with "unsupported protocol" (a terminal failure for a link
     * that has not verified its peer yet) until it is set back to false.
     */
    fun relayRejectsProtocol(rejects: Boolean) {
        net.rejectProtocol = rejects
    }

    /** The network is back: the cores' next attempts connect again. */
    fun restoreNetwork() {
        net.outage = false
    }

    /**
     * The baby app starts again (process restart): a new controller over the saved files. Only
     * after monitoring was stopped, since the old controller is not shut down.
     */
    fun restartBaby() = babyPhone.boot(first = false)

    /** As [restartBaby], for the parent app. */
    fun restartParent() = parentPhone.boot(first = false)

    fun close() {
        listOf(babyPhone, parentPhone).forEach { runCatching { it.controller.stopMonitoring() } }
        scheduler.runUntilIdle()
    }

    companion object {
        fun create(dir: File): TestPhones = TestPhones(dir)

        private const val PSK_BYTES = 32
        private const val BABY_SEED = 0x6275_6279L
        private const val SEEDED_PSK = 0x7365_6564L
        private const val PARENT_SEED = 0x7061_7265L
        private const val NOISE_RMS = 0.3
        private const val OWN_BATTERY_PCT = 57
    }
}

/** Deterministic and recording: the first 32-byte draw of the baby is its pairing secret. */
private class RecordingRandom(seed: Long) : SecureRandom() {
    private val inner = java.util.Random(seed)
    val drawn = CopyOnWriteArrayList<ByteArray>()

    override fun nextBytes(bytes: ByteArray) {
        inner.nextBytes(bytes)
        drawn += bytes.copyOf()
    }

    override fun nextLong() = inner.nextLong()
}
