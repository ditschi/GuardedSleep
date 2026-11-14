package org.guardedsleep.app.fixtures

import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.guardedsleep.app.alerts.AlertSink
import org.guardedsleep.app.alerts.LinkSnapshot
import org.guardedsleep.app.alerts.NoiseKind
import org.guardedsleep.app.alerts.Notice
import org.guardedsleep.app.pairing.PairingCipher
import org.guardedsleep.app.settings.SettingsRepository

/** Settings on a DataStore file in [dir]; writes complete on the caller's thread. */
fun testSettings(
    dir: File,
    name: String = "s",
    scope: CoroutineScope = CoroutineScope(Dispatchers.Unconfined),
): SettingsRepository {
    return SettingsRepository(
        PreferenceDataStoreFactory.create(scope = scope) { File(dir, "$name.preferences_pb") },
        scope,
    )
}

/** Reversible, not secure: only for tests of the file handling. */
class FakeCipher : PairingCipher {
    var keyDeleted = false

    override fun encrypt(plain: ByteArray) =
        plain.map { (it.toInt() xor 0x5a).toByte() }.toByteArray()

    override fun decrypt(sealed: ByteArray) = encrypt(sealed)

    override fun deleteKey() {
        keyDeleted = true
    }
}

class RecordingSink : AlertSink {
    val snapshots = mutableListOf<LinkSnapshot>()
    val lost = mutableListOf<Long>()
    val noise = mutableListOf<Triple<NoiseKind, Boolean, Boolean>>()
    val noiseDiscreet = mutableListOf<Boolean>()
    val notices = mutableListOf<Notice>()
    val fgsLost = mutableListOf<Boolean>()
    var ended = 0
    var soundStopped = 0
    var cleared = 0

    override fun linkState(snapshot: LinkSnapshot) {
        snapshots += snapshot
    }

    override fun linkLost(downMs: Long) {
        lost += downMs
    }

    override fun noiseAlert(kind: NoiseKind, probe: Boolean, snoozed: Boolean, discreet: Boolean) {
        noise += Triple(kind, probe, snoozed)
        noiseDiscreet += discreet
    }

    override fun noiseEnded() {
        ended++
    }

    override fun stopNoiseSound() {
        soundStopped++
    }

    override fun notice(notice: Notice) {
        notices += notice
    }

    override fun backgroundProtection(lost: Boolean) {
        fgsLost += lost
    }

    override fun clear() {
        cleared++
    }
}

/** A healthy, charging phone whose microphone hears something; a journey changes one fact. */
class FakeDeviceCheckEnv : org.guardedsleep.app.readiness.DeviceCheckEnv {
    @Volatile var reachable = true
    @Volatile var micRms: Double? = 0.01
    val probed = java.util.concurrent.CopyOnWriteArrayList<String>()

    override fun micGranted() = true

    override fun battery() =
        org.guardedsleep.app.readiness.BatterySnapshot(80, charging = true, unhealthy = false)

    override fun batteryOptimizationIgnored() = true

    override fun dndAccessGranted() = true

    override fun sampleMicRms(ms: Long, cancelled: () -> Boolean): Double? = micRms

    override fun probeMailbox(url: String, timeoutMs: Long): Boolean {
        probed += url
        return reachable
    }
}

/**
 * Runs every task at once on the calling thread. The device checker blocks on its probes, which a
 * virtual scheduler cannot serve, so its pool is this one.
 */
class InlineExecutorService : java.util.concurrent.AbstractExecutorService() {
    @Volatile private var down = false

    override fun execute(command: Runnable) = command.run()

    override fun shutdown() {
        down = true
    }

    override fun shutdownNow(): MutableList<Runnable> {
        down = true
        return mutableListOf()
    }

    override fun isShutdown() = down

    override fun isTerminated() = down

    override fun awaitTermination(timeout: Long, unit: java.util.concurrent.TimeUnit) = down
}
