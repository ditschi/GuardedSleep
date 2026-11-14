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
fun testSettings(dir: File, name: String = "s"): SettingsRepository {
    val scope = CoroutineScope(Dispatchers.Unconfined)
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
