package org.guardedsleep.app.link

import org.guardedsleep.app.alerts.NoiseKind
import org.guardedsleep.app.config.AlertPolicy

/**
 * Lifecycle of a noise event on the parent (alpha-ux-flows §3.7): a new full alert only for noise
 * at least 2 minutes after the previous event ended; noise in between continues that event. The
 * event ends after 2 minutes without loud audio.
 */
class NoiseTracker {
    var active = false
        private set

    private var lastLoudAt = 0L
    private var endedAt: Long? = null

    val lastLoudElapsedMs: Long
        get() = lastLoudAt

    fun onAlert(now: Long): NoiseKind {
        val kind =
            if (active || endedAt?.let { now - it < AlertPolicy.NOISE_EVENT_JOIN_MS } == true)
                NoiseKind.CONTINUE
            else NoiseKind.NEW
        active = true
        endedAt = null
        lastLoudAt = now
        return kind
    }

    fun onLevel(now: Long, level: Double, threshold: Double) {
        if (active && level >= threshold) lastLoudAt = now
    }

    /** True once, when the event just ended. */
    fun tick(now: Long): Boolean {
        if (active && now - lastLoudAt >= AlertPolicy.NOISE_QUIET_AFTER_MS) {
            active = false
            endedAt = now
            return true
        }
        return false
    }

    fun reset() {
        active = false
        endedAt = null
    }
}
