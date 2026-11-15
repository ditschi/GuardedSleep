@file:OptIn(ExperimentalTestApi::class)

package org.guardedsleep.app.ui

import android.app.Activity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.runComposeUiTest
import org.guardedsleep.app.ui.common.KioskMinBrightness
import org.guardedsleep.app.ui.common.KioskWakeBrightness
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.common.findActivity
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.BabyMonitorState
import org.guardedsleep.app.ui.model.ConnectionUi
import org.guardedsleep.app.ui.model.LastTalkUi
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.Theme
import org.guardedsleep.app.ui.screens.BabyKioskScreen
import org.guardedsleep.app.ui.theme.GuardedSleepTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The baby kiosk of `redesign-calm-ui` slice 5: marks, status line, confirmation, wake rows. */
class KioskTests : ShotBase() {
    private val named =
        AppState(
            role = Role.BABY,
            onboardingDone = true,
            paired = true,
            monitoring = true,
            connections =
                listOf(ConnectionUi("a", "Travel cot", isDefault = true, selected = true)),
        )

    private fun kioskController(
        b: BabyMonitorState = BabyMonitorState(link = Connected),
        app: AppState = named,
    ) = FakeAppController(app = app, baby = b.copy(kioskLocked = true))

    private fun ComposeUiTest.show(
        env: org.guardedsleep.app.ui.common.UiEnv = TestEnv,
        c: FakeAppController,
    ) = setContent {
        CompositionLocalProvider(LocalUiEnv provides env) {
            GuardedSleepTheme(Theme.DARK, forceDark = true) { BabyKioskScreen(c) }
        }
    }

    private fun ComposeUiTest.marks() =
        listOf("bar", "dashed", "frame", "attention").filter {
            onAllNodesWithTagExists("baby_kiosk_mark_$it")
        }

    private fun ComposeUiTest.onAllNodesWithTagExists(tag: String) =
        onAllNodes(androidx.compose.ui.test.hasTestTag(tag)).fetchSemanticsNodes().isNotEmpty()

    @Test
    fun connectedShowsTheShortBarAndTheStatusLineWithTheConnectionName() = runComposeUiTest {
        show(c = kioskController())
        assertEquals(listOf("bar"), marks())
        onNodeWithTag("baby_status").assertIsDisplayed().assertTextEquals("Listening · Travel cot")
    }

    @Test
    fun anUnnamedConnectionShowsTheStateAlone() = runComposeUiTest {
        show(c = kioskController(app = named.copy(connections = emptyList())))
        onNodeWithTag("baby_status").assertTextEquals("Listening")
    }

    @Test
    fun levelZeroOfADropShowsNoMark() = runComposeUiTest {
        show(c = kioskController(BabyMonitorState(link = link(LinkPhase.RECONNECTING, 0, 8_000))))
        assertEquals(emptyList<String>(), marks())
        onNodeWithTag("baby_status").assertIsDisplayed()
    }

    @Test
    fun levelOneShowsTheDashedEdge() = runComposeUiTest {
        show(c = kioskController(BabyMonitorState(link = link(LinkPhase.RECONNECTING, 1, 40_000))))
        assertEquals(listOf("dashed"), marks())
    }

    @Test
    fun fromLevelTwoTheFrameShows() = runComposeUiTest {
        show(c = kioskController(BabyMonitorState(link = link(LinkPhase.RECONNECTING, 3, 200_000))))
        assertEquals(listOf("frame"), marks())
        onNodeWithTag("baby_status").assertIsDisplayed()
    }

    @Test
    fun aParentThatStoppedShowsNoMarkButTheWords() = runComposeUiTest {
        show(c = kioskController(BabyMonitorState(link = link(LinkPhase.PEER_STOPPED, 0, 4_000))))
        assertEquals(emptyList<String>(), marks())
        onNodeWithTag("baby_status").assertIsDisplayed()
    }

    private val pluggedIn =
        BabyMonitorState(
            link = Connected,
            ownBatteryPct = 64,
            ownPlugged = true,
            chargerSeen = true,
        )

    @Test
    fun aLowBatteryAddsTheSquareAndTheReason() = runComposeUiTest {
        show(c = kioskController(pluggedIn.copy(ownBatteryPct = 12, ownPlugged = false)))
        assertEquals(listOf("bar", "attention"), marks())
        onNodeWithTag("baby_kiosk_attention").assertTextEquals("Needs attention: battery low · 12%")
    }

