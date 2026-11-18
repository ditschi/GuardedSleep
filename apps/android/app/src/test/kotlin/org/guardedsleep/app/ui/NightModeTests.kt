@file:OptIn(ExperimentalTestApi::class)

package org.guardedsleep.app.ui

import androidx.activity.ComponentActivity
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runAndroidComposeUiTest
import androidx.compose.ui.test.runComposeUiTest
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.common.StatusBar
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.NightMode
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.SettingsState
import org.guardedsleep.app.ui.model.Theme
import org.guardedsleep.app.ui.screens.HomeScreen
import org.guardedsleep.app.ui.screens.SettingsPage
import org.guardedsleep.app.ui.screens.SettingsScreen
import org.guardedsleep.app.ui.theme.AppScheme
import org.guardedsleep.app.ui.theme.DuskScheme
import org.guardedsleep.app.ui.theme.EmberScheme
import org.guardedsleep.app.ui.theme.GuardedSleepTheme
import org.guardedsleep.app.ui.theme.SafetyColors
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * Night mode (`ui-appearance` "Night mode", "Night mode never hides safety signals"): the Display
 * page, the scheme the root picks, and that the status bar keeps its safety colours in Ember.
 */
class NightModeTests : ShotBase() {
    private val parentApp =
        AppState(
            role = Role.PARENT,
            onboardingDone = true,
            paired = true,
            peerName = "Nursery",
        )

    private fun ComposeUiTest.showEmber(content: @Composable () -> Unit) = setContent {
        CompositionLocalProvider(LocalUiEnv provides TestEnv) {
            GuardedSleepTheme(Theme.DARK, scheme = AppScheme.EMBER, content = content)
        }
    }

    /** The bar's background: the colour most pixels of the bar have (text and icons are few). */
    private fun ComposeUiTest.barColor(): Color {
        val image = onNodeWithTag("status_bar").captureToImage().toPixelMap()
        val counts = HashMap<Int, Int>()
        for (x in 0 until image.width) {
            for (y in 0 until image.height) counts.merge(image[x, y].toArgb(), 1, Int::plus)
        }
        return Color(counts.maxByOrNull { it.value }!!.key)
    }

    private fun assertSameColor(expected: Color, actual: Color) =
        assertEquals(expected.toArgb().toUInt().toString(16), actual.toArgb().toUInt().toString(16))

    private fun bar(phase: LinkPhase, level: Int, agoMs: Long) =
        link(phase, level, agoMs).let {
            if (phase == LinkPhase.CONNECTING) it.copy(lastAliveElapsedMs = null) else it
        }

    // ---- safety colours stay -------------------------------------------------------------------

    @Test
    fun theBarAtLevelOneKeepsTheSafetyOrangeInNightMode() = runComposeUiTest {
        showEmber { StatusBar(bar(LinkPhase.RECONNECTING, 1, 25_000), "Nursery") }
        val shown = barColor()
        assertSameColor(SafetyColors.warnContainer, shown)
        assertNotEquals(EmberScheme.surfaceContainer.toArgb(), shown.toArgb())
    }

    @Test
    fun everyColouredStageAndTheGreyOneUseTheFixedSafetyColoursInNightMode() {
        val stages =
            listOf(
                bar(LinkPhase.RECONNECTING, 0, 10_000) to SafetyColors.infoContainer,
                bar(LinkPhase.RECONNECTING, 1, 25_000) to SafetyColors.warnContainer,
                bar(LinkPhase.RECONNECTING, 2, 72_000) to SafetyColors.alertContainer,
                bar(LinkPhase.RECONNECTING, 3, 200_000) to SafetyColors.alertContainer,
                link(LinkPhase.PEER_STOPPED, 0, 3_000) to SafetyColors.greyContainer,
            )
        for ((l, expected) in stages) {
            runComposeUiTest {
                showEmber { StatusBar(l, "Nursery") }
                assertSameColor(expected, barColor())
            }
        }
    }

    @Test
    fun theCalmBarAtLevelZeroFollowsTheNightScheme() = runComposeUiTest {
        showEmber { StatusBar(Connected, "Nursery") }
        assertSameColor(EmberScheme.surfaceContainer, barColor())
    }

    // ---- the root picks the scheme
    // ---------------------------------------------------------------

    private fun rootBarColor(settings: SettingsState): Color {
        var color = Color.Unspecified
        runAndroidComposeUiTest<ComponentActivity> {
            val c =
                FakeAppController(
                    app = parentApp.copy(monitoring = true),
                    settings = settings,
                    parent = ParentMonitorState(link = Connected),
                )
            setContent {
                CompositionLocalProvider(LocalUiEnv provides TestEnv) { GuardedSleepRoot(c) }
            }
            waitForIdle()
            color = barColor()
        }
        return color
    }

