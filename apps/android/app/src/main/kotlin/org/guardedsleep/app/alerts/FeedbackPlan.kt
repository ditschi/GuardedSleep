package org.guardedsleep.app.alerts

import org.guardedsleep.app.config.AlertPolicy
import org.guardedsleep.app.ui.model.LinkPhase

enum class NotifyLevel { NONE, LOW, HIGH }

/** What the parent phone does for the current connection level (ADR-017 §7, alpha-ux-flows §3.6). */
data class LinkFeedbackPlan(
    val notify: NotifyLevel = NotifyLevel.NONE,
    val fullScreen: Boolean = false,
    /** One gentle vibration when level 1 is entered (owner decision, off by default). */
    val oneShotHaptic: Boolean = false,
    val hapticEveryMs: Long? = null,
    val toneEveryMs: Long? = null,
    val continuousAlarm: Boolean = false,
) {
    val silent: Boolean
        get() = hapticEveryMs == null && toneEveryMs == null && !continuousAlarm && !oneShotHaptic
}

/** What the parent phone does for a noise alert. */
data class NoiseFeedbackPlan(
    val notify: Boolean = false,
    val fullScreen: Boolean = false,
    val repeatingHaptic: Boolean = false,
    val shortHaptic: Boolean = false,
)

object FeedbackPlans {
    fun forLink(phase: LinkPhase, level: Int, acknowledged: Boolean, wasConnected: Boolean = false): LinkFeedbackPlan {
        // A terminal failure of a link that worked before must never be silent, whatever the core says.
        if (phase == LinkPhase.FAILED_NEEDS_USER && wasConnected) return forLink(LinkPhase.RECONNECTING, 2, acknowledged)
        if (phase != LinkPhase.CONNECTING && phase != LinkPhase.RECONNECTING) return LinkFeedbackPlan()
        return when {
            level <= 0 -> LinkFeedbackPlan()
            level == 1 ->
                LinkFeedbackPlan(notify = NotifyLevel.LOW, oneShotHaptic = AlertPolicy.LEVEL1_VIBRATES && !acknowledged)
            level == 2 ->
                LinkFeedbackPlan(
                    notify = NotifyLevel.HIGH,
                    fullScreen = true,
                    hapticEveryMs = AlertPolicy.LEVEL2_HAPTIC_INTERVAL_MS.takeUnless { acknowledged },
                    toneEveryMs = AlertPolicy.LEVEL2_TONE_INTERVAL_MS.takeUnless { acknowledged },
                )
            else ->
                LinkFeedbackPlan(
                    notify = NotifyLevel.HIGH,
                    fullScreen = true,
                    hapticEveryMs = AlertPolicy.LEVEL2_HAPTIC_INTERVAL_MS.takeUnless { acknowledged },
                    continuousAlarm = AlertPolicy.LEVEL3_CONTINUOUS && !acknowledged,
                )
        }
    }

    /** Snoozed noise is visual only; a continuing event gets one short haptic and no new full screen. */
    fun forNoise(kind: NoiseKind, snoozed: Boolean): NoiseFeedbackPlan =
        when {
            snoozed -> NoiseFeedbackPlan()
            kind == NoiseKind.NEW ->
                NoiseFeedbackPlan(notify = true, fullScreen = true, repeatingHaptic = true)
            else -> NoiseFeedbackPlan(notify = true, shortHaptic = true)
        }
}
