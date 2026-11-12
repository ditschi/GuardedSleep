package org.guardedsleep.app.media

import kotlin.math.ceil
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
 * Raises the alarm stream only as far as a talk needs and attenuates the talk track for the rest
 * (design §7, Critical Review I12). It restores the previous index only when the index is still the
 * one it set, so a change the user made meanwhile is never overwritten.
 */
class AlarmVolume(private val stream: StreamVolume, private val memory: VolumeMemory) {
    private var previous: Int? = null
    private var raisedTo: Int? = null
    private var attenuation = 1.0

    /** Track gain (0..1] for [level], or null when the level cannot be reached (report muted). */
    fun prepare(level: TalkLevel): Double? {
        if (raisedTo != null || previous != null) return attenuation
        if (stream.fixed || stream.max <= 0) return null
        val target = ceil(fraction(level) * stream.max).toInt().coerceIn(1, stream.max)
        val now = runCatching { stream.index }.getOrNull() ?: return null
        if (now >= target) {
            attenuation = target.toDouble() / now
            previous = now
            return attenuation
        }
        val ok = runCatching { stream.index = target }.isSuccess
        val read = runCatching { stream.index }.getOrNull()
        if (!ok || read != target) {
            if (read != null && read != now) runCatching { stream.index = now }
            return null
        }
        previous = now
        raisedTo = target
        attenuation = 1.0
        memory.save(now, target)
        PocLog.event("alarm_volume", "phase" to "raised", "previous" to now, "set" to target)
        return attenuation
    }

    /** Undoes a raise of this run; a no-op when nothing was raised or the user changed it. */
    fun restore() {
        val set = raisedTo
        val prev = previous
        previous = null
        raisedTo = null
        attenuation = 1.0
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

    private fun fraction(level: TalkLevel) =
        when (level) {
            TalkLevel.LOW -> TalkPolicy.LOW_FRACTION
            TalkLevel.MEDIUM -> TalkPolicy.MEDIUM_FRACTION
            TalkLevel.HIGH -> TalkPolicy.HIGH_FRACTION
        }
}
