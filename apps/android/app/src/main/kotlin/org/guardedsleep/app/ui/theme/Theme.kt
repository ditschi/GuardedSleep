package org.guardedsleep.app.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import org.guardedsleep.app.R
import org.guardedsleep.app.ui.model.Theme

/**
 * Colours that carry meaning (state), identical in every scheme so a colour always means the same.
 * Group 2 of `redesign-calm-ui` replaces the remaining fills with [SafetyColors].
 */
object StateColors {
    val Green = Color(0xFF1B5E3A)
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
    /** Level 0 (connecting, nothing wrong yet): a dim take on [SafetyColors.info]. */
    val KioskInfo = Color(0xFF1A3350)
    val KioskOrange = Color(0xFF4F2400)
    val KioskGrey = Color(0xFF30333A)
    val KioskText = Color(0xFF8C8C8C)
}

/** One full-screen alert palette: dimmed background, a saturated slab and its light border. */
data class AlertPalette(
    val background: Color,
    val surface: Color,
    val onBackground: Color,
    val onVariant: Color,
    val accent: Color,
    val slab: Color,
    val onSlab: Color,
    val slabBorder: Color,
)

/**
 * Safety roles and alert palettes (design section 10). They live outside every [ColorScheme]: Dusk,
 * Day and Ember (night mode) all provide this same object, so "orange" and "red" never change
 * meaning or brightness with the scheme. Values are the Dusk tokens of `mockup/gen_css.py`.
 */
object SafetyColors {
    val info = Color(0xFFA9C6EA)
    val infoContainer = Color(0xFF1E2A38)
    /** Text and glyphs drawn on a solid [info] fill. */
    val onInfo = Color(0xFF0F2540)
    val warn = Color(0xFFF0A774)
    val warnContainer = Color(0xFF3E2719)
    val alert = Color(0xFFFF948A)
    val alertContainer = Color(0xFF4A1B1C)
    val caution = Color(0xFFE3C978)
    val cautionContainer = Color(0xFF363019)
    /** Used as a dot or glyph only while everything is calm. */
    val ok = Color(0xFF86CFA6)
    val okContainer = Color(0xFF1B2F26)

    val noise =
        AlertPalette(
            background = Color(0xFF250A10),
            surface = Color(0xFF35111A),
            onBackground = Color(0xFFFFF0EE),
            onVariant = Color(0xFFF2C6C0),
            accent = Color(0xFFFF948A),
            slab = Color(0xFF7A1E30),
            onSlab = Color(0xFFFFEAE7),
            slabBorder = Color(0xFFFF948A),
        )
    val noConnection =
        AlertPalette(
            background = Color(0xFF2A0709),
            surface = Color(0xFF3B0B0E),
            onBackground = Color(0xFFFFF0EE),
            onVariant = Color(0xFFF7C6C0),
            accent = Color(0xFFFF948A),
            slab = Color(0xFF8C1D24),
            onSlab = Color(0xFFFFE9E6),
            slabBorder = Color(0xFFFF948A),
        )
    val loudAlarm =
        AlertPalette(
            background = Color(0xFF3A080B),
            surface = Color(0xFF4D0C10),
            onBackground = Color(0xFFFFF3F1),
            onVariant = Color(0xFFFFCFC9),
            accent = Color(0xFFFFA198),
            slab = Color(0xFF951E27),
            onSlab = Color(0xFFFFF1EF),
            slabBorder = Color(0xFFFFB4AB),
        )
    val batteryCritical =
        AlertPalette(
            background = Color(0xFF1F1209),
            surface = Color(0xFF2E1C0F),
            onBackground = Color(0xFFFFF3EA),
            onVariant = Color(0xFFF5D2BB),
            accent = Color(0xFFF0A774),
            slab = Color(0xFF7A3D12),
            onSlab = Color(0xFFFFE9D8),
            slabBorder = Color(0xFFF0A774),
        )
}

/** Not a `ColorScheme` slot on purpose: no scheme may override it. */
val LocalSafetyColors = staticCompositionLocalOf { SafetyColors }

/** The three colour schemes (design section 10). Night mode (Ember) gets its switch in group 6. */
enum class AppScheme {
    DUSK,
    DAY,
    EMBER,
}

/** Text-level helper colours that follow the scheme. */
data class AppColors(
    val ok: Color,
    val warn: Color,
    val bad: Color,
    val neutralChip: Color,
    val onNeutralChip: Color,
    val secondaryText: Color,
)

val LocalAppColors = staticCompositionLocalOf { DuskApp }

