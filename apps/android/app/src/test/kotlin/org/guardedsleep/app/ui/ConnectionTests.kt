@file:OptIn(ExperimentalTestApi::class)

package org.guardedsleep.app.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
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
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.ConnectionUi
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
}
