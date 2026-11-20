package org.guardedsleep.app.link

import uniffi.guardedsleep_core.minNoiseThreshold

/**
 * The noise threshold limits of the core, so a stored or slider value never becomes a second
 * literal: a threshold below the core's lowest is raised to it (the core would ignore it, and a
 * status that carries it would be rejected whole by the peer).
 */
internal object NoiseLimits {
    private const val MAX = 1.0

    fun clamp(threshold: Float): Double = threshold.toDouble().coerceIn(minNoiseThreshold(), MAX)
}
