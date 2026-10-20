package org.guardedsleep.app.link

import org.guardedsleep.app.ui.model.CauseHint
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.LinkQuality
import org.guardedsleep.app.ui.model.LinkUi
import uniffi.guardedsleep_core.LinkState
import uniffi.guardedsleep_core.LinkStatus

object LinkMapping {
    fun phase(state: LinkState): LinkPhase =
        when (state) {
            LinkState.CONNECTING -> LinkPhase.CONNECTING
            LinkState.CONNECTED -> LinkPhase.CONNECTED
            LinkState.RECONNECTING -> LinkPhase.RECONNECTING
            LinkState.PEER_STOPPED -> LinkPhase.PEER_STOPPED
            LinkState.FAILED_NEEDS_USER -> LinkPhase.FAILED_NEEDS_USER
            LinkState.STOPPED -> LinkPhase.STOPPED
        }

    fun causeHint(key: String?): CauseHint? =
        when (key) {
            "server_unreachable" -> CauseHint.SERVER_UNREACHABLE
            "peer_absent" -> CauseHint.PEER_ABSENT
            "media_blocked" -> CauseHint.MEDIA_BLOCKED
            else -> null
        }

    fun linkUi(status: LinkStatus, quality: LinkQuality): LinkUi {
        val phase = phase(status.state)
        val escalating = phase == LinkPhase.CONNECTING || phase == LinkPhase.RECONNECTING
        return LinkUi(
            phase = phase,
            level = if (escalating) status.level.toInt() else 0,
            lastAliveElapsedMs = status.lastAliveMs?.toLong(),
            acknowledgedUntilElapsedMs = status.acknowledgedUntil?.toLong(),
            causeHint = causeHint(status.cause),
            // update_required also arrives as a sticky cause while connected or reconnecting.
            failureKey = if (phase == LinkPhase.FAILED_NEEDS_USER) status.key else status.cause.takeIf { it == "update_required" },
            quality = if (phase == LinkPhase.CONNECTED) quality else LinkQuality.UNKNOWN,
        )
    }

    /** Loss over the last stats interval; null when too few packets arrived to judge. */
    fun quality(deltaReceived: Long, deltaLost: Long): LinkQuality? {
        val total = deltaReceived + deltaLost
        if (total < MIN_PACKETS) return null
        val loss = deltaLost.coerceAtLeast(0) * 100.0 / total
        return when {
            loss < 3.0 -> LinkQuality.GOOD
            loss < 10.0 -> LinkQuality.MEDIUM
            else -> LinkQuality.POOR
        }
    }

    private const val MIN_PACKETS = 20L
}