internal val DuskScheme =
    darkColorScheme(
        primary = Color(0xFFC9BCEB),
        onPrimary = Color(0xFF2A2148),
        primaryContainer = Color(0xFF3A3354),
        onPrimaryContainer = Color(0xFFE7E0FA),
        secondary = Color(0xFFBEB6C4),
        onSecondary = Color(0xFF141218),
        secondaryContainer = Color(0xFF3B3446),
        onSecondaryContainer = Color(0xFFE4DCEF),
        tertiary = Color(0xFF6FD8C8),
        onTertiary = Color(0xFF00201B),
        tertiaryContainer = Color(0xFF0A3A33),
        onTertiaryContainer = Color(0xFFFFFFFF),
        background = Color(0xFF141218),
        onBackground = Color(0xFFECE6EF),
        surface = Color(0xFF141218),
        onSurface = Color(0xFFECE6EF),
        surfaceVariant = Color(0xFF29262F),
        onSurfaceVariant = Color(0xFFBEB6C4),
        surfaceContainerLow = Color(0xFF1A1820),
        surfaceContainer = Color(0xFF211E27),
        surfaceContainerHigh = Color(0xFF29262F),
        surfaceContainerHighest = Color(0xFF322E39),
        error = Color(0xFFFF948A),
        onError = Color(0xFF3B0000),
        errorContainer = Color(0xFF4A1B1C),
        onErrorContainer = Color(0xFFFFDAD6),
        outline = Color(0xFF8B8393),
        outlineVariant = Color(0xFF3B3642),
        scrim = Color(0xFF08060C),
    )

internal val DayScheme =
    lightColorScheme(
        primary = Color(0xFF5A4893),
        onPrimary = Color(0xFFFFFFFF),
        primaryContainer = Color(0xFFE6DEF8),
        onPrimaryContainer = Color(0xFF21184A),
        secondary = Color(0xFF4F4756),
        onSecondary = Color(0xFFFFFFFF),
        secondaryContainer = Color(0xFFECE6F3),
        onSecondaryContainer = Color(0xFF2E2540),
        tertiary = Color(0xFF00695C),
        onTertiary = Color(0xFFFFFFFF),
        tertiaryContainer = Color(0xFFBDECE3),
        onTertiaryContainer = Color(0xFF00201B),
        background = Color(0xFFFAF7F4),
        onBackground = Color(0xFF1F1B23),
        surface = Color(0xFFFAF7F4),
        onSurface = Color(0xFF1F1B23),
        surfaceVariant = Color(0xFFEFE9EC),
        onSurfaceVariant = Color(0xFF4F4756),
        surfaceContainerLow = Color(0xFFF3EEEE),
        surfaceContainer = Color(0xFFFFFFFF),
        surfaceContainerHigh = Color(0xFFEFE9EC),
        surfaceContainerHighest = Color(0xFFE5DEE4),
        error = Color(0xFFB3261E),
        onError = Color(0xFFFFFFFF),
        errorContainer = Color(0xFFFBDDD9),
        onErrorContainer = Color(0xFF410002),
        outline = Color(0xFF7F7588),
        outlineVariant = Color(0xFFDDD3DD),
        scrim = Color(0xFF1F1B23),
    )

/** Night mode: red-shifted on black; the dimness comes from the backlight, not from contrast. */
internal val EmberScheme =
    darkColorScheme(
        primary = Color(0xFFD08A6C),
        onPrimary = Color(0xFF1A0703),
        primaryContainer = Color(0xFF2E140D),
        onPrimaryContainer = Color(0xFFE8B19A),
        secondary = Color(0xFFA87365),
        onSecondary = Color(0xFF000000),
        secondaryContainer = Color(0xFF22100C),
        onSecondaryContainer = Color(0xFFDDA08E),
        tertiary = Color(0xFFD08A6C),
        onTertiary = Color(0xFF1A0703),
        tertiaryContainer = Color(0xFF2E140D),
        onTertiaryContainer = Color(0xFFE8B19A),
        background = Color(0xFF000000),
        onBackground = Color(0xFFC98A7A),
        surface = Color(0xFF000000),
        onSurface = Color(0xFFC98A7A),
        surfaceVariant = Color(0xFF180B0A),
        onSurfaceVariant = Color(0xFFA87365),
        surfaceContainerLow = Color(0xFF0A0505),
        surfaceContainer = Color(0xFF110807),
        surfaceContainerHigh = Color(0xFF180B0A),
        surfaceContainerHighest = Color(0xFF200F0D),
        error = Color(0xFFE06A5E),
        onError = Color(0xFF1A0703),
        errorContainer = Color(0xFF230807),
        onErrorContainer = Color(0xFFE8B19A),
        outline = Color(0xFF9A5E50),
        outlineVariant = Color(0xFF2E1714),
        scrim = Color(0xFF000000),
    )

