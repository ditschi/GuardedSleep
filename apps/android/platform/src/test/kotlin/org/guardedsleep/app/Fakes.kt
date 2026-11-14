package org.guardedsleep.app

import org.guardedsleep.app.fixtures.FakeCipher
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

/** Fails [failures] decrypts with [error], then behaves like [FakeCipher]. */
class FlakyCipher(var failures: Int, private val error: () -> Exception) : PairingCipher {
    private val inner = FakeCipher()

    override fun encrypt(plain: ByteArray) = inner.encrypt(plain)

    override fun decrypt(sealed: ByteArray): ByteArray {
        if (failures > 0) {
            failures--
            throw error()
        }
        return inner.decrypt(sealed)
    }

    override fun deleteKey() = inner.deleteKey()
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
