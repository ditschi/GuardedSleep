package org.guardedsleep.app.link

import android.os.SystemClock

/**
 * Two clocks, never mixed: the core and all durations use the monotonic [elapsedMs] (it keeps
 * running in deep sleep); only user-visible clock times use [wallMs].
 */
interface Clock {
    fun elapsedMs(): Long

    fun wallMs(): Long
}

object SystemClockSource : Clock {
    override fun elapsedMs(): Long = SystemClock.elapsedRealtime()

    override fun wallMs(): Long = System.currentTimeMillis()
}
