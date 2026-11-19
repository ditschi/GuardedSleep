package org.guardedsleep.app

import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.guardedsleep.app.settings.NightClock
import org.guardedsleep.app.settings.SettingsRepository
import org.guardedsleep.app.ui.model.NightMode
import org.guardedsleep.app.ui.model.NightOverride
import org.guardedsleep.app.ui.model.NightSchedule
import org.guardedsleep.app.ui.model.SettingsState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

/**
 * `ui-appearance` "Night mode" and "Automatic night mode by time": the window logic (midnight wrap,
 * manual override until the next boundary), its ticking flow and the stored choice.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class NightModeTest {
    @get:Rule val tmp = TemporaryFolder()

    private companion object {
        const val MIN = 60_000L
        const val DAY = 24 * 60 * MIN
        const val FROM = 21 * 60
        const val TO = 7 * 60

        /** Local clock time on day [day] (UTC, offset 0). */
        fun at(day: Int, hour: Int, minute: Int = 0, second: Int = 0) =
            day * DAY + (hour * 60L + minute) * MIN + second * 1000L

        fun auto(override: NightOverride? = null) =
            SettingsState(nightMode = NightMode.AUTO, nightOverride = override)

        fun active(s: SettingsState, now: Long, offset: Long = 0) =
            NightSchedule.isActive(s, now, offset)
    }

    // ---- the window --------------------------------------------------------------------------

    @Test
    fun aWindowThatEndsBeforeItStartsWrapsPastMidnight() {
        fun on(h: Int, m: Int = 0) = NightSchedule.inWindow(FROM, TO, h * 60 + m)
        assertTrue(on(21))
        assertTrue(on(23, 59))
        assertTrue(on(0))
        assertTrue(on(3))
        assertTrue(on(6, 59))
        assertFalse("the end is exclusive", on(7))
        assertFalse(on(12))
        assertFalse(on(20, 59))
    }

    @Test
    fun aWindowWithinOneDayDoesNotWrap() {
        assertTrue(NightSchedule.inWindow(13 * 60, 15 * 60, 14 * 60))
        assertFalse(NightSchedule.inWindow(13 * 60, 15 * 60, 15 * 60))
        assertFalse(NightSchedule.inWindow(13 * 60, 15 * 60, 2 * 60))
    }

    @Test
    fun anEmptyWindowIsNeverOnAndHasNoBoundary() {
        assertFalse(NightSchedule.inWindow(8 * 60, 8 * 60, 8 * 60))
        assertNull(NightSchedule.nextBoundaryWallMs(8 * 60, 8 * 60, at(3, 8), 0))
    }

    @Test
    fun theNextBoundaryIsTheNextStartOrEndStrictlyAfterNow() {
        fun next(now: Long) = NightSchedule.nextBoundaryWallMs(FROM, TO, now, 0)
        assertEquals(at(5, 21), next(at(5, 12)))
        assertEquals(at(6, 7), next(at(5, 22)))
        assertEquals(at(5, 7), next(at(5, 3)))
        assertEquals("exactly at the end the next one is the start", at(5, 21), next(at(5, 7)))
        assertEquals("exactly at the start the next one is the end", at(6, 7), next(at(5, 21)))
        assertEquals(at(5, 21), next(at(5, 20, 59, 59)))
    }

    @Test
    fun theLocalZoneOffsetShiftsTheWindow() {
        val plus2 = 2 * 60 * MIN
        // 19:00 UTC is 21:00 in UTC+2: night mode is on; at 18:59 UTC it is not.
        assertTrue(active(auto(), at(5, 19), plus2))
        assertFalse(active(auto(), at(5, 18, 59), plus2))
        assertEquals(at(5, 19), NightSchedule.nextBoundaryWallMs(FROM, TO, at(5, 12), plus2))
        // Before 1970 the day still wraps correctly.
        assertTrue(active(auto(), -3 * 60 * MIN))
    }

    @Test
    fun offAndOnIgnoreTheWindowAndAnyOverride() {
        val held = NightOverride(active = true, untilWallMs = at(5, 18))
        assertFalse(
            active(SettingsState(nightMode = NightMode.OFF, nightOverride = held), at(5, 22))
        )
        assertTrue(active(SettingsState(nightMode = NightMode.ON), at(5, 12)))
    }

    // ---- manual change wins until the next boundary -------------------------------------------

    @Test
    fun switchedOffInTheNightStaysOffUntilTheNextWindowStarts() {
        // The spec scenario: window 21:00 to 07:00, turned off at 23:00.
        var s = auto()
        assertTrue(active(s, at(5, 23)))
        s = NightSchedule.toggled(s, at(5, 23), 0)
        assertEquals(NightMode.AUTO, s.nightMode)
        assertEquals(NightOverride(false, at(6, 7)), s.nightOverride)
        assertFalse(active(s, at(5, 23, 30)))
        assertFalse(active(s, at(6, 3)))
        assertFalse("at 07:00 the override ends and the window is off too", active(s, at(6, 7)))
        assertFalse(active(s, at(6, 12)))
        assertTrue("and it turns on at 21:00", active(s, at(6, 21)))
    }

    @Test
    fun switchedOnInTheDayStaysOnUntilTheWindowStartsAndThenFollowsIt() {
        var s = auto()
        assertFalse(active(s, at(5, 12)))
        s = NightSchedule.toggled(s, at(5, 12), 0)
        assertEquals(NightOverride(true, at(5, 21)), s.nightOverride)
        assertTrue(active(s, at(5, 20, 59)))
        assertTrue(active(s, at(5, 21)))
        assertTrue(active(s, at(6, 3)))
        assertFalse("the next morning the window ends as usual", active(s, at(6, 7)))
    }

    @Test
    fun anOverrideFromAnotherClockIsIgnored() {
        // More than a day ahead: written under a different clock or zone; the window rules.
        val far = NightOverride(active = true, untilWallMs = at(5, 12) + DAY + MIN)
        assertFalse(active(auto(far), at(5, 12)))
    }

    @Test
    fun theToggleFlipsOffAndOnDirectlyAndKeepsNoOverride() {
        val off = NightSchedule.toggled(SettingsState(), at(5, 12), 0)
        assertEquals(NightMode.ON, off.nightMode)
        assertNull(off.nightOverride)
        val on = NightSchedule.toggled(off, at(5, 12), 0)
        assertEquals(NightMode.OFF, on.nightMode)
    }

    @Test
    fun withAnEmptyWindowTheToggleBecomesTheMode() {
        val s = SettingsState(nightMode = NightMode.AUTO, nightFromMinute = 60, nightToMinute = 60)
        val toggled = NightSchedule.toggled(s, at(5, 12), 0)
        assertEquals(NightMode.ON, toggled.nightMode)
        assertNull(toggled.nightOverride)
    }

    @Test
    fun pickingAModeOrChangingTheWindowDropsTheOverride() {
        val held = auto(NightOverride(false, at(6, 7)))
        assertNull(NightSchedule.withMode(held, NightMode.AUTO).nightOverride)
        val moved = NightSchedule.withWindow(held, 22 * 60, 6 * 60)
        assertNull(moved.nightOverride)
        assertEquals(22 * 60, moved.nightFromMinute)
    }

    @Test
    fun aMinuteOutsideTheDayFallsBackToTheDefault() {
        assertEquals(5, NightSchedule.normalizeMinute(5, 99))
        assertEquals(99, NightSchedule.normalizeMinute(-1, 99))
        assertEquals(99, NightSchedule.normalizeMinute(1440, 99))
    }

    @Test
    fun theDefaultsAreOffWithTheEveningWindow() {
        val s = SettingsState()
        assertEquals(NightMode.OFF, s.nightMode)
        assertEquals(21 * 60, s.nightFromMinute)
        assertEquals(7 * 60, s.nightToMinute)
        assertNull(s.nightOverride)
        assertFalse(active(s, at(5, 23)))
    }

    // ---- the ticking flow --------------------------------------------------------------------

    @Test
    fun theFlowEmitsAgainAtTheWindowBoundary() = runTest {
        val start = at(5, 20, 59, 30)
        val settings = MutableStateFlow(auto())
        val seen = mutableListOf<Boolean>()
        backgroundScope.launch {
            NightClock.active(settings, { start + testScheduler.currentTime }, { 0L }).toList(seen)
        }
        runCurrent()
        assertEquals(listOf(false), seen)
        advanceTimeBy(29_000)
        runCurrent()
        assertEquals(listOf(false), seen)
        advanceTimeBy(1_500)
        runCurrent()
        assertEquals(listOf(false, true), seen)
    }

    @Test
    fun theFlowFollowsTheSettingAndAnOverrideEndsAtItsBoundary() = runTest {
        val start = at(5, 23)
        val settings = MutableStateFlow(SettingsState())
        val seen = mutableListOf<Boolean>()
        backgroundScope.launch {
            NightClock.active(settings, { start + testScheduler.currentTime }, { 0L }).toList(seen)
        }
        runCurrent()
        settings.value = SettingsState(nightMode = NightMode.ON)
        runCurrent()
        // Automatic, but the parent switched night mode off until 07:00.
        settings.value = auto(NightOverride(false, at(6, 7)))
        runCurrent()
        assertEquals(listOf(false, true, false), seen)
        // Eight hours later the override has ended and the window is over: still off, no repeat.
        advanceTimeBy(8 * 60 * MIN + 1_000)
        runCurrent()
        assertEquals(listOf(false, true, false), seen)
        // At 21:00 the window starts.
        advanceTimeBy(14 * 60 * MIN)
        runCurrent()
        assertEquals(listOf(false, true, false, true), seen)
    }

    // ---- the stored choice -------------------------------------------------------------------

    private fun store(name: String, scope: CoroutineScope) =
        PreferenceDataStoreFactory.create(scope = scope) { File(tmp.root, "$name.preferences_pb") }

    @Test
    fun theChoiceWindowAndOverrideSurviveARestart() {
        val scope = CoroutineScope(Dispatchers.Unconfined)
        val ds = store("night", scope)
        val repo = SettingsRepository(ds, scope)
        assertEquals(NightMode.OFF, repo.current.settings.nightMode)
        val chosen =
            SettingsState(
                nightMode = NightMode.AUTO,
                nightFromMinute = 22 * 60 + 30,
                nightToMinute = 6 * 60,
                nightOverride = NightOverride(true, 1_234_567L),
            )
        repo.update { it.copy(settings = chosen) }
        val back = SettingsRepository(ds, scope).current.settings
        assertEquals(chosen.nightMode, back.nightMode)
        assertEquals(chosen.nightFromMinute, back.nightFromMinute)
        assertEquals(chosen.nightToMinute, back.nightToMinute)
        assertEquals(chosen.nightOverride, back.nightOverride)
        // Clearing the override removes it from the store.
        repo.update { it.copy(settings = chosen.copy(nightOverride = null)) }
        assertNull(SettingsRepository(ds, scope).current.settings.nightOverride)
    }

    @Test
    fun anUnknownModeOrAMinuteOutsideTheDayReadsAsTheDefault() {
        val scope = CoroutineScope(Dispatchers.Unconfined)
        val ds = store("bad", scope)
        runBlocking {
            ds.edit {
                it[stringPreferencesKey("night_mode")] = "twilight"
                it[intPreferencesKey("night_from_minute")] = 9999
                it[intPreferencesKey("night_to_minute")] = -5
            }
            ds.data.first()
        }
        val s = SettingsRepository(ds, scope).current.settings
        assertEquals(NightMode.OFF, s.nightMode)
        assertEquals(21 * 60, s.nightFromMinute)
        assertEquals(7 * 60, s.nightToMinute)
    }
}
