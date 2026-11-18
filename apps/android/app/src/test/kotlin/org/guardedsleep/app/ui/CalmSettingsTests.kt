@file:OptIn(ExperimentalTestApi::class)

package org.guardedsleep.app.ui

import androidx.activity.ComponentActivity
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.AndroidComposeUiTest
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runAndroidComposeUiTest
import androidx.compose.ui.test.runComposeUiTest
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.model.AlertStyle
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.ConnectionUi
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.SettingsState
import org.guardedsleep.app.ui.model.Theme
import org.guardedsleep.app.ui.screens.HomeScreen
import org.guardedsleep.app.ui.screens.ParentMonitorScreen
import org.guardedsleep.app.ui.screens.ParentSheet
import org.guardedsleep.app.ui.screens.RoleScreen
import org.guardedsleep.app.ui.screens.SettingsPage
import org.guardedsleep.app.ui.screens.SettingsScreen
import org.guardedsleep.app.ui.theme.GuardedSleepTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The Settings and Home restyle and the back behaviour of `redesign-calm-ui` group 4: the two
 * Settings groups, Open at start, the privacy and Vibrate-only lines, back and Close on sheets.
 */
class CalmSettingsTests : ShotBase() {
    private fun ComposeUiTest.show(content: @Composable () -> Unit) = setContent {
        CompositionLocalProvider(LocalUiEnv provides TestEnv) {
            GuardedSleepTheme(Theme.DARK, content = content)
        }
    }

    private val parentApp =
        AppState(
            role = Role.PARENT,
            onboardingDone = true,
            paired = true,
            peerName = "Nursery",
            connections = listOf(ConnectionUi("1", "Nursery", isDefault = false, selected = true)),
        )

    // ---- Settings structure ----------------------------------------------------------------

    @Test
    fun theOverviewHasTheConnectionsAndThisDeviceGroups() = runComposeUiTest {
        show { SettingsScreen(FakeAppController(app = parentApp), {}, {}, {}) }
        onNodeWithTag("settings_group_connections").assertIsDisplayed()
        onNodeWithTag("settings_group_phone").performScrollTo().assertIsDisplayed()
        for (row in
            listOf(
                "settings_nav_alerts",
                "settings_nav_display",
                "settings_nav_open_at_start",
                "settings_open_setup",
                "settings_nav_language",
                "settings_nav_about",
                "settings_nav_advanced",
            )) {
            onNodeWithTag(row).performScrollTo().assertIsDisplayed()
        }
    }

    @Test
    fun theAlarmToneIsUnderAlertsAndTheServerAddressesUnderAdvanced() =
        runAndroidComposeUiTest<ComponentActivity> {
            val c =
                FakeAppController(app = parentApp, settings = SettingsState(advancedEnabled = true))
            show { SettingsScreen(c, {}, {}, {}) }
            onNodeWithTag("settings_alarm_tone").assertDoesNotExist()
            onNodeWithTag("settings_nav_alerts").performScrollTo().performClick()
            onNodeWithTag("settings_alarm_tone").performScrollTo().assertIsDisplayed()
            activeBack()
            onNodeWithTag("settings_url_new").assertDoesNotExist()
            onNodeWithTag("settings_nav_advanced").performScrollTo().performClick()
            onNodeWithTag("settings_url_new").performScrollTo().assertIsDisplayed()
        }

    @Test
    fun theBabyDeviceOffersItsOwnRowsAndNoAlertsOrDisplay() = runComposeUiTest {
        val baby = AppState(role = Role.BABY, onboardingDone = true, paired = true)
        show { SettingsScreen(FakeAppController(app = baby), {}, {}, {}) }
        onNodeWithTag("settings_nav_detection").performScrollTo().assertIsDisplayed()
        onNodeWithTag("settings_nav_silence").assertIsDisplayed()
        onNodeWithTag("settings_nav_talk").assertIsDisplayed()
        onNodeWithTag("settings_nav_alerts").assertDoesNotExist()
        onNodeWithTag("settings_nav_display").assertDoesNotExist()
    }

    @Test
    fun theOpenAtStartRowSummarisesTheStartMode() = runComposeUiTest {
        val c = FakeAppController(app = parentApp.copy(defaultRole = Role.BABY))
        show { SettingsScreen(c, {}, {}, {}) }
        onNodeWithTag("settings_nav_open_at_start").performScrollTo().performClick()
        onNodeWithTag("start_mode_baby").assertIsDisplayed()
        onNodeWithTag("settings_group_auto_start").performScrollTo().assertIsDisplayed()
    }

    @Test
    fun theBabyDeviceHasNoAutomaticStartChoiceWithOneConnection() = runComposeUiTest {
        val c =
            FakeAppController(
                app = parentApp.copy(role = Role.BABY),
                settings = SettingsState(),
            )
        show { SettingsScreen(c, {}, {}, {}, initialPage = SettingsPage.OPEN_AT_START) }
        onNodeWithTag("settings_auto_none").assertDoesNotExist()
        onNodeWithTag("settings_connection_default_1").performScrollTo().assertIsDisplayed()
    }

    // ---- back behaviour --------------------------------------------------------------------

    private fun AndroidComposeUiTest<ComponentActivity>.activeBack() {
        runOnUiThread { activity!!.onBackPressedDispatcher.onBackPressed() }
        waitForIdle()
    }

