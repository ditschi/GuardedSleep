@file:OptIn(ExperimentalTestApi::class)

package org.guardedsleep.app.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runComposeUiTest
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.model.AlarmTonePlayback
import org.guardedsleep.app.ui.model.AlarmTones
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.SettingsState
import org.guardedsleep.app.ui.model.Theme
import org.guardedsleep.app.ui.screens.AlarmTonePickerScreen
import org.guardedsleep.app.ui.screens.SettingsPage
import org.guardedsleep.app.ui.screens.SettingsScreen
import org.guardedsleep.app.ui.theme.GuardedSleepTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The connection alarm tone row and picker (`add-connection-alarm-tone` 2.3). */
class AlarmToneTests : ShotBase() {
    private fun ComposeUiTest.show(content: @Composable () -> Unit) = setContent {
        CompositionLocalProvider(LocalUiEnv provides TestEnv) {
            GuardedSleepTheme(Theme.DARK, content = content)
        }
    }

    private fun parent(tone: String = AlarmTones.DEFAULT) =
        FakeAppController(
            app = AppState(role = Role.PARENT, onboardingDone = true, paired = true),
            settings = SettingsState(connectionAlarmTone = tone),
        )

    @Test
    fun settingsRowShowsTheCurrentToneAndOpensThePicker() = runComposeUiTest {
        var opened = false
        show {
            SettingsScreen(
                parent("beacon"),
                {},
                {},
                {},
                onOpenAlarmTone = { opened = true },
                initialPage = SettingsPage.ALERTS,
            )
        }
        onNodeWithTag("settings_alarm_tone").performScrollTo().assertIsDisplayed()
        onNodeWithTag("settings_alarm_tone_name", useUnmergedTree = true).assertIsDisplayed()
        onNodeWithTag("settings_alarm_tone").performClick()
        assertTrue(opened)
    }

    @Test
    fun theBabyDeviceHasNoAlarmToneRow() = runComposeUiTest {
        val baby =
            FakeAppController(
                app = AppState(role = Role.BABY, onboardingDone = true, paired = true)
            )
        show { SettingsScreen(baby, {}, {}, {}, initialPage = SettingsPage.TALK) }
        onNodeWithTag("settings_group_talk").assertIsDisplayed()
        assertTrue(onAllNodesWithTagCount("settings_alarm_tone") == 0)
    }

    private fun ComposeUiTest.onAllNodesWithTagCount(tag: String) =
        onAllNodesWithTag(tag).fetchSemanticsNodes().size

    @Test
    fun theDefaultIsSelectedAndAChoiceIsStored() = runComposeUiTest {
        val c = parent()
        show { AlarmTonePickerScreen(c, {}) }
        onNodeWithTag("alarm_tone_classic").assertIsSelected()
        onNodeWithTag("alarm_tone_chime").performScrollTo().performClick()
        assertEquals("chime", c.settingsFlow.value.connectionAlarmTone)
        onNodeWithTag("alarm_tone_chime").assertIsSelected()
    }

    @Test
    fun theRowsKeepTheDesignOrderAndLowCarriesTheHearingTag() = runComposeUiTest {
        show { AlarmTonePickerScreen(parent(), {}) }
        assertEquals(listOf("classic", "low", "beacon", "rising", "chime"), AlarmTones.IDS)
        AlarmTones.IDS.forEach {
            onNodeWithTag("alarm_tone_$it").performScrollTo().assertIsDisplayed()
            onNodeWithTag("alarm_tone_preview_$it").performScrollTo().assertIsDisplayed()
        }
        onNodeWithTag("alarm_tone_tag_low", useUnmergedTree = true)
            .performScrollTo()
            .assertIsDisplayed()
        onNodeWithTag("alarm_tone_hint").performScrollTo().assertIsDisplayed()
        onNodeWithTag("alarm_tone_explanation").assertIsDisplayed()
    }

    @Test
    fun playTurnsIntoStopWhilePlayingAndStopEndsIt() = runComposeUiTest {
        val c = parent()
        show { AlarmTonePickerScreen(c, {}) }
        onNodeWithTag("alarm_tone_preview_low").performScrollTo().performClick()
        assertEquals(AlarmTonePlayback.Preview("low"), c.playbackFlow.value)
        // The button of the playing tone is now a stop button with the same tag.
        onNodeWithTag("alarm_tone_preview_low").performClick()
        assertEquals(AlarmTonePlayback.Idle, c.playbackFlow.value)
        assertTrue("previewAlarmTone:low" in c.calls)
    }

    @Test
    fun theTestPlaysTheSelectedToneAndStops() = runComposeUiTest {
        val c = parent("rising")
        show { AlarmTonePickerScreen(c, {}) }
        onNodeWithTag("alarm_tone_test_warning").performScrollTo().assertIsDisplayed()
        onNodeWithTag("alarm_tone_test").performScrollTo().performClick()
        assertEquals(AlarmTonePlayback.Test("rising"), c.playbackFlow.value)
        onNodeWithTag("alarm_tone_test_stop").performScrollTo().performClick()
        assertEquals(AlarmTonePlayback.Idle, c.playbackFlow.value)
    }

    @Test
    fun leavingThePickerStopsAPreview() = runComposeUiTest {
        val c = parent()
        val visible = mutableStateOf(true)
        show { if (visible.value) AlarmTonePickerScreen(c, {}) }
        onNodeWithTag("alarm_tone_preview_beacon").performScrollTo().performClick()
        visible.value = false
        waitForIdle()
        assertEquals(AlarmTonePlayback.Idle, c.playbackFlow.value)
    }
}
