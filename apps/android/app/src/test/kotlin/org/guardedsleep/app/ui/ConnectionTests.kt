@file:OptIn(ExperimentalTestApi::class)

package org.guardedsleep.app.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.runComposeUiTest
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.common.limitName
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.ConnectionUi
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.Theme
import org.guardedsleep.app.ui.screens.HomeScreen
import org.guardedsleep.app.ui.screens.SettingsScreen
import org.guardedsleep.app.ui.theme.GuardedSleepTheme
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The connection list on the parent home and Settings -> Connections (`device-setup` scenarios).
 */
class ConnectionTests : ShotBase() {
    private fun ComposeUiTest.show(content: @Composable () -> Unit) = setContent {
        CompositionLocalProvider(LocalUiEnv provides TestEnv) {
            GuardedSleepTheme(Theme.DARK, content = content)
        }
    }

    private val one =
        AppState(
            role = Role.PARENT,
            onboardingDone = true,
            paired = true,
            connections = listOf(ConnectionUi("1", null, isDefault = true, selected = true)),
        )

    private val two =
        one.copy(
            peerName = "Home",
            connections =
                listOf(
                    ConnectionUi("1", "Home", isDefault = true, selected = true),
                    ConnectionUi("2", "Travel", isDefault = false, selected = false),
                ),
            canAddPairing = false,
        )

    @Test
    fun withOneSavedPairingTheHomeShowsNoList() = runComposeUiTest {
        show { HomeScreen(FakeAppController(app = one), {}) }
        onNodeWithTag("home_card").assertIsDisplayed()
        onNodeWithTag("home_connection_change").assertDoesNotExist()
        onNodeWithTag("home_connection_current").assertDoesNotExist()
        onNodeWithTag("home_new_pairing").assertIsDisplayed()
    }

    @Test
    fun withTwoTheDefaultIsPreselectedAndChangeOpensTheList() = runComposeUiTest {
        val c = FakeAppController(app = two)
        show { HomeScreen(c, {}) }
        onNodeWithTag("home_connection_current")
            .performScrollTo()
            .assertTextContains("Home", substring = true)
        onNodeWithTag("home_connection_list").assertDoesNotExist()
        onNodeWithTag("home_connection_change").performScrollTo().performClick()
        onNodeWithTag("connection_choice_2").performScrollTo().performClick()
        assertEquals(listOf("selectConnection:2"), c.calls)
        onNodeWithTag("home_connection_current").assertTextContains("Travel", substring = true)
        onNodeWithTag("home_connection_list").assertDoesNotExist()
        // At the cap no further pairing is offered.
        onNodeWithTag("home_new_pairing").assertDoesNotExist()
    }

    @Test
    fun settingsListsEachConnectionWithRenameDefaultAndForget() = runComposeUiTest {
        val c = FakeAppController(app = two)
        show { SettingsScreen(c, {}, {}, {}) }
        onNodeWithTag("settings_connection_default_1").assertDoesNotExist()
        onNodeWithTag("settings_connection_default_2").performScrollTo().performClick()
        assertEquals("setDefaultConnection:2", c.calls.last())

        onNodeWithTag("settings_connection_rename_2").performScrollTo().performClick()
        onNodeWithTag("rename_connection_field").performTextInput("Gran")
        onNodeWithTag("rename_connection_save").performClick()
        assertEquals("renameConnection:2", c.calls.last())
        assertEquals("GranTravel", c.appFlow.value.connections[1].name)
        onNodeWithTag("dialog_rename_connection").assertDoesNotExist()

        onNodeWithTag("settings_connection_forget_1").performScrollTo().performClick()
        onNodeWithTag("dialog_forget_confirm").performClick()
        assertEquals("forgetConnection:1", c.calls.last())
        assertEquals(listOf("2"), c.appFlow.value.connections.map { it.id })
    }

    private val empty = AppState(role = Role.PARENT, onboardingDone = true)

    @Test
    fun theEmptyParentHomeOpensTheScanScreenOnceAndTheButtonWorks() = runComposeUiTest {
        val c = FakeAppController(app = empty)
        var scanned by mutableStateOf(false)
        show { HomeScreen(c, {}, autoScanned = scanned, onAutoScanned = { scanned = true }) }
        waitForIdle()
        assertEquals(listOf("startPairing"), c.calls)
        onNodeWithTag("home_empty").assertIsDisplayed()
        onNodeWithTag("home_new_pairing").performClick()
        assertEquals(listOf("startPairing", "startPairing"), c.calls)
    }

    @Test
    fun theEmptyParentGoesThroughTheScanScreenAndBackWithoutRescanning() = runComposeUiTest {
        val c = FakeAppController(app = empty)
        setContent {
            // Camera denied: the paste view, so no native scanner library is needed on the JVM.
            CompositionLocalProvider(LocalUiEnv provides TestEnv.copy(cameraGranted = false)) {
                GuardedSleepRoot(c)
            }
        }
        waitForIdle()
        assertEquals(listOf("startPairing"), c.calls)
        onNodeWithTag("parent_pair").assertIsDisplayed()
        c.cancelPairing()
        waitForIdle()
        onNodeWithTag("home_empty").assertIsDisplayed()
        assertEquals(listOf("startPairing", "cancelPairing"), c.calls)
    }