    @Test
    fun aCriticalBatteryNeedsAttentionEvenWhilePluggedIn() = runComposeUiTest {
        show(c = kioskController(pluggedIn.copy(ownBatteryPct = 4, ownPlugged = true)))
        assertEquals(listOf("bar", "attention"), marks())
        onNodeWithTag("baby_kiosk_attention")
            .assertTextEquals("Needs attention: battery almost empty · 4%")
    }

    @Test
    fun theLowBatteryLineStartsBelowFifteenPercent() = runComposeUiTest {
        val c =
            kioskController(
                pluggedIn.copy(ownBatteryPct = 15, ownPlugged = false, chargerSeen = false)
            )
        show(c = c)
        assertFalse(onAllNodesWithTagExists("baby_kiosk_attention"))
        c.babyFlow.value = c.babyFlow.value.copy(ownBatteryPct = 14)
        waitForIdle()
        onNodeWithTag("baby_kiosk_attention").assertTextEquals("Needs attention: battery low · 14%")
    }

    @Test
    fun aChargerPulledDuringTheRunAddsTheSquareAndTheReason() = runComposeUiTest {
        val c = kioskController(pluggedIn)
        show(c = c)
        assertEquals(listOf("bar"), marks())
        c.babyFlow.value = c.babyFlow.value.copy(ownPlugged = false)
        waitForIdle()
        assertEquals(listOf("bar", "attention"), marks())
        onNodeWithTag("baby_kiosk_attention")
            .assertTextEquals("Needs attention: not charging · 64%")
    }

    @Test
    fun unpluggedFromTheStartShowsNoSquareWhileTheBatteryIsFine() = runComposeUiTest {
        show(c = kioskController(pluggedIn.copy(ownPlugged = false, chargerSeen = false)))
        assertEquals(listOf("bar"), marks())
        assertFalse(onAllNodesWithTagExists("baby_kiosk_attention"))
    }

    @Test
    fun aHealthyBatteryShowsNoSquare() = runComposeUiTest {
        show(c = kioskController())
        assertFalse(onAllNodesWithTagExists("baby_kiosk_attention"))
    }

    @Test
    fun duringATalkOnlyTheTalkIndicatorShows() = runComposeUiTest {
        show(c = kioskController(BabyMonitorState(link = Connected, parentTalking = true)))
        onNodeWithTag("baby_talking").assertIsDisplayed()
        assertFalse(onAllNodesWithTagExists("baby_status"))
        assertFalse(onAllNodesWithTagExists("baby_kiosk_edge"))
        assertFalse(onAllNodesWithTagExists("baby_kiosk_touch_hint"))
    }

    @Test
    fun theConfirmationNamesTheParentDeviceForTheFirstSixtySeconds() = runComposeUiTest {
        val b =
            BabyMonitorState(
                link = Connected,
                parentName = "Anna's phone",
                firstContactElapsedMs = ELAPSED - 15_000,
            )
        show(c = kioskController(b))
        onNodeWithTag("baby_kiosk_headline").assertTextEquals("Connected to Anna's phone")
        onNodeWithTag("baby_kiosk_substatus").assertTextEquals("Listening · Travel cot")
        onNodeWithTag("baby_kiosk_dims_in")
            .assertTextEquals("This screen dims in 45 s. Touch it any time to see this again.")
    }

    @Test
    fun theConfirmationFallsBackToThePlainWordsWithoutAName() = runComposeUiTest {
        val b = BabyMonitorState(link = Connected, parentName = "", firstContactElapsedMs = ELAPSED)
        show(c = kioskController(b))
        onNodeWithTag("baby_kiosk_headline").assertTextEquals("Connected to the parent device")
    }

    @Test
    fun noConfirmationBeforeTheFirstContact() = runComposeUiTest {
        show(c = kioskController(BabyMonitorState(link = link(LinkPhase.CONNECTING))))
        assertFalse(onAllNodesWithTagExists("baby_kiosk_dims_in"))
    }

    @Test
    fun theConfirmationEndsAfterSixtySeconds() = runComposeUiTest {
        val b = BabyMonitorState(link = Connected, firstContactElapsedMs = ELAPSED - 61_000)
        show(c = kioskController(b))
        assertFalse(onAllNodesWithTagExists("baby_kiosk_dims_in"))
    }

