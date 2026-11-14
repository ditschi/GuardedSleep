package org.guardedsleep.app.fixtures

import java.io.File
import java.security.SecureRandom
import java.util.Base64
import java.util.concurrent.CopyOnWriteArrayList
import org.guardedsleep.app.link.LinkController
import org.guardedsleep.app.link.MonitorHost
import org.guardedsleep.app.link.NetworkWatcher
import org.guardedsleep.app.pairing.TurnStore
import org.guardedsleep.app.signaling.SignalingFactory
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.Role

/**
 * A baby phone and a parent phone, each a real [LinkController] with its real core, joined by an
 * in-memory relay and fake media on one [ManualScheduler]. Only signaling, media, audio, time and
 * system services are replaced. The public surface is contract types only, so UI tests built on it
 * never see a UniFFI type.
 *
 * Nothing moves until the test thread calls [advance] or [runUntilIdle].
 */
class TestPhones private constructor(dir: File) {
    private val scheduler = ManualScheduler()
    private val net = FakeNet(scheduler)
    private val babyRandom = RecordingRandom(BABY_SEED)
    private val babyMedia = CopyOnWriteArrayList<FakeMedia>()
    private val phones = mutableListOf<LinkController>()

    val baby: AppController = phone(File(dir, "baby").apply { mkdirs() }, Role.BABY, babyRandom)
    val parent: AppController =
        phone(File(dir, "parent").apply { mkdirs() }, Role.PARENT, RecordingRandom(PARENT_SEED))

    /**
     * Every form a pairing secret of this run could take in a log line: raw 32-byte draws of the
     * baby's random source as Base64URL (with and without padding), standard Base64 and hex, in
     * both cases. The harness fails a run that finds one of them in the logs.
     */
    val pskForms: List<String>
        get() =
            babyRandom.drawn
                .filter { it.size == PSK_BYTES }
                .flatMap { b ->
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

    /** The virtual monotonic clock (what the contract's elapsed timestamps are measured on). */
    fun elapsedMs(): Long = scheduler.elapsedMs()

    /** The virtual wall clock. */
    fun wallMs(): Long = scheduler.wallMs()

    /** Virtual time moves by [ms]; everything due runs in order. */
    fun advance(ms: Long) = scheduler.advanceBy(ms)

    /** Runs what is due now (deliveries, posted work), without moving time. */
    fun runUntilIdle() = scheduler.runUntilIdle()

    /** Steps virtual time until [cond] holds; fails after [timeoutMs] of virtual time. */
    fun runUntil(what: String, timeoutMs: Long = 15_000, cond: () -> Boolean) =
        scheduler.runUntil(what, timeoutMs, cond = cond)

    /** The baby's microphone hears [rms] (0.0 to 1.0) until [stopNoise]. */
    fun injectNoise(rms: Double = NOISE_RMS) {
        babyMedia.forEach { it.micRms = rms }
        noiseRms = rms
    }

    fun stopNoise() {
        babyMedia.forEach { it.micRms = null }
        noiseRms = null
    }

    /** Both ends of every live connection fail at once, like a Wi-Fi outage. */
    fun dropNetwork() = net.breakAll()

    fun close() {
        phones.forEach { runCatching { it.stopMonitoring() } }
        scheduler.runUntilIdle()
    }

    @Volatile private var noiseRms: Double? = null

    private fun phone(dir: File, role: Role, random: SecureRandom): AppController {
        val settings = testSettings(dir)
        val pairings = rolePairings(dir, role)
        lateinit var controller: LinkController
        controller =
            LinkController(
                settingsRepo = settings,
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
                        }
                    }
                },
                silence = FakeSilence(),
                alerts = RecordingSink(),
                readiness = { emptyList() },
                host =
                    object : MonitorHost {
                        override fun start() {
                            controller.ensureRun()
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
                deviceCheckIo = scheduler.executor(),
                turnIo = scheduler.executor(),
            )
        // The relay every pairing uses is the fake one; a public default would never connect.
        controller.updateSettings { it.copy(signalingUrls = listOf("wss://fake.example")) }
        phones += controller
        return controller
    }

    companion object {
        fun create(dir: File): TestPhones = TestPhones(dir)

        private const val PSK_BYTES = 32
        private const val BABY_SEED = 0x6275_6279L
        private const val PARENT_SEED = 0x7061_7265L
        private const val NOISE_RMS = 0.3
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
