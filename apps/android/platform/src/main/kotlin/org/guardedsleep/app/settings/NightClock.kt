package org.guardedsleep.app.settings

import java.util.TimeZone
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.transformLatest
import org.guardedsleep.app.ui.model.NightMode
import org.guardedsleep.app.ui.model.NightSchedule
import org.guardedsleep.app.ui.model.SettingsState

/**
 * Resolves the night mode setting against the device clock for the UI (`ui-appearance` "Automatic
 * night mode by time"). Presentation only: nothing here reaches the core, and no sensor is read. It
 * emits again at each window boundary and, with Automatic, re-reads the clock at least once a
 * minute so a changed clock or time zone is picked up.
 */
object NightClock {
    /** The longest wait before the clock is read again (clock or zone changes). */
    internal const val RECHECK_MS = 60_000L

    /** Local zone offset at an instant; the platform's, replaced in tests. */
    fun systemOffsetMs(wallMs: Long): Long = TimeZone.getDefault().getOffset(wallMs).toLong()

    @OptIn(ExperimentalCoroutinesApi::class)
    fun active(
        settings: Flow<SettingsState>,
        wallMs: () -> Long,
        offsetMs: (Long) -> Long = ::systemOffsetMs,
    ): Flow<Boolean> =
        settings
            .transformLatest { s ->
                while (true) {
                    val now = wallMs()
                    val offset = offsetMs(now)
                    emit(NightSchedule.isActive(s, now, offset))
                    if (s.nightMode != NightMode.AUTO) awaitCancellation()
                    val boundary =
                        NightSchedule.nextBoundaryWallMs(
                            s.nightFromMinute,
                            s.nightToMinute,
                            now,
                            offset,
                        )
                    val held = s.nightOverride?.untilWallMs?.takeIf { it > now }
                    val wake = listOfNotNull(boundary, held).minOrNull()
                    delay(((wake ?: Long.MAX_VALUE) - now).coerceIn(1L, RECHECK_MS))
                }
            }
            .distinctUntilChanged()
}
