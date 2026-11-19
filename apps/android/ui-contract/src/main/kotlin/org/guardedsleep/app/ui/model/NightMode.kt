package org.guardedsleep.app.ui.model

/**
 * The window logic of Automatic night mode, pure so that both clients and the tests share the same
 * rules. Times are minutes since local midnight (0 until 1440); the platform supplies the wall
 * clock and the offset of the local zone, this object never reads a clock.
 */
object NightSchedule {
    const val MINUTES_PER_DAY = 24 * 60
    const val DEFAULT_FROM_MINUTE = 21 * 60
    const val DEFAULT_TO_MINUTE = 7 * 60

    private const val MS_PER_MINUTE = 60_000L
    private const val MS_PER_DAY = MINUTES_PER_DAY * MS_PER_MINUTE

    /**
     * A stored value outside the day becomes the default; keeps a bad file from reaching the UI.
     */
    fun normalizeMinute(minute: Int, fallback: Int): Int =
        if (minute in 0 until MINUTES_PER_DAY) minute else fallback

    private fun localMsOfDay(nowWallMs: Long, offsetMs: Long): Long =
        Math.floorMod(nowWallMs + offsetMs, MS_PER_DAY)

    /**
     * Whether [minuteOfDay] lies in the window [from] (inclusive) to [to] (exclusive). A window
     * that ends before it starts wraps past midnight (21:00 to 07:00); equal ends make an empty
     * window, which is never on.
     */
    fun inWindow(from: Int, to: Int, minuteOfDay: Int): Boolean =
        when {
            from == to -> false
            from < to -> minuteOfDay in from until to
            else -> minuteOfDay >= from || minuteOfDay < to
        }

    /**
     * The first start or end of the window strictly after [nowWallMs] as a wall-clock instant, or
     * null for an empty window. Daylight-saving changes may shift it by the change itself; the
     * platform recomputes on every clock or zone change anyway.
     */
    fun nextBoundaryWallMs(from: Int, to: Int, nowWallMs: Long, offsetMs: Long): Long? {
        if (from == to) return null
        val now = localMsOfDay(nowWallMs, offsetMs)
        fun until(minute: Int): Long {
            val d = Math.floorMod(minute * MS_PER_MINUTE - now, MS_PER_DAY)
            return if (d == 0L) MS_PER_DAY else d
        }
        return nowWallMs + minOf(until(from), until(to))
    }

    /** Whether night mode is on right now, from the stored choice. */
    fun isActive(
        mode: NightMode,
        from: Int,
        to: Int,
        override: NightOverride?,
        nowWallMs: Long,
        offsetMs: Long,
    ): Boolean =
        when (mode) {
            NightMode.OFF -> false
            NightMode.ON -> true
            NightMode.AUTO -> {
                val held =
                    // An override further away than a day was written under another clock.
                    override?.takeIf {
                        nowWallMs < it.untilWallMs && it.untilWallMs - nowWallMs <= MS_PER_DAY
                    }
                held?.active
                    ?: inWindow(
                        from,
                        to,
                        (localMsOfDay(nowWallMs, offsetMs) / MS_PER_MINUTE).toInt(),
                    )
            }
        }

    fun isActive(s: SettingsState, nowWallMs: Long, offsetMs: Long): Boolean =
        isActive(
            s.nightMode,
            s.nightFromMinute,
            s.nightToMinute,
            s.nightOverride,
            nowWallMs,
            offsetMs,
        )

    /**
     * The settings after the night icon or the More-sheet switch is tapped: it turns night mode off
     * while on and on while off. With Automatic the choice becomes an override that lasts until the
     * next window boundary; with Off or On the mode itself flips.
     */
    fun toggled(s: SettingsState, nowWallMs: Long, offsetMs: Long): SettingsState {
        val target = !isActive(s, nowWallMs, offsetMs)
        return when (s.nightMode) {
            NightMode.AUTO -> {
                val until =
                    nextBoundaryWallMs(s.nightFromMinute, s.nightToMinute, nowWallMs, offsetMs)
                s.copy(
                    nightOverride = until?.let { NightOverride(target, it) },
                    // An empty window has no boundary to hold an override until: the choice becomes
                    // the mode.
                    nightMode =
                        if (until == null) (if (target) NightMode.ON else NightMode.OFF)
                        else NightMode.AUTO,
                )
            }
            else ->
                s.copy(
                    nightMode = if (target) NightMode.ON else NightMode.OFF,
                    nightOverride = null,
                )
        }
    }

    /** The settings after the user picks [mode] on the Display page: an override never survives. */
    fun withMode(s: SettingsState, mode: NightMode): SettingsState =
        s.copy(nightMode = mode, nightOverride = null)

    /** The settings after the window changes: the old boundary no longer applies. */
    fun withWindow(s: SettingsState, from: Int, to: Int): SettingsState =
        s.copy(nightFromMinute = from, nightToMinute = to, nightOverride = null)
}
