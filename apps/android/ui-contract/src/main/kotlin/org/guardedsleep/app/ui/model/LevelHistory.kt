package org.guardedsleep.app.ui.model

/**
 * The last minute of the received sound level for the monitor's sound strip. Presentation only (the
 * existing meter model, sampled): bucket 0 is the oldest, a bucket keeps the loudest sample it saw,
 * buckets without a sample read 0. The web client mirrors this in `level-history.ts`.
 */
class LevelHistory(private val windowMs: Long = 60_000L, private val stepMs: Long = 1_000L) {
    private class Sample(val atMs: Long, val level: Float)

    private val samples = ArrayDeque<Sample>()

    fun push(nowMs: Long, level: Float) {
        samples.addLast(Sample(nowMs, level.coerceIn(0f, 1f)))
        // Keep one extra window: `bars` may be asked for a later `now` than the last push.
        while (samples.isNotEmpty() && samples.first().atMs < nowMs - 2 * windowMs) samples
            .removeFirst()
    }

    fun clear() = samples.clear()

    /** [count] bars, oldest first, covering the window that ends at [nowMs]. */
    fun bars(nowMs: Long, count: Int = (windowMs / stepMs).toInt()): List<Float> {
        val out = FloatArray(count)
        val start = nowMs - windowMs
        val bucketMs = windowMs.toDouble() / count
        for (s in samples) {
            if (s.atMs < start || s.atMs > nowMs) continue
            val i = ((s.atMs - start) / bucketMs).toInt().coerceIn(0, count - 1)
            if (s.level > out[i]) out[i] = s.level
        }
        return out.toList()
    }
}
