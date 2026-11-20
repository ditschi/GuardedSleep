package org.guardedsleep.app.media

import org.guardedsleep.app.config.TalkPolicy
import org.guardedsleep.app.diag.PocLog
import uniffi.guardedsleep_core.TalkLevel

/** The baby unit's loudspeaker side of talk-back, one method per physical effect. */
interface TalkSpeaker {
    /** Enables the talk receiver of [attempt]; every other attempt's receiver stays disabled. */
    fun setReceiverEnabled(attempt: UInt, enabled: Boolean)

    fun setReceiverGain(attempt: UInt, gain: Double)

    /** The ADM speaker: muted outside a lease. */
    fun setSpeakerMuted(muted: Boolean)

    /** Plays the chime on its own alarm-stream track; [onDone] runs when it ended or failed. */
    fun playChime(gain: Double, onDone: () -> Unit)

    fun stopChime()

    /** Logs where the playback is routed (diagnostics only). */
    fun logRoute() {}
}

/**
 * The lease on the baby's speaker (design §7): playback exists only between `set_talk_playback {
 * true }` and `{ false }` or `until_ms`, whichever comes first; the media layer mutes by itself at
 * the deadline so a lost stop cannot leave the speaker on. Driven by [tick] (about 10 Hz) and
 * [onTalkLevel], so it needs no timers of its own. All calls come from the core thread.
 */
class TalkPlayout(
    private val speaker: TalkSpeaker,
    private val volume: AlarmVolume,
    private val onChimeDone: (UInt) -> Unit,
    private val onMuted: (Boolean) -> Unit,
    private val onOutsideGrant: (UInt) -> Unit,
    /** See [TalkPolicy.LEVEL_IS_POST_GAIN]; a parameter so both positions are tested. */
    private val levelIsPostGain: Boolean = TalkPolicy.LEVEL_IS_POST_GAIN,
) {
    private class Lease(
        val attempt: UInt,
        val startMs: Long,
        val fadeInMs: Long,
        val untilMs: Long,
        val attenuation: Double,
    )

    private var lease: Lease? = null
    private var limiter = 1.0
    private var appliedGain = 0.0
    private var lastGainLogMs = 0L
    private var lastPreLogMs = 0L
    private var routeLogged = false
    private var leaseEndedMs: Long? = null
    private val baseline = HashMap<UInt, Long>()
    private val warned = HashSet<UInt>()

    val active: Boolean
        get() = lease != null

    /** The engine starts muted; call once after the engine exists. */
    fun start() {
        speaker.setSpeakerMuted(true)
        volume.restoreLeftover()
    }

    fun playChime(attempt: UInt, level: TalkLevel) {
        val att = prepare(level)
        if (att == null) {
            onChimeDone(attempt)
            return
        }
        speaker.playChime(att) { onChimeDone(attempt) }
    }

    fun open(attempt: UInt, level: TalkLevel, fadeInMs: Long, untilMs: Long, nowMs: Long) {
        close(nowMs)
        val att = prepare(level) ?: return
        limiter = 1.0
        lease = Lease(attempt, nowMs, fadeInMs.coerceAtLeast(1), untilMs, att)
        appliedGain = 0.0
        lastGainLogMs = nowMs
        routeLogged = false
        speaker.setReceiverGain(attempt, 0.0)
        speaker.setReceiverEnabled(attempt, true)
        speaker.setSpeakerMuted(false)
        baseline.remove(attempt)
        PocLog.event("talk_playback", "enabled" to true)
        // What the baby unit does to reach the set volume: the stream index before and the target,
        // and the residual track gain (diagnostics only, no PII).
        PocLog.event(
            "talk_volume",
            "pct" to level.pct,
            "before" to volume.indexBefore,
            "target" to volume.targetIndex,
            "residual" to "%.3f".format(att),
        )
    }

    /** `set_talk_playback { false }`, the deadline, teardown: mute, disable, restore the volume. */
    fun close(nowMs: Long) {
        speaker.stopChime()
        val l = lease
        lease = null
        if (l != null) {
            speaker.setReceiverEnabled(l.attempt, false)
            leaseEndedMs = nowMs
            PocLog.event("talk_playback", "enabled" to false)
        }
        speaker.setSpeakerMuted(true)
        volume.restore()
    }

    fun tick(nowMs: Long) {
        val l = lease ?: return
        if (nowMs >= l.untilMs) return close(nowMs)
        if (!routeLogged && nowMs - l.startMs >= ROUTE_LOG_AFTER_MS) {
            routeLogged = true
            speaker.logRoute()
        }
        val fade = ((nowMs - l.startMs).toDouble() / l.fadeInMs).coerceIn(0.0, 1.0)
        appliedGain = l.attenuation * fade * limiter
        speaker.setReceiverGain(l.attempt, appliedGain)
        if (nowMs - lastGainLogMs >= GAIN_LOG_MS) {
            lastGainLogMs = nowMs
            PocLog.event("talk_gain", "gain" to "%.3f".format(appliedGain))
        }
    }

    /**
     * One level sample of the incoming voice (stats `audioLevel`). The cap judges the level before
     * our gain, so it never reacts to its own gain: [levelIsPostGain] divides by the gain applied
     * (skipped when that is too small to divide). Steps the gain down above the ceiling. Returns
     * the pre-gain level for the core's `talk_level` input, or null when none could be judged. The
     * talk line stays sink-free: only this number is kept, never a sample.
     */
    fun onTalkLevel(rms: Double, nowMs: Long): Double? {
        if (lease == null) return null
        val pre =
            when {
                !levelIsPostGain -> rms
                appliedGain > TalkPolicy.MIN_JUDGED_GAIN -> rms / appliedGain
                else -> null
            }
        if (pre != null && pre > 0.0) {
            val needed = TalkPolicy.CEILING_RMS / pre
            limiter =
                if (needed < limiter) needed
                else minOf(1.0, limiter + TalkPolicy.LIMITER_RELEASE_PER_TICK)
        }
        tick(nowMs)
        if (pre != null && nowMs - lastPreLogMs >= GAIN_LOG_MS) {
            lastPreLogMs = nowMs
            PocLog.event("talk_level_pre", "rms" to "%.4f".format(pre))
        }
        return pre
    }

    /** Inbound talk packets of [attempt] (stats poll). Reports audio outside a lease once. */
    fun onTalkPackets(attempt: UInt, packets: Long, nowMs: Long) {
        val inLease = lease?.attempt == attempt
        val inGrace = leaseEndedMs?.let { nowMs - it < TalkPolicy.RTP_GRACE_MS } == true
        val base = baseline[attempt]
        if (inLease || inGrace || base == null) {
            baseline[attempt] = packets
            return
        }
        if (packets - base > TalkPolicy.RTP_OUTSIDE_PACKETS && warned.add(attempt)) {
            onOutsideGrant(attempt)
        }
    }

    fun attemptClosed(attempt: UInt) {
        baseline.remove(attempt)
        warned.remove(attempt)
    }

    private fun prepare(level: TalkLevel): Double? {
        val att = volume.prepare(level)
        onMuted(att == null)
        return att
    }

    private companion object {
        const val GAIN_LOG_MS = 1_000L

        /** The playback exists a moment after the unmute; the route is read then. */
        const val ROUTE_LOG_AFTER_MS = 500L
    }
}
