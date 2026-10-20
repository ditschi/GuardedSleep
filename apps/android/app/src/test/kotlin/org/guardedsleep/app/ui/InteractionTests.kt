@file:OptIn(ExperimentalTestApi::class)

package org.guardedsleep.app.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.center
import androidx.compose.ui.test.swipeUp
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.runComposeUiTest
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.ListenMode
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.screens.BabyKioskScreen
import org.guardedsleep.app.ui.screens.ParentMonitorScreen
import org.guardedsleep.app.ui.theme.GuardedSleepTheme
import org.guardedsleep.app.ui.model.BabyMonitorState
import org.guardedsleep.app.ui.model.Theme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class InteractionTests : ShotBase() {
    private val monitoring = AppState(role = Role.PARENT, onboardingDone = true, paired = true, monitoring = true)

    private fun ComposeUiTest.show(content: @Composable () -> Unit) = setContent {
        CompositionLocalProvider(LocalUiEnv provides TestEnv) { GuardedSleepTheme(Theme.DARK, content = content) }
    }

    private fun connectedController() = FakeAppController(app = monitoring, parent = ParentMonitorState(link = Connected))

    @Test fun slideUpWhileHoldingLocksListening() = runComposeUiTest {
        val c = connectedController()
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_listen").performTouchInput { down(center) }
        mainClock.advanceTimeBy(400)
        onNodeWithTag("parent_listen").performTouchInput { moveBy(Offset(0f, -400f)) }
        onNodeWithTag("parent_listen").performTouchInput { up() }
        waitForIdle()
        assertEquals(listOf("setListen:HOLD", "setListen:LOCKED"), c.calls.filter { it.startsWith("setListen") })
        assertEquals(ListenMode.LOCKED, c.parentFlow.value.listen)
    }

    @Test fun releasingWithoutSlidingStopsListening() = runComposeUiTest {
        val c = connectedController()
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_listen").performTouchInput { down(center) }
        mainClock.advanceTimeBy(400)
        onNodeWithTag("parent_listen").performTouchInput { up() }
        waitForIdle()
        assertEquals(listOf("setListen:HOLD", "setListen:OFF"), c.calls.filter { it.startsWith("setListen") })
    }

    @Test fun tapWhileLockedStops() = runComposeUiTest {
        val c = FakeAppController(app = monitoring, parent = ParentMonitorState(link = Connected, listen = ListenMode.LOCKED))
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_listen").performTouchInput { down(center); up() }
        waitForIdle()
        assertEquals(listOf("setListen:OFF"), c.calls.filter { it.startsWith("setListen") })
    }

    @Test fun listenIsDisabledWhileDisconnected() = runComposeUiTest {
        val c = FakeAppController(app = monitoring, parent = ParentMonitorState(link = link(LinkPhase.RECONNECTING, 1, 30_000)))
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_listen").assertIsNotEnabled()
    }

    @Test fun holdAndSlideUnlocksKiosk() = runComposeUiTest {
        val c = FakeAppController(app = monitoring.copy(role = Role.BABY), baby = BabyMonitorState(link = Connected, kioskLocked = true))
        show { BabyKioskScreen(c) }
        onNodeWithTag("baby_kiosk").performTouchInput { down(Offset(center.x, height - 100f)) }
        mainClock.advanceTimeBy(700)
        onNodeWithTag("baby_kiosk").performTouchInput { moveBy(Offset(0f, -600f)) }
        onNodeWithTag("baby_kiosk").performTouchInput { up() }
        waitForIdle()
        assertTrue(c.calls.contains("setKioskLocked:false"))
    }

    @Test fun quickSwipeDoesNotUnlockKiosk() = runComposeUiTest {
        val c = FakeAppController(app = monitoring.copy(role = Role.BABY), baby = BabyMonitorState(link = Connected, kioskLocked = true))
        show { BabyKioskScreen(c) }
        onNodeWithTag("baby_kiosk").performTouchInput { swipeUp() }
        waitForIdle()
        assertFalse(c.calls.contains("setKioskLocked:false"))
    }

    @Test fun snoozeSheetCallsController() = runComposeUiTest {
        val c = connectedController()
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("sheet_snooze").assertDoesNotExist()
        onNodeWithTag("parent_snooze").performClick()
        onNodeWithTag("sheet_snooze").assertIsDisplayed()
        onNodeWithTag("sheet_snooze_30").performClick()
        assertTrue(c.calls.contains("snooze:30"))
        onNodeWithTag("sheet_snooze").assertDoesNotExist()
    }

    @Test fun stopMonitoringNeedsOverflowAndConfirmation() = runComposeUiTest {
        val c = connectedController()
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_stop").assertDoesNotExist()
        onNodeWithTag("parent_overflow").performClick()
        onNodeWithTag("parent_stop").performClick()
        assertFalse(c.calls.contains("stopMonitoring"))
        onNodeWithTag("dialog_stop_cancel").performClick()
        assertFalse(c.calls.contains("stopMonitoring"))
        onNodeWithTag("parent_overflow").performClick()
        onNodeWithTag("parent_stop").performClick()
        onNodeWithTag("dialog_stop_confirm").performClick()
        assertTrue(c.calls.contains("stopMonitoring"))
    }

    @Test fun acknowledgeButtonCallsController() = runComposeUiTest {
        val c = FakeAppController(app = monitoring, parent = ParentMonitorState(link = link(LinkPhase.RECONNECTING, 2, 95_000)))
        show { LinkAlarmScreen(c, {}) }
        onNodeWithTag("link_alarm_ack").performClick()
        assertEquals(listOf("acknowledgeAlarm"), c.calls)
    }

    @Test fun acknowledgeOnMonitorScreenDuringLevel2() = runComposeUiTest {
        val c = FakeAppController(app = monitoring, parent = ParentMonitorState(link = link(LinkPhase.RECONNECTING, 2, 95_000)))
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_ack").performClick()
        assertTrue(c.calls.contains("acknowledgeAlarm"))
    }

    @Test fun stopSoundCallsController() = runComposeUiTest {
        val c = FakeAppController(app = monitoring, parent = ParentMonitorState(link = Connected, noiseAlertActive = true, level = 0.8f))
        show { NoiseAlertScreen(c, {}) }
        onNodeWithTag("alert_stop_sound").performClick()
        assertTrue(c.calls.contains("stopAlertSound"))
    }

    @Test fun pairingTextIsHiddenUntilRequested() = runComposeUiTest {
        val exp = ELAPSED + 200_000
        val c = FakeAppController(app = monitoring.copy(role = Role.BABY, monitoring = false), pairing = org.guardedsleep.app.ui.model.PairingState(org.guardedsleep.app.ui.model.PairingPhase.SHOWING_CODE, "guardedsleep://secret", exp))
        show { org.guardedsleep.app.ui.screens.PairBabyScreen(c) }
        onNodeWithTag("baby_pair_code_text").assertDoesNotExist()
        onNodeWithTag("baby_pair_show_text").performClick()
        onNodeWithTag("baby_pair_code_text").assertIsDisplayed()
    }

    @Test fun expiredCodeHidesCopyAndBrightness() = runComposeUiTest {
        val c = FakeAppController(app = monitoring.copy(role = Role.BABY, monitoring = false), pairing = org.guardedsleep.app.ui.model.PairingState(org.guardedsleep.app.ui.model.PairingPhase.EXPIRED, "guardedsleep://secret", ELAPSED - 1))
        show { org.guardedsleep.app.ui.screens.PairBabyScreen(c) }
        onNodeWithTag("baby_pair_copy").assertDoesNotExist()
        onNodeWithTag("baby_pair_brighter").assertDoesNotExist()
        onNodeWithTag("baby_pair_new_code").assertIsDisplayed()
    }

    @Test fun noRecordedClockLeak() { assertEquals(1_000_000L, ELAPSED) }
}