    private fun autoStartedRoot(parent: ParentMonitorState) =
        FakeAppController(
            app = two.copy(monitoring = true, autoStarted = true, canAddPairing = false),
            parent = parent,
        )

    @Test
    fun chooseAnotherIsAButtonBeforeVerifyAndLandsOnTheList() = runComposeUiTest {
        val c = autoStartedRoot(ParentMonitorState(link = link(LinkPhase.CONNECTING)))
        setContent { CompositionLocalProvider(LocalUiEnv provides TestEnv) { GuardedSleepRoot(c) } }
        onNodeWithTag("menu_choose_other_connection").assertDoesNotExist()
        onNodeWithTag("parent_choose_other_connection").performScrollTo().performClick()
        assertEquals("chooseAnotherConnection", c.calls.last())
        waitForIdle()
        onNodeWithTag("home_connection_list").performScrollTo().assertIsDisplayed()
        onNodeWithTag("connection_choice_2").assertExists()
    }

    @Test
    fun chooseAnotherIsAMenuEntryWithConfirmationAfterVerify() = runComposeUiTest {
        val c = autoStartedRoot(ParentMonitorState(link = Connected))
        setContent { CompositionLocalProvider(LocalUiEnv provides TestEnv) { GuardedSleepRoot(c) } }
        onNodeWithTag("parent_choose_other_connection").assertDoesNotExist()
        onNodeWithTag("parent_overflow").performClick()
        onNodeWithTag("menu_choose_other_connection").performClick()
        onNodeWithTag("dialog_choose_other").assertIsDisplayed()
        assertEquals(emptyList<String>(), c.calls)
        onNodeWithTag("dialog_choose_other_confirm").performClick()
        assertEquals("chooseAnotherConnection", c.calls.last())
        waitForIdle()
        onNodeWithTag("home_connection_list").performScrollTo().assertIsDisplayed()
    }

    @Test
    fun aSavedConnectionDoesNotScanOnEntry() = runComposeUiTest {
        val c = FakeAppController(app = one)
        show { HomeScreen(c, {}, autoScanned = false) }
        waitForIdle()
        assertEquals(emptyList<String>(), c.calls)
    }

    @Test
    fun theBabyHomeDoesNotScanOnEntry() = runComposeUiTest {
        val baby = FakeAppController(app = empty.copy(role = Role.BABY))
        show { HomeScreen(baby, {}, autoScanned = false) }
        waitForIdle()
        assertEquals(emptyList<String>(), baby.calls)
    }

    @Test
    fun theAutoConnectSwitchFollowsTheSelectedConnectionAlsoWithOne() = runComposeUiTest {
        val c =
            FakeAppController(
                app =
                    one.copy(
                        connections =
                            listOf(ConnectionUi("1", null, isDefault = false, selected = true))
                    )
            )
        show { HomeScreen(c, {}) }
        onNodeWithTag("home_auto_connect").performScrollTo().performClick()
        assertEquals("setDefaultConnection:1", c.calls.last())
        onNodeWithTag("home_auto_connect").performScrollTo().performClick()
        assertEquals("clearDefaultConnection", c.calls.last())
    }

    @Test
    fun settingsOffersRemoveDefaultOnTheDefaultOnly() = runComposeUiTest {
        val c = FakeAppController(app = two)
        show { SettingsScreen(c, {}, {}, {}) }
        onNodeWithTag("settings_connection_remove_default_2").assertDoesNotExist()
        onNodeWithTag("settings_connection_remove_default_1").performScrollTo().performClick()
        assertEquals("clearDefaultConnection", c.calls.last())
    }

    @Test
    fun anEmptyNameIsRejectedInTheDialog() = runComposeUiTest {
        val c = FakeAppController(app = one)
        show { SettingsScreen(c, {}, {}, {}) }
        // Single connection keeps today's rows plus rename.
        onNodeWithTag("settings_peer_name").performScrollTo().assertIsDisplayed()
        onNodeWithTag("settings_connection_rename_1").performScrollTo().performClick()
        onNodeWithTag("rename_connection_save").performClick()
        onNodeWithTag("dialog_rename_connection").assertIsDisplayed()
        onNodeWithTag("settings_forget").assertExists()
    }

    @Test
    fun theNameLimitCountsCodePointsNotChars() {
        val emoji = "\uD83D\uDE34" // two chars, one code point
        assertEquals(emoji.repeat(30), limitName(emoji.repeat(30)))
        assertEquals(emoji.repeat(30), limitName(emoji.repeat(40)))
        assertEquals("a".repeat(30), limitName("a".repeat(31)))
    }
}