    private fun Activity.brightness() = window.attributes.screenBrightness

    @Test
    fun brightnessIsTheWakeLevelOnlyWhileTheConfirmationRunsThenTheMinimum() = runComposeUiTest {
        var env by mutableStateOf(TestEnv)
        var activity: Activity? = null
        val b = BabyMonitorState(link = Connected, firstContactElapsedMs = ELAPSED - 10_000)
        val c = kioskController(b)
        setContent {
            activity = LocalContext.current.findActivity()
            CompositionLocalProvider(LocalUiEnv provides env) {
                GuardedSleepTheme(Theme.DARK, forceDark = true) { BabyKioskScreen(c) }
            }
        }
        waitForIdle()
        assertEquals(KioskWakeBrightness, activity!!.brightness(), 0.0001f)
        // 70 s after the first contact the confirmation is over: back to the minimum.
        env = TestEnv.copy(elapsedNow = { ELAPSED + 60_000 })
        waitForIdle()
        assertEquals(KioskMinBrightness, activity!!.brightness(), 0.0001f)
    }

    @Test
    fun theBrightnessStaysAtTheMinimumWithoutAConfirmationAndDuringATalk() = runComposeUiTest {
        var activity: Activity? = null
        val b =
            BabyMonitorState(
                link = Connected,
                parentTalking = true,
                firstContactElapsedMs = ELAPSED - 10_000,
            )
        val c = kioskController(b)
        setContent {
            activity = LocalContext.current.findActivity()
            CompositionLocalProvider(LocalUiEnv provides TestEnv) {
                GuardedSleepTheme(Theme.DARK, forceDark = true) { BabyKioskScreen(c) }
            }
        }
        waitForIdle()
        assertEquals(KioskMinBrightness, activity!!.brightness(), 0.0001f)
    }

    @Test
    fun aTouchWakesTheRowsAndStaysAtTheWakeBrightness() = runComposeUiTest {
        var activity: Activity? = null
        val b =
            BabyMonitorState(
                link = Connected,
                parentName = "Anna's phone",
                lastTalk = LastTalkUi("Anna's phone", WALL, 12_000),
                ownBatteryPct = 82,
                ownPlugged = true,
            )
        val c = kioskController(b)
        setContent {
            activity = LocalContext.current.findActivity()
            CompositionLocalProvider(LocalUiEnv provides TestEnv) {
                GuardedSleepTheme(Theme.DARK, forceDark = true) { BabyKioskScreen(c) }
            }
        }
        waitForIdle()
        assertEquals(KioskMinBrightness, activity!!.brightness(), 0.0001f)
        onNodeWithTag("baby_kiosk").performTouchInput {
            down(center)
            up()
        }
        waitForIdle()
        assertEquals(KioskWakeBrightness, activity!!.brightness(), 0.0001f)
        onNodeWithTag("baby_row_connection")
            .assertTextEquals(
                "Connection",
                "Travel cot · baby device",
            )
        onNodeWithTag("baby_peer").assertTextEquals("Parent device", "Anna's phone")
        onNodeWithTag("baby_last_talk").assertTextContains("12 s", substring = true)
        onNodeWithTag("baby_kiosk_battery").assertTextEquals("This device", "charging · 82%")
    }

    private fun ComposeUiTest.unlockAction() {
        val actions =
            onNodeWithTag("baby_kiosk").fetchSemanticsNode().config[SemanticsActions.CustomActions]
        runOnUiThread { actions.first().action() }
    }

    @Test
    fun theUnlockActionAsksFirstAndOnlyTheConfirmationUnlocks() = runComposeUiTest {
        val c = kioskController()
        show(c = c)
        unlockAction()
        waitForIdle()
        onNodeWithTag("dialog_unlock").assertIsDisplayed()
        assertFalse("setKioskLocked:false" in c.calls)
        onNodeWithTag("dialog_unlock_cancel").performClick()
        waitForIdle()
        assertFalse("setKioskLocked:false" in c.calls)
        unlockAction()
        waitForIdle()
        onNodeWithTag("dialog_unlock_confirm").performClick()
        waitForIdle()
        assertTrue("setKioskLocked:false" in c.calls)
    }
}