    @Test
    fun backOnASettingsPageReturnsToTheOverviewThenLeaves() =
        runAndroidComposeUiTest<ComponentActivity> {
            var left = 0
            setContent {
                CompositionLocalProvider(LocalUiEnv provides TestEnv) {
                    GuardedSleepTheme(Theme.DARK) {
                        SettingsScreen(
                            FakeAppController(app = parentApp),
                            { left++ },
                            {},
                            {},
                            initialPage = SettingsPage.ALERTS,
                        )
                    }
                }
            }
            onNodeWithTag("settings_group_alerts").assertIsDisplayed()
            activeBack()
            onNodeWithTag("settings_group_alerts").assertDoesNotExist()
            onNodeWithTag("settings_group_connections").assertIsDisplayed()
            assertEquals(0, left)
            activeBack()
            assertEquals(1, left)
        }

    private val monitoring = parentApp.copy(monitoring = true)

    @Test
    fun backOnTheMonitorMovesTheAppToTheBackgroundAndKeepsMonitoring() =
        runAndroidComposeUiTest<ComponentActivity> {
            var away = 0
            val c =
                FakeAppController(app = monitoring, parent = ParentMonitorState(link = Connected))
            setContent {
                CompositionLocalProvider(
                    LocalUiEnv provides TestEnv.copy(moveToBackground = { away++ })
                ) {
                    GuardedSleepRoot(c)
                }
            }
            activeBack()
            assertEquals(1, away)
            assertTrue(c.appFlow.value.monitoring)
            assertFalse("stopMonitoring" in c.calls)
        }

    @Test
    fun backOnASheetClosesTheSheetAndStaysOnTheMonitor() =
        runAndroidComposeUiTest<ComponentActivity> {
            var away = 0
            val c =
                FakeAppController(app = monitoring, parent = ParentMonitorState(link = Connected))
            setContent {
                CompositionLocalProvider(
                    LocalUiEnv provides TestEnv.copy(moveToBackground = { away++ })
                ) {
                    GuardedSleepRoot(c)
                }
            }
            onNodeWithTag("parent_snooze").performClick()
            onNodeWithTag("sheet_snooze").assertIsDisplayed()
            activeBack()
            onNodeWithTag("sheet_snooze").assertDoesNotExist()
            assertEquals(0, away)
        }

    @Test
    fun backOnAFullScreenAlertDoesNothing() =
        runAndroidComposeUiTest<ComponentActivity> {
            var closed = 0
            val c =
                FakeAppController(
                    app = monitoring,
                    parent =
                        ParentMonitorState(
                            link =
                                link(
                                    org.guardedsleep.app.ui.model.LinkPhase.RECONNECTING,
                                    2,
                                    72_000,
                                )
                        ),
                )
            setContent {
                CompositionLocalProvider(LocalUiEnv provides TestEnv) {
                    LinkAlarmScreen(c) { closed++ }
                }
            }
            activeBack()
            assertEquals(0, closed)
            assertFalse(activity!!.isFinishing)
        }

    @Test
    fun everySheetHasAVisibleCloseButton() = runComposeUiTest {
        val c = FakeAppController(app = monitoring, parent = ParentMonitorState(link = Connected))
        show {
            ParentMonitorScreen(c, onSettings = {}, initialSheet = ParentSheet.SNOOZE)
        }
        onNodeWithTag("sheet_close").assertIsDisplayed().performClick()
        onNodeWithTag("sheet_snooze").assertDoesNotExist()
    }

    // ---- Home ------------------------------------------------------------------------------

    @Test
    fun homeShowsThePrivacyLineAndAStartButtonThatStartsWithOneTap() = runComposeUiTest {
        val c = FakeAppController(app = parentApp)
        show { HomeScreen(c, {}) }
        onNodeWithTag("home_privacy").performScrollTo().assertIsDisplayed()
        onNodeWithTag("home_start", useUnmergedTree = true).assertIsDisplayed()
        onNodeWithTag("home_card").performClick()
        assertEquals(listOf("startMonitoring"), c.calls)
    }

    @Test
    fun theFirstStartShowsThePrivacyLine() = runComposeUiTest {
        show { RoleScreen(FakeAppController(app = AppState())) }
        onNodeWithTag("role_privacy_note").performScrollTo().assertIsDisplayed()
    }

    @Test
    fun vibrateOnlyIsRemindedOnTheParentHomeOnly() = runComposeUiTest {
        val vibrate = SettingsState(alertStyle = AlertStyle.DISCREET)
        val c = FakeAppController(app = parentApp, settings = vibrate)
        show { HomeScreen(c, {}) }
        onNodeWithTag("home_vibrate_only").performScrollTo().assertIsDisplayed()
        // No prompt: Start still begins monitoring with one tap.
        onNodeWithTag("home_card").performClick()
        assertEquals(listOf("startMonitoring"), c.calls)
    }

    @Test
    fun noVibrateOnlyLineInSoundStyleOrOnTheBabyHome() = runComposeUiTest {
        show { HomeScreen(FakeAppController(app = parentApp), {}) }
        onNodeWithTag("home_vibrate_only").assertDoesNotExist()
    }

    @Test
    fun noVibrateOnlyLineOnTheBabyHome() = runComposeUiTest {
        val c =
            FakeAppController(
                app = parentApp.copy(role = Role.BABY),
                settings = SettingsState(alertStyle = AlertStyle.DISCREET),
            )
        show { HomeScreen(c, {}) }
        onNodeWithTag("home_vibrate_only").assertDoesNotExist()
    }
}
