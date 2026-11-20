package org.guardedsleep.app.media

import kotlin.math.roundToInt
import org.guardedsleep.app.config.TalkPolicy
import org.guardedsleep.app.diag.PocLog
import uniffi.guardedsleep_core.TalkLevel

/** The alarm stream as the platform sees it (`AudioManager`, STREAM_ALARM); fake in tests. */
interface StreamVolume {
    val max: Int
    val fixed: Boolean

    /** Throws when the system refuses (Do Not Disturb, fixed volume). */
    var index: Int
}

/** The last raise survives a crash so the next start can undo it. */
interface VolumeMemory {
    fun save(previous: Int, set: Int)

    fun load(): Pair<Int, Int>?

    fun clear()
}

/**
 * Sets the alarm stream to the step nearest the requested percent of its maximum, never above
 * [TalkPolicy.MAX_FRACTION], and returns a residual track gain (at most 1.0) for the difference
 * between the percent and the step, so neighbouring percents still differ (design §4). The stream
 * may be lowered as well as raised; the previous index is restored only when the index is still the
 * one it set, so a change the user made meanwhile is never overwritten.
 */
class AlarmVolume(private val stream: StreamVolume, private val memory: VolumeMemory) {
    private var previous: Int? = null
    private var raisedTo: Int? = null
    private var residual = 1.0

    /** The last stream index this talk asked for, for the diagnostics; null before a prepare. */
    var targetIndex: Int? = null
        private set

    /** The index the stream had before the talk, for the diagnostics. */
    val indexBefore: Int?
        get() = previous

    /** Residual track gain (0..1] for [level], or null when the level cannot be reached (muted). */
    fun prepare(level: TalkLevel): Double? {
        if (raisedTo != null || previous != null) return residual
        if (stream.fixed || stream.max <= 0) return null
        val max = stream.max
        val cap = (TalkPolicy.MAX_FRACTION * max).roundToInt().coerceAtLeast(1)
        val requested = minOf(level.pct.toInt() / 100.0, cap.toDouble() / max)
        val target = (requested * max).roundToInt().coerceIn(1, cap)
        val gain = (requested / (target.toDouble() / max)).coerceAtMost(1.0)
        val now = runCatching { stream.index }.getOrNull() ?: return null
        targetIndex = target
        if (now == target) {
            previous = now
            residual = gain
            return residual
        }
        val ok = runCatching { stream.index = target }.isSuccess
        val read = runCatching { stream.index }.getOrNull()
        if (!ok || read != target) {
            if (read != null && read != now) runCatching { stream.index = now }
            targetIndex = null
            return null
        }
        previous = now
        raisedTo = target
        residual = gain
        memory.save(now, target)
        PocLog.event(
            "alarm_volume",
            "phase" to "set",
            "previous" to now,
            "set" to target,
            "gain" to "%.3f".format(gain),
        )
        return residual
    }

    /** Undoes a raise of this run; a no-op when nothing was raised or the user changed it. */
    fun restore() {
        val set = raisedTo
        val prev = previous
        previous = null
        raisedTo = null
        residual = 1.0
        targetIndex = null
        if (set != null && prev != null) restoreIfUnchanged(prev, set)
    }

    /** At the next start after a crash: the remembered raise is undone under the same rule. */
    fun restoreLeftover() {
        val (prev, set) = memory.load() ?: return
        restoreIfUnchanged(prev, set)
    }

    private fun restoreIfUnchanged(prev: Int, set: Int) {
        if (runCatching { stream.index }.getOrNull() == set) runCatching { stream.index = prev }
        PocLog.event(
            "alarm_volume",
            "phase" to "restored",
            "previous" to prev,
            "set" to set,
            "now" to runCatching { stream.index }.getOrNull(),
        )
        memory.clear()
    }
}