private val DuskApp =
    AppColors(
        ok = Color(0xFF86CFA6),
        warn = Color(0xFFF0A774),
        bad = Color(0xFFFF948A),
        neutralChip = Color(0xFF29262F),
        onNeutralChip = Color(0xFFECE6EF),
        secondaryText = Color(0xFFBEB6C4),
    )
private val DayApp =
    AppColors(
        ok = Color(0xFF1C6B45),
        warn = Color(0xFF9A4510),
        bad = Color(0xFFB3261E),
        neutralChip = Color(0xFFEFE9EC),
        onNeutralChip = Color(0xFF1F1B23),
        secondaryText = Color(0xFF4F4756),
    )
private val EmberApp =
    AppColors(
        ok = Color(0xFF7AA883),
        warn = Color(0xFFD07A44),
        bad = Color(0xFFE06A5E),
        neutralChip = Color(0xFF180B0A),
        onNeutralChip = Color(0xFFC98A7A),
        secondaryText = Color(0xFFA87365),
    )

internal fun AppScheme.colorScheme(): ColorScheme =
    when (this) {
        AppScheme.DUSK -> DuskScheme
        AppScheme.DAY -> DayScheme
        AppScheme.EMBER -> EmberScheme
    }

private fun AppScheme.appColors(): AppColors =
    when (this) {
        AppScheme.DUSK -> DuskApp
        AppScheme.DAY -> DayApp
        AppScheme.EMBER -> EmberApp
    }

/**
 * The bundled typeface (SIL OFL 1.1, subset to Latin; `apps/web/src/assets/fonts/README.md` holds
 * the provenance). Scripts it lacks (Cyrillic, Arabic, Devanagari, CJK) fall back to the system
 * font.
 */
val PlusJakartaSans =
    FontFamily(
        Font(R.font.plus_jakarta_sans_regular, FontWeight.Normal),
        Font(R.font.plus_jakarta_sans_medium, FontWeight.Medium),
        Font(R.font.plus_jakarta_sans_semibold, FontWeight.SemiBold),
        Font(R.font.plus_jakarta_sans_bold, FontWeight.Bold),
    )

/**
 * Type scale of design section 10: 12 (caps labels) / 14 (floor for text) / 15 / 16 / 20 / 28 / 52.
 * Tabular numerals everywhere so counters and times do not jump.
 */
internal fun appTypography(family: FontFamily = PlusJakartaSans): Typography {
    fun style(size: Int, line: Int, weight: FontWeight, caps: Boolean = false) =
        TextStyle(
            fontFamily = family,
            fontSize = size.sp,
            lineHeight = line.sp,
            fontWeight = weight,
            letterSpacing = if (caps) 1.1.sp else 0.sp,
            fontFeatureSettings = "tnum",
        )
    return Typography(
        displayMedium = style(52, 52, FontWeight.SemiBold),
        headlineMedium = style(28, 34, FontWeight.Bold),
        headlineSmall = style(20, 26, FontWeight.Bold),
        titleLarge = style(20, 26, FontWeight.Bold),
        titleMedium = style(16, 22, FontWeight.SemiBold),
        titleSmall = style(14, 20, FontWeight.SemiBold),
        bodyLarge = style(15, 22, FontWeight.Normal),
        bodyMedium = style(14, 20, FontWeight.Normal),
        bodySmall = style(14, 20, FontWeight.Normal),
        labelLarge = style(14, 20, FontWeight.SemiBold),
        labelMedium = style(12, 16, FontWeight.Bold, caps = true),
        labelSmall = style(12, 16, FontWeight.Bold, caps = true),
    )
}

/**
 * Entry point of the theme. [scheme] is explicit for tests and for night mode (group 6); without it
 * the user's theme setting picks Dusk (dark, the default) or Day (light).
 */
@Composable
fun GuardedSleepTheme(
    theme: Theme,
    forceDark: Boolean = false,
    scheme: AppScheme? = null,
    content: @Composable () -> Unit,
) {
    val dark =
        forceDark ||
            when (theme) {
                Theme.DARK -> true
                Theme.LIGHT -> false
                Theme.SYSTEM -> isSystemInDarkTheme()
            }
    val active = scheme ?: if (dark) AppScheme.DUSK else AppScheme.DAY
    CompositionLocalProvider(
        LocalAppColors provides active.appColors(),
        LocalSafetyColors provides SafetyColors,
    ) {
        MaterialTheme(
            colorScheme = active.colorScheme(),
            typography = appTypography(),
            content = content,
        )
    }
}
