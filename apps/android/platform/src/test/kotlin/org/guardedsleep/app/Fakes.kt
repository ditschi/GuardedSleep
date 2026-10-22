package org.guardedsleep.app

import org.guardedsleep.app.alerts.AlertSink
import org.guardedsleep.app.alerts.LinkSnapshot
import org.guardedsleep.app.alerts.NoiseKind
import org.guardedsleep.app.alerts.Notice
import org.guardedsleep.app.link.Clock
import org.guardedsleep.app.link.ParentEffects
import org.guardedsleep.app.pairing.PairingCipher
import uniffi.guardedsleep_core.LinkState
import uniffi.guardedsleep_core.LinkStatus

class FakeClock(var elapsed: Long = 1_000, var wall: Long = 1_700_000_000_000) : Clock {
    override fun elapsedMs() = elapsed

    override fun wallMs() = wall

    fun advance(ms: Long) {
        elapsed += ms
        wall += ms
    }
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

    override fun noiseAlert(kind: NoiseKind, probe: Boolean, snoozed: Boolean) {
        noise += Triple(kind, probe, snoozed)
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

class RecordingEffects : ParentEffects {
    val muted = mutableListOf<Boolean>()
    var listenStopped = 0
    var tests = 0
    var snoozeEnded = 0

    override fun muteRemote(muted: Boolean) {
        this.muted += muted
    }

    override fun listenStopped() {
        listenStopped++
    }

    override fun testAlertArrived(wallMs: Long) {
        tests++
    }

    override fun snoozeEnded() {
        snoozeEnded++
    }
}

fun status(
    state: LinkState,
    level: Int = 0,
    key: String? = null,
    cause: String? = null,
    ack: Long? = null,
    lastAlive: Long? = null,
) = LinkStatus(state, level.toUByte(), key, cause, ack?.toULong(), lastAlive?.toULong())
