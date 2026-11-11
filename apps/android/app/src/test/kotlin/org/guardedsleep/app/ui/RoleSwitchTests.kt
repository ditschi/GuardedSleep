@file:OptIn(ExperimentalTestApi::class)

package org.guardedsleep.app.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runComposeUiTest
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.Theme
import org.guardedsleep.app.ui.screens.HomeScreen
import org.guardedsleep.app.ui.screens.RoleScreen
import org.guardedsleep.app.ui.screens.SettingsScreen
import org.guardedsleep.app.ui.theme.GuardedSleepTheme
import org.junit.Assert.assertEquals
import org.junit.Test

/** Session role, start mode and "Switch mode" as the UI drives the controller. */
class RoleSwitchTests : ShotBase() {
    private fun ComposeUiTest.show(content: @Composable () -> Unit) = setContent {
        CompositionLocalProvider(LocalUiEnv provides TestEnv) {
            GuardedSleepTheme(Theme.DARK, content = content)
        }
    }

    private val idle = AppState(role = Role.PARENT, onboardingDone = true, paired = true)

    @Test
    fun choosingARoleWithoutTheToggleDoesNotSetTheDefault() = runComposeUiTest {
        val c = FakeAppController(app = AppState())
        show { RoleScreen(c) }
        onNodeWithTag("role_baby").performClick()
        assertEquals(listOf("chooseRole:BABY:false"), c.calls)
    }

    @Test
    fun theDefaultToggleSetsTheStartModeWithTheChoice() = runComposeUiTest {
        val c = FakeAppController(app = AppState())
        show { RoleScreen(c) }
        onNodeWithTag("role_default_toggle").performScrollTo().performClick()
        onNodeWithTag("role_parent").performClick()
        assertEquals(listOf("chooseRole:PARENT:true"), c.calls)
        assertEquals(Role.PARENT, c.appFlow.value.defaultRole)
    }

    @Test
    fun switchModeOnHomeShowsTheRoleScreenStateAndKeepsTheDefault() = runComposeUiTest {
        val c = FakeAppController(app = idle.copy(defaultRole = Role.PARENT))
        show { HomeScreen(c, {}) }
        onNodeWithTag("switch_mode").performScrollTo().assertIsDisplayed().performClick()
        assertEquals(listOf("switchMode"), c.calls)
        assertEquals(null, c.appFlow.value.role)
        assertEquals(Role.PARENT, c.appFlow.value.defaultRole)
    }

    @Test
    fun startModeRadioGroupSetsTheDefault() = runComposeUiTest {
        val c = FakeAppController(app = idle)
        show { SettingsScreen(c, {}, {}, {}) }
        onNodeWithTag("start_mode_ask").performScrollTo().assertIsDisplayed()
        onNodeWithTag("start_mode_baby").performScrollTo().performClick()
        assertEquals(Role.BABY, c.appFlow.value.defaultRole)
        onNodeWithTag("start_mode_parent").performScrollTo().performClick()
        assertEquals(Role.PARENT, c.appFlow.value.defaultRole)
        onNodeWithTag("start_mode_ask").performScrollTo().performClick()
        assertEquals(null, c.appFlow.value.defaultRole)
    }
}
