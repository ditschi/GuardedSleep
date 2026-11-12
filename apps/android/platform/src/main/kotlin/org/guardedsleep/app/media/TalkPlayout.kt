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
        speaker.setReceiverGain(attempt, 0.0)
        speaker.setReceiverEnabled(attempt, true)
        speaker.setSpeakerMuted(false)
        baseline.remove(attempt)
        PocLog.event("talk_playback", "enabled" to true)
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
        val fade = ((nowMs - l.startMs).toDouble() / l.fadeInMs).coerceIn(0.0, 1.0)
        speaker.setReceiverGain(l.attempt, l.attenuation * fade * limiter)
    }

    /** RMS of the incoming voice before our gain; steps the gain down above the ceiling. */
    fun onTalkLevel(rms: Double, nowMs: Long) {
        if (lease == null) return
        if (rms > 0.0) {
            val needed = TalkPolicy.CEILING_RMS / rms
            limiter =
                if (needed < limiter) needed
                else minOf(1.0, limiter + TalkPolicy.LIMITER_RELEASE_PER_TICK)
        }
        tick(nowMs)
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
}