    @Test
    fun nightModeOnMakesTheRootUseEmberAndOffKeepsDusk() {
        assertSameColor(
            EmberScheme.surfaceContainer,
            rootBarColor(SettingsState(nightMode = NightMode.ON)),
        )
        assertSameColor(
            DuskScheme.surfaceContainer,
            rootBarColor(SettingsState(nightMode = NightMode.OFF)),
        )
    }

    @Test
    fun automaticFollowsTheWindowAtTheTestClock() {
        // The fake's clock is 02:14 UTC: inside the default window 21:00 to 07:00.
        assertSameColor(
            EmberScheme.surfaceContainer,
            rootBarColor(SettingsState(nightMode = NightMode.AUTO)),
        )
        assertSameColor(
            DuskScheme.surfaceContainer,
            rootBarColor(
                SettingsState(
                    nightMode = NightMode.AUTO,
                    nightFromMinute = 20 * 60,
                    nightToMinute = 22 * 60,
                )
            ),
        )
    }

    // ---- Settings › Display ---------------------------------------------------------------------

    private fun ComposeUiTest.showDisplay(c: FakeAppController) = setContent {
        CompositionLocalProvider(LocalUiEnv provides TestEnv) {
            GuardedSleepTheme(Theme.DARK) {
                SettingsScreen(c, {}, {}, {}, initialPage = SettingsPage.DISPLAY)
            }
        }
    }

    @Test
    fun theDisplayPageOffersOffOnAndAutomaticAndStoresTheChoice() = runComposeUiTest {
        val c = FakeAppController(app = parentApp)
        showDisplay(c)
        onNodeWithTag("settings_night_off").performScrollTo().assertIsSelected()
        onNodeWithTag("settings_night_window").assertDoesNotExist()
        onNodeWithTag("settings_night_on").performScrollTo().performClick()
        assertEquals(NightMode.ON, c.settingsFlow.value.nightMode)
        onNodeWithTag("settings_night_on").assertIsSelected()
        onNodeWithTag("settings_night_auto").performScrollTo().performClick()
        assertEquals(NightMode.AUTO, c.settingsFlow.value.nightMode)
        onNodeWithTag("settings_night_window").performScrollTo().assertIsDisplayed()
        onNodeWithTag("settings_night_from").assertIsDisplayed()
        onNodeWithTag("settings_night_to").assertIsDisplayed()
        onNodeWithTag("settings_night_note").performScrollTo().assertIsDisplayed()
    }

    @Test
    fun theWindowRowsAreUnderAutomaticOnlyAndTheStatusNamesTheNextChange() = runComposeUiTest {
        val c =
            FakeAppController(app = parentApp, settings = SettingsState(nightMode = NightMode.AUTO))
        showDisplay(c)
        // 02:14 UTC, window 21:00 to 07:00: on now, off at 07:00.
        onNodeWithTag("settings_night_status")
            .performScrollTo()
            .assertTextContains("Now on", substring = true)
        onNodeWithTag("settings_night_status").assertTextContains("turns off at", substring = true)
        onNodeWithTag("settings_night_off").performScrollTo().performClick()
        onNodeWithTag("settings_night_window").assertDoesNotExist()
        onNodeWithTag("settings_night_status").assertTextContains("Now off", substring = true)
    }

    @Test
    fun aTimeChipOpensThePickerAndCancelLeavesTheWindowAlone() = runComposeUiTest {
        val c =
            FakeAppController(app = parentApp, settings = SettingsState(nightMode = NightMode.AUTO))
        showDisplay(c)
        onNodeWithTag("settings_night_from").performScrollTo().performClick()
        onNodeWithTag("settings_night_from_picker").assertIsDisplayed()
        onNodeWithTag("settings_night_from_cancel").performClick()
        onNodeWithTag("settings_night_from_picker").assertDoesNotExist()
        assertEquals(21 * 60, c.settingsFlow.value.nightFromMinute)
        assertNull(c.settingsFlow.value.nightOverride)
    }

    // ---- shots (recorded in the pinned image only) ----------------------------------------------

    @Test
    fun emberShots() {
        shot("home/parent_saved_ember", scheme = AppScheme.EMBER) {
            HomeScreen(
                FakeAppController(app = parentApp.copy(lastUsedWallMs = WALL - 8 * 3_600_000L)),
                {},
            )
        }
        shot("settings/display_ember", scheme = AppScheme.EMBER) {
            SettingsScreen(
                FakeAppController(
                    app = parentApp,
                    settings = SettingsState(nightMode = NightMode.AUTO),
                ),
                {},
                {},
                {},
                initialPage = SettingsPage.DISPLAY,
            )
        }
        shot("status_bar/cant_reach_ember", scheme = AppScheme.EMBER) {
            StatusBar(bar(LinkPhase.RECONNECTING, 1, 25_000), "Nursery")
        }
        shot("status_bar/stopped_ember", scheme = AppScheme.EMBER) {
            StatusBar(link(LinkPhase.PEER_STOPPED, 0, 3_000), "Nursery")
        }
    }
}
