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
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsSelected
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
import org.guardedsleep.app.ui.screens.SettingsPage
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
            parentConnections = listOf(ConnectionUi("1", null, isDefault = true, selected = true)),
        )

    private val two =
        one.copy(
            peerName = "Home",
            connections =
                listOf(
                    ConnectionUi("1", "Home", isDefault = true, selected = true),
                    ConnectionUi("2", "Travel", isDefault = false, selected = false),
                ),
            parentConnections =
                listOf(
                    ConnectionUi("1", "Home", isDefault = true, selected = true),
                    ConnectionUi("2", "Travel", isDefault = false, selected = false),
                ),
            canAddPairing = false,
            canAddParentPairing = false,
        )

    @Test
    fun withOneSavedPairingTheHomeShowsItsRowAndSetUpNewDevices() = runComposeUiTest {
        show { HomeScreen(FakeAppController(app = one), {}) }
        onNodeWithTag("home_connection_parent_1").assertIsDisplayed()
        onNodeWithTag("home_new_pairing_parent").performScrollTo().assertIsDisplayed()
    }

    @Test
    fun withTwoBothRowsStartTheirOwnConnectionAndTheCapHidesSetUp() = runComposeUiTest {
        val c = FakeAppController(app = two)
        show { HomeScreen(c, {}) }
        onNodeWithTag("home_connection_name_parent_1", useUnmergedTree = true)
            .assertTextContains("Home")
        onNodeWithTag("home_connection_name_parent_2", useUnmergedTree = true)
            .assertTextContains("Travel")
        onNodeWithTag("home_connection_parent_2").performClick()
        assertEquals(listOf("startConnection:PARENT:2"), c.calls)
        // At the cap no further parent pairing is offered.
        onNodeWithTag("home_new_pairing_parent").assertDoesNotExist()
        onNodeWithTag("home_new_pairing_baby").assertExists()
    }

    @Test
    fun settingsListsEachConnectionWithRenameAndForget() = runComposeUiTest {
        val c = FakeAppController(app = two)
        show { SettingsScreen(c, {}, {}, {}, initialPage = SettingsPage.CONNECTIONS) }
        // The default has one home, Open at start (frame 22: no default toggle here).
        onNodeWithTag("settings_connection_default_1").assertDoesNotExist()

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
        onNodeWithTag("home_empty_parent").assertIsDisplayed()
        onNodeWithTag("home_new_pairing_parent").performClick()
        assertEquals(listOf("startPairing", "setUpNewDevices:PARENT"), c.calls)
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
        onNodeWithTag("home_empty_parent").assertIsDisplayed()
        onNodeWithTag("home_new_pairing_parent").assertIsDisplayed()
        assertEquals(listOf("startPairing", "cancelPairing"), c.calls)
    }

    private fun autoStartedRoot(parent: ParentMonitorState) =
        FakeAppController(
            app = two.copy(monitoring = true, autoStarted = true),
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
        onNodeWithTag("home_connections_parent").performScrollTo().assertIsDisplayed()
        onNodeWithTag("home_connection_parent_2").assertExists()
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
        onNodeWithTag("home_connections_parent").performScrollTo().assertIsDisplayed()
        onNodeWithTag("home_connection_parent_2").assertExists()
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
    fun homeHasNoDefaultOrAutoConnectControlsOnlyAnOpenAtStartLink() = runComposeUiTest {
        var opened = false
        show { HomeScreen(FakeAppController(app = two), {}, onOpenAtStart = { opened = true }) }
        onNodeWithTag("home_auto_connect").assertDoesNotExist()
        onNodeWithTag("role_default_toggle").assertDoesNotExist()
        onNodeWithTag("switch_mode").assertDoesNotExist()
        onNodeWithTag("home_open_at_start").performScrollTo().assertIsDisplayed().performClick()
        assertEquals(true, opened)
    }

    @Test
    fun openAtStartMarksTheAutomaticStartConnectionAndCanClearOrChangeIt() = runComposeUiTest {
        val c = FakeAppController(app = two)
        show { SettingsScreen(c, {}, {}, {}, initialPage = SettingsPage.OPEN_AT_START) }
        onNodeWithTag("settings_auto_none").performScrollTo().assertIsNotSelected()
        onNodeWithTag("settings_connection_default_1").performScrollTo().assertIsSelected()
        onNodeWithTag("settings_connection_default_2").performScrollTo().performClick()
        assertEquals("setDefaultConnection:2", c.calls.last())
        onNodeWithTag("settings_auto_none").performScrollTo().performClick()
        assertEquals("clearDefaultConnection", c.calls.last())
        onNodeWithTag("settings_auto_none").assertIsSelected()
    }

    @Test
    fun settingsShowsTheConnectionsOnTheOverviewAndOpensThePageWhenIdle() = runComposeUiTest {
        val c = FakeAppController(app = two)
        show { SettingsScreen(c, {}, {}, {}) }
        // The card is one button, so its rows only exist in the unmerged tree.
        onNodeWithTag("settings_connection_row_1", useUnmergedTree = true).assertIsDisplayed()
        onNodeWithTag("settings_connection_row_2", useUnmergedTree = true).assertIsDisplayed()
        onNodeWithTag("settings_connection_in_use").assertDoesNotExist()
        onNodeWithTag("settings_connections_read_only").assertDoesNotExist()
        onNodeWithTag("settings_connections").performClick()
        onNodeWithTag("settings_group_paired").assertIsDisplayed()
        onNodeWithTag("settings_connection_rename_2").assertIsDisplayed()
    }

    @Test
    fun whileMonitoringTheConnectionsAreReadOnlyAndTheOneInUseIsMarked() = runComposeUiTest {
        val c = FakeAppController(app = two.copy(monitoring = true))
        show { SettingsScreen(c, {}, {}, {}) }
        onNodeWithTag("settings_connections_read_only")
            .assertIsDisplayed()
            .assertTextContains("Stop monitoring to change connections", substring = true)
        onNodeWithTag("settings_connection_in_use").assertIsDisplayed()
        // The overview rows are not a way into the editable page any more.
        onNodeWithTag("settings_connections").performClick()
        onNodeWithTag("settings_group_paired").assertDoesNotExist()
        onNodeWithTag("settings_connection_rename_1").assertDoesNotExist()
    }

    @Test
    fun theConnectionsPageIsReadOnlyIfMonitoringStartsMeanwhile() = runComposeUiTest {
        val c = FakeAppController(app = two.copy(monitoring = true))
        show { SettingsScreen(c, {}, {}, {}, initialPage = SettingsPage.CONNECTIONS) }
        onNodeWithTag("settings_connections_read_only").assertIsDisplayed()
        onNodeWithTag("settings_connection_rename_1").assertDoesNotExist()
        onNodeWithTag("settings_connection_forget_1").assertDoesNotExist()
    }

    @Test
    fun anEmptyNameIsRejectedInTheDialog() = runComposeUiTest {
        val c = FakeAppController(app = one)
        show { SettingsScreen(c, {}, {}, {}, initialPage = SettingsPage.CONNECTIONS) }
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
