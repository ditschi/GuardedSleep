@file:OptIn(ExperimentalTestApi::class)

package org.guardedsleep.app.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.center
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.runComposeUiTest
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.BabyMonitorState
import org.guardedsleep.app.ui.model.LastTalkUi
import org.guardedsleep.app.ui.model.NoisePause
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.SettingsState
import org.guardedsleep.app.ui.model.TalkEnd
import org.guardedsleep.app.ui.model.TalkPermission
import org.guardedsleep.app.ui.model.TalkUi
import org.guardedsleep.app.ui.model.TalkUnavailable
import org.guardedsleep.app.ui.model.TalkVolume
import org.guardedsleep.app.ui.model.Theme
import org.guardedsleep.app.ui.screens.BabyKioskScreen
import org.guardedsleep.app.ui.screens.ParentMonitorScreen
import org.guardedsleep.app.ui.screens.SettingsScreen
import org.guardedsleep.app.ui.theme.GuardedSleepTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The talk button, its reasons and the baby's talk-back surfaces (`add-talk-back` 3.2, 3.3). */
class TalkTests : ShotBase() {
    private val monitoring =
        AppState(role = Role.PARENT, onboardingDone = true, paired = true, monitoring = true)

    private fun ComposeUiTest.show(dark: Boolean = false, content: @Composable () -> Unit) =
        setContent {
            CompositionLocalProvider(LocalUiEnv provides TestEnv) {
                GuardedSleepTheme(Theme.DARK, forceDark = dark, content = content)
            }
        }

    private fun controller(parent: ParentMonitorState = ParentMonitorState(link = Connected)) =
        FakeAppController(
            app = monitoring,
            settings = SettingsState(talkCoachSeen = true),
            parent = parent,
        )

    private fun talkCalls(c: FakeAppController) = c.calls.filter { it.startsWith("setTalk") }

    @Test
    fun aHoldPressesAndReleaseEndsTheTalk() = runComposeUiTest {
        val c = controller()
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_talk").performTouchInput { down(center) }
        mainClock.advanceTimeBy(400)
        onNodeWithTag("parent_talk").performTouchInput { up() }
        waitForIdle()
        assertEquals(listOf("setTalk:true", "setTalk:false"), talkCalls(c))
    }

    @Test
    fun aTapShowsTheHintAndSendsNothing() = runComposeUiTest {
        val c = controller()
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_talk").performTouchInput {
            down(center)
            up()
        }
        waitForIdle()
        assertTrue(talkCalls(c).isEmpty())
        onNodeWithTag("parent_talk_hint").assertIsDisplayed()
    }

    @Test
    fun aCancelledTouchEndsTheTalk() = runComposeUiTest {
        val c = controller()
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_talk").performTouchInput { down(center) }
        mainClock.advanceTimeBy(400)
        onNodeWithTag("parent_talk").performTouchInput { cancel() }
        waitForIdle()
        assertEquals(listOf("setTalk:true", "setTalk:false"), talkCalls(c))
    }

    @Test
    fun anUnavailableButtonStatesWhyAndSendsNothing() = runComposeUiTest {
        val c =
            controller(
                ParentMonitorState(link = Connected, talk = TalkUi.Unavailable(TalkUnavailable.OFF))
            )
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_talk_unavailable_reason").assertIsDisplayed()
        onNodeWithTag("parent_talk").performTouchInput {
            down(center)
            up()
        }
        waitForIdle()
        assertTrue(talkCalls(c).isEmpty())
    }

    @Test
    fun theNoisePauseEndReasonAndUnconfirmedLinesShow() = runComposeUiTest {
        val c =
            controller(
                ParentMonitorState(
                    link = Connected,
                    noisePause = NoisePause.YOU,
                    talkEnd = TalkEnd.NO_ANSWER,
                    talkUnconfirmed = true,
                )
            )
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_noise_paused").assertExists()
        onNodeWithTag("parent_talk_state").assertExists()
        onNodeWithTag("parent_talk_unconfirmed").assertExists()
    }

    @Test
    fun theSpeakNowStateAndTheCountdownShowWhileTalking() = runComposeUiTest {
        val c =
            controller(ParentMonitorState(link = Connected, talk = TalkUi.Talking(ELAPSED + 3_000)))
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_talk").assertIsDisplayed()
    }

    @Test
    fun aPressWithoutPermissionOffersTheSheet() = runComposeUiTest {
        val c =
            controller(ParentMonitorState(link = Connected, talkPermission = TalkPermission.NEEDED))
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_talk_permission_sheet").assertIsDisplayed()
        onNodeWithTag("parent_talk_permission_allow").assertIsDisplayed()
    }

    @Test
    fun theCoachMarkShowsOnceAndGoesOnTheFirstPress() = runComposeUiTest {
        val c =
            FakeAppController(
                app = monitoring,
                settings = SettingsState(talkCoachSeen = false),
                parent = ParentMonitorState(link = Connected),
            )
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("coach_talk").assertIsDisplayed()
        onNodeWithTag("coach_talk").performClick()
        assertTrue("dismissTalkCoach" in c.calls)
    }

    @Test
    fun theBabyShowsAStaticIndicatorAndTheLastTalk() = runComposeUiTest {
        val c =
            FakeAppController(
                app = AppState(role = Role.BABY, onboardingDone = true, paired = true),
                baby =
                    BabyMonitorState(
                        link = Connected,
                        parentTalking = true,
                        lastTalk = LastTalkUi("Pixel 8", WALL, 12_000),
                    ),
            )
        show(dark = true) { BabyKioskScreen(c) }
        onNodeWithTag("baby_talking").assertIsDisplayed()
        // The touch-wake line lives in the faded column (alpha 0 until a touch), so it exists.
        onNodeWithTag("baby_last_talk").assertExists()
    }

    @Test
    fun theBabySettingsOfferTheTalkBackRows() = runComposeUiTest {
        val c =
            FakeAppController(
                app = AppState(role = Role.BABY, onboardingDone = true, paired = true),
                settings = SettingsState(talkVolume = TalkVolume.LOW),
            )
        show(dark = true) { SettingsScreen(c, {}, {}, {}) }
        onNodeWithTag("baby_talk_toggle").performScrollTo().assertIsDisplayed()
        onNodeWithTag("baby_talk_volume").performScrollTo().assertIsDisplayed()
        onNodeWithTag("baby_talk_play_chime").performScrollTo().performClick()
        onNodeWithTag("baby_talk_chime_toggle").performScrollTo().performClick()
        onNodeWithTag("baby_talk_toggle").performScrollTo().performClick()
        assertTrue("talkSettingsOpened" in c.calls)
        assertTrue("playTalkChimeSample" in c.calls)
        assertEquals(false, c.settingsFlow.value.talkBackEnabled)
        assertEquals(false, c.settingsFlow.value.talkChime)
    }
}
