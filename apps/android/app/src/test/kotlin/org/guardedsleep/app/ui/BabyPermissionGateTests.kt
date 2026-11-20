@file:OptIn(ExperimentalTestApi::class)

package org.guardedsleep.app.ui

import android.Manifest
import android.app.Application
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runComposeUiTest
import androidx.test.core.app.ApplicationProvider
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.ConnectionUi
import org.guardedsleep.app.ui.model.PairingPhase
import org.guardedsleep.app.ui.model.PairingState
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.Theme
import org.guardedsleep.app.ui.screens.HomeScreen
import org.guardedsleep.app.ui.screens.PairBabyScreen
import org.guardedsleep.app.ui.theme.GuardedSleepTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.robolectric.Shadows.shadowOf

/** `device-setup` "Baby unit permission gate before connecting". */
class BabyPermissionGateTests : ShotBase() {
    private val baby =
        AppState(
            role = Role.BABY,
            onboardingDone = true,
            permissionsIntroDone = true,
            babyConnections = listOf(ConnectionUi("1", "Cot", isDefault = true, selected = true)),
            microphoneNeeded = true,
        )

    private fun androidx.compose.ui.test.ComposeUiTest.screen(content: @Composable () -> Unit) =
        setContent {
            CompositionLocalProvider(LocalUiEnv provides TestEnv) {
                GuardedSleepTheme(Theme.DARK, forceDark = true, content = content)
            }
        }

    @Test
    fun noMicrophoneShowsTheWarningAndNoCodeAndStartsNothing() = runComposeUiTest {
        val c =
            FakeAppController(
                app = baby,
                pairing = PairingState(PairingPhase.NEEDS_MICROPHONE),
            )
        screen { PairBabyScreen(c) }
        onNodeWithTag("parent_banner_baby_pair_gate_mic").assertIsDisplayed()
        onNodeWithTag("parent_banner_baby_pair_gate_mic_action").assertIsDisplayed()
        onNodeWithTag("baby_pair_code_text").assertDoesNotExist()
        onNodeWithTag("baby_pair_show_text").assertDoesNotExist()
        onNodeWithTag("baby_pair_waiting").assertDoesNotExist()
        assertEquals(emptyList<String>(), c.calls)
    }

    @Test
    fun grantingTheMicrophoneAsksForTheCodeInPlace() = runComposeUiTest {
        shadowOf(ApplicationProvider.getApplicationContext<Application>())
            .grantPermissions(Manifest.permission.RECORD_AUDIO)
        val c =
            FakeAppController(
                app = baby,
                pairing = PairingState(PairingPhase.NEEDS_MICROPHONE),
            )
        screen { PairBabyScreen(c) }
        waitForIdle()
        assertTrue(c.calls.contains("startPairing"))
        onNodeWithTag("parent_banner_baby_pair_gate_mic").assertDoesNotExist()
    }

    @Test
    fun homeHidesTheStartRowsWhileTheMicrophoneIsMissing() = runComposeUiTest {
        val c = FakeAppController(app = baby)
        screen { HomeScreen(c, {}) }
        onNodeWithTag("parent_banner_home_baby_gate_mic").performScrollTo().assertIsDisplayed()
        onNodeWithTag("home_connection_baby_1").assertDoesNotExist()
        assertEquals(emptyList<String>(), c.calls)
    }

    @Test
    fun missingBackgroundOperationWarnsWithoutBlocking() = runComposeUiTest {
        shadowOf(ApplicationProvider.getApplicationContext<Application>())
            .grantPermissions(Manifest.permission.RECORD_AUDIO)
        val c = FakeAppController(app = baby.copy(microphoneNeeded = false))
        screen { HomeScreen(c, {}) }
        onNodeWithTag("parent_banner_home_baby_gate_background")
            .performScrollTo()
            .assertIsDisplayed()
        onNodeWithTag("parent_banner_home_baby_gate_background_action").assertExists()
        // Not blocking: the start row is still there.
        onNodeWithTag("home_connection_baby_1").performScrollTo().assertIsDisplayed()
    }
}
