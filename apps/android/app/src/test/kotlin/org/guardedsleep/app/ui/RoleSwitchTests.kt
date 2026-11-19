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
import androidx.compose.ui.test.runComposeUiTest
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.ConnectionUi
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.Theme
import org.guardedsleep.app.ui.screens.HomeScreen
import org.guardedsleep.app.ui.screens.RoleScreen
import org.guardedsleep.app.ui.screens.SettingsPage
import org.guardedsleep.app.ui.screens.SettingsScreen
import org.guardedsleep.app.ui.theme.GuardedSleepTheme
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * First-start choice, one Home with both roles and the start mode as the UI drives the controller.
 */
class RoleSwitchTests : ShotBase() {
    private fun ComposeUiTest.show(content: @Composable () -> Unit) = setContent {
        CompositionLocalProvider(LocalUiEnv provides TestEnv) {
            GuardedSleepTheme(Theme.DARK, content = content)
        }
    }

    private val idle = AppState(role = Role.PARENT, onboardingDone = true, paired = true)

    private val twoRoles =
        idle.copy(
            role = null,
            lastRole = Role.PARENT,
            parentConnections =
                listOf(
                    ConnectionUi("1", "Nursery", isDefault = false, selected = true),
                    ConnectionUi("2", "Grandma", isDefault = false, selected = false),
                ),
            babyConnections = listOf(ConnectionUi("1", "Travel cot", false, true)),
        )

    @Test
    fun firstStartShowsTwoLargeChoicesWithoutUseAsDefault() = runComposeUiTest {
        val c = FakeAppController(app = AppState())
        show { RoleScreen(c) }
        onNodeWithTag("role_baby").assertIsDisplayed()
        onNodeWithTag("role_parent").assertIsDisplayed()
        onNodeWithTag("role_default_toggle").assertDoesNotExist()
        onNodeWithTag("role_baby").performClick()
        assertEquals(listOf("chooseRole:BABY"), c.calls)
        assertEquals(null, c.appFlow.value.defaultRole)
    }

    @Test
    fun noPairingAnywhereShowsTheFirstStartChoiceAtTheRoot() = runComposeUiTest {
        setContent {
            CompositionLocalProvider(LocalUiEnv provides TestEnv) {
                GuardedSleepRoot(FakeAppController(app = AppState()))
            }
        }
        onNodeWithTag("role_title").assertIsDisplayed()
    }

    @Test
    fun aPairingInEitherRoleShowsHomeAtTheRootWhileTheStartModeAsks() = runComposeUiTest {
        setContent {
            CompositionLocalProvider(LocalUiEnv provides TestEnv) {
                GuardedSleepRoot(FakeAppController(app = twoRoles.copy(onboardingDone = true)))
            }
        }
        onNodeWithTag("home").assertIsDisplayed()
        onNodeWithTag("role_title").assertDoesNotExist()
    }

    @Test
    fun homeShowsBothRolesWithTheirConnectionsAndStartPicksTheRole() = runComposeUiTest {
        val c = FakeAppController(app = twoRoles)
        show { HomeScreen(c, {}) }
        onNodeWithTag("home_role_header_parent").assertIsDisplayed()
        onNodeWithTag("home_connection_name_parent_1", useUnmergedTree = true)
            .assertTextContains("Nursery")
        onNodeWithTag("home_connection_name_parent_2", useUnmergedTree = true)
            .assertTextContains("Grandma")
        onNodeWithTag("home_role_header_baby").performScrollTo().assertIsDisplayed()
        onNodeWithTag("home_connection_name_baby_1", useUnmergedTree = true)
            .performScrollTo()
            .assertTextContains("Travel cot")
        onNodeWithTag("home_connection_baby_1").performClick()
        assertEquals(listOf("startConnection:BABY:1"), c.calls)
        assertEquals(Role.BABY, c.appFlow.value.role)
        assertEquals(null, c.appFlow.value.defaultRole)
    }

    @Test
    fun homeOffersSetUpNewDevicesPerRoleAndNoSwitchMode() = runComposeUiTest {
        val c = FakeAppController(app = twoRoles.copy(canAddParentPairing = true))
        show { HomeScreen(c, {}) }
        onNodeWithTag("switch_mode").assertDoesNotExist()
        onNodeWithTag("home_new_pairing_baby").performScrollTo().performClick()
        assertEquals(listOf("setUpNewDevices:BABY"), c.calls)
    }

    @Test
    fun homeShowsTheStartModeInTheOpenAtStartButton() = runComposeUiTest {
        var opened = 0
        val c = FakeAppController(app = twoRoles.copy(defaultRole = Role.BABY))
        show { HomeScreen(c, {}, onOpenAtStart = { opened++ }) }
        onNodeWithTag("home_open_at_start")
            .performScrollTo()
            .assertTextContains("Baby device", substring = true)
            .performClick()
        assertEquals(1, opened)
    }

    @Test
    fun theOpenAtStartLinkOpensSettingsOnThatPage() = runComposeUiTest {
        setContent {
            CompositionLocalProvider(LocalUiEnv provides TestEnv) {
                GuardedSleepRoot(FakeAppController(app = twoRoles.copy(onboardingDone = true)))
            }
        }
        onNodeWithTag("home_open_at_start").performScrollTo().performClick()
        onNodeWithTag("start_mode_ask").assertIsDisplayed()
    }

    @Test
    fun noRoleOrConnectionSwitchIsOfferedWhileMonitoring() = runComposeUiTest {
        val c =
            FakeAppController(
                app = twoRoles.copy(role = Role.PARENT, monitoring = true, onboardingDone = true),
                parent = ParentMonitorState(link = Connected),
            )
        setContent { CompositionLocalProvider(LocalUiEnv provides TestEnv) { GuardedSleepRoot(c) } }
        onNodeWithTag("home").assertDoesNotExist()
        onNodeWithTag("home_connection_parent_2").assertDoesNotExist()
        onNodeWithTag("switch_mode").assertDoesNotExist()
        onNodeWithTag("parent_choose_other_connection").assertDoesNotExist()
    }

    @Test
    fun startModeRadioGroupSetsTheDefault() = runComposeUiTest {
        val c = FakeAppController(app = idle)
        show { SettingsScreen(c, {}, {}, {}, initialPage = SettingsPage.OPEN_AT_START) }
        onNodeWithTag("start_mode_ask").performScrollTo().assertIsDisplayed()
        onNodeWithTag("start_mode_baby").performScrollTo().performClick()
        assertEquals(Role.BABY, c.appFlow.value.defaultRole)
        onNodeWithTag("start_mode_parent").performScrollTo().performClick()
        assertEquals(Role.PARENT, c.appFlow.value.defaultRole)
        onNodeWithTag("start_mode_ask").performScrollTo().performClick()
        assertEquals(null, c.appFlow.value.defaultRole)
    }
}
