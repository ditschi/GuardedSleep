package org.guardedsleep.app.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import org.guardedsleep.app.ui.model.Theme

/**
 * Colours that carry meaning (state), identical in both themes so a colour always means the same.
 */
object StateColors {
    val Green = Color(0xFF1B5E3A)
    val Yellow = Color(0xFF7A5C00)
    val Orange = Color(0xFF8A3F00)
    val Red = Color(0xFF8E1B1B)
    val Grey = Color(0xFF3A3F47)
    val Amber = Color(0xFF6B4E00)
    val OnState = Color(0xFFFFFFFF)

    /** Dark-red alert background (never white). */
    val AlertBackground = Color(0xFF2A0A0A)
    val AlertSurface = Color(0xFF3D1212)

    /** Baby side: near-black with dim state indicators. */
    val KioskBlack = Color(0xFF000000)
    val KioskPulsePeak = Color(0xFF330000)
    val KioskGreen = Color(0xFF0E4A28)
    val KioskYellow = Color(0xFF4A3A00)
    val KioskOrange = Color(0xFF4F2400)
    val KioskGrey = Color(0xFF30333A)
    val KioskText = Color(0xFF8C8C8C)
}

data class AppColors(
    val ok: Color,
    val warn: Color,
    val bad: Color,
    val neutralChip: Color,
    val onNeutralChip: Color,
    val secondaryText: Color,
)

val LocalAppColors = staticCompositionLocalOf {
    AppColors(
        ok = Color(0xFF63D18B),
        warn = Color(0xFFFFC94D),
        bad = Color(0xFFFF8A80),
        neutralChip = Color(0xFF262B32),
        onNeutralChip = Color(0xFFE6E8EB),
        secondaryText = Color(0xFFB4BAC2),
    )
}

private val DarkScheme =
    darkColorScheme(
        primary = Color(0xFF8FB8FF),
        onPrimary = Color(0xFF00214D),
        primaryContainer = Color(0xFF1E3A66),
        onPrimaryContainer = Color(0xFFFFFFFF),
        secondary = Color(0xFFB4BAC2),
        onSecondary = Color(0xFF111418),
        background = Color(0xFF0E1013),
        onBackground = Color(0xFFE6E8EB),
        surface = Color(0xFF16191E),
        onSurface = Color(0xFFE6E8EB),
        surfaceVariant = Color(0xFF22262C),
        onSurfaceVariant = Color(0xFFB4BAC2),
        error = Color(0xFFFF8A80),
        onError = Color(0xFF3B0000),
        outline = Color(0xFF7D858F),
    )

private val LightScheme =
    lightColorScheme(
        primary = Color(0xFF1F4FA3),
        onPrimary = Color(0xFFFFFFFF),
        primaryContainer = Color(0xFFD6E3FF),
        onPrimaryContainer = Color(0xFF001B3E),
        secondary = Color(0xFF454C55),
        onSecondary = Color(0xFFFFFFFF),
        background = Color(0xFFF7F7F4),
        onBackground = Color(0xFF14181C),
        surface = Color(0xFFFFFFFF),
        onSurface = Color(0xFF14181C),
        surfaceVariant = Color(0xFFE8EAEE),
        onSurfaceVariant = Color(0xFF454C55),
        error = Color(0xFFB3261E),
        onError = Color(0xFFFFFFFF),
        outline = Color(0xFF6B727B),
    )

private val DarkApp =
    AppColors(
        ok = Color(0xFF63D18B),
        warn = Color(0xFFFFC94D),
        bad = Color(0xFFFF8A80),
        neutralChip = Color(0xFF262B32),
        onNeutralChip = Color(0xFFE6E8EB),
        secondaryText = Color(0xFFB4BAC2),
    )
private val LightApp =
    AppColors(
        ok = Color(0xFF136B3A),
        warn = Color(0xFF7A5200),
        bad = Color(0xFFB3261E),
        neutralChip = Color(0xFFE3E6EB),
        onNeutralChip = Color(0xFF14181C),
        secondaryText = Color(0xFF454C55),
    )

@Composable
fun GuardedSleepTheme(theme: Theme, forceDark: Boolean = false, content: @Composable () -> Unit) {
    val dark =
        forceDark ||
            when (theme) {
                Theme.DARK -> true
                Theme.LIGHT -> false
                Theme.SYSTEM -> isSystemInDarkTheme()
            }
    CompositionLocalProvider(LocalAppColors provides if (dark) DarkApp else LightApp) {
        MaterialTheme(colorScheme = if (dark) DarkScheme else LightScheme, content = content)
    }
}
