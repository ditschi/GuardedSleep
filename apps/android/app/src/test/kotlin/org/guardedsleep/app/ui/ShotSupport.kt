package org.guardedsleep.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.runComposeUiTest
import androidx.compose.ui.test.onRoot
import com.github.takahirom.roborazzi.captureRoboImage
import org.guardedsleep.app.ui.common.BatteryReading
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.common.UiEnv
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.Theme
import org.guardedsleep.app.ui.screens.LocalScannerView
import org.guardedsleep.app.ui.theme.GuardedSleepTheme
import org.robolectric.RuntimeEnvironment
import java.util.TimeZone

/** Fixed clocks so every screenshot is reproducible: 2026-10-20 02:14:00 UTC, elapsed 1_000_000 ms. */
const val ELAPSED = 1_000_000L
const val WALL = 1_792_462_440_000L

val TestEnv = UiEnv(
    elapsedNow = { ELAPSED },
    wallNow = { WALL },
    ticking = false,
    reduceMotion = true,
    zone = TimeZone.getTimeZone("UTC"),
    cameraGranted = true,
    battery = BatteryReading(82, true),
)

private val placeholderScanner: @Composable (onCode: (String) -> Unit, modifier: Modifier) -> Unit =
    { _, m -> Box(m.background(Color(0xFF1E2A38))) }

/**
 * Sets [content] with the test environment and captures it to src/test/screenshots/<name>.png.
 * [locale] is a qualifier fragment such as "de" or "en-rXA" (pseudo-locale).
 */
@OptIn(ExperimentalTestApi::class)
fun shot(
    name: String,
    locale: String = "en",
    role: Role? = Role.PARENT,
    theme: Theme = Theme.DARK,
    fontScale: Float = 1f,
    content: @Composable () -> Unit,
) {
    TimeZone.setDefault(TimeZone.getTimeZone("UTC"))
    RuntimeEnvironment.setQualifiers("+$locale")
    RuntimeEnvironment.setFontScale(fontScale)
    runComposeUiTest {
        setContent {
            CompositionLocalProvider(LocalUiEnv provides TestEnv, LocalScannerView provides placeholderScanner) {
                GuardedSleepTheme(theme, forceDark = role == Role.BABY, content = content)
            }
        }
        val suffix = if (locale == "en") "" else "_$locale"
        onRoot().captureRoboImage("src/test/screenshots/$name$suffix.png")
    }
}
