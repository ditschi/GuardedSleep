@file:OptIn(ExperimentalTestApi::class)

package org.guardedsleep.app.ui.theme

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.runComposeUiTest
import org.guardedsleep.app.ui.common.Tone
import org.guardedsleep.app.ui.common.container
import org.guardedsleep.app.ui.common.onContainer
import org.guardedsleep.app.ui.model.Theme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Token rules of `ui-appearance`: fixed safety colours and the contrast floors per scheme. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class ThemeTokensTest {
    private fun contrast(a: Color, b: Color): Double {
        val la = a.luminance().toDouble()
        val lb = b.luminance().toDouble()
        return (maxOf(la, lb) + 0.05) / (minOf(la, lb) + 0.05)
    }

    private fun ColorScheme.surfaces() = listOf(background, surface, surfaceContainer)

    @Test
    fun everySchemeProvidesTheSameSafetyColors() {
        val primaries = mutableSetOf<Color>()
        for (scheme in AppScheme.entries) {
            var provided: SafetyColors? = null
            var infoFill: Color? = null
            var infoContent: Color? = null
            var primary: Color? = null
            runComposeUiTest {
                setContent {
                    GuardedSleepTheme(Theme.DARK, scheme = scheme) {
                        provided = LocalSafetyColors.current
                        // Components read the provided value, not the object.
                        infoFill = Tone.INFO.container()
                        infoContent = Tone.INFO.onContainer()
                        primary = MaterialTheme.colorScheme.primary
                    }
                }
                waitForIdle()
            }
            assertSame("$scheme provides SafetyColors", SafetyColors, provided)
            assertEquals("$scheme info fill", SafetyColors.info, infoFill)
            assertEquals("$scheme info content", SafetyColors.onInfo, infoContent)
            primaries += primary!!
        }
        // The scheme did change what the composition provides; only the safety colours did not.
        assertEquals(AppScheme.entries.size, primaries.size)
    }

    @Test
    fun bodyTextReachesFourPointFiveInEveryScheme() {
        for (scheme in AppScheme.entries) {
            val cs = scheme.colorScheme()
            for (surface in cs.surfaces()) {
                assertTrue("$scheme onSurface", contrast(cs.onSurface, surface) >= 4.5)
                assertTrue(
                    "$scheme onSurfaceVariant",
                    contrast(cs.onSurfaceVariant, surface) >= 4.5,
                )
            }
            assertTrue("$scheme primary", contrast(cs.primary, cs.surfaceContainer) >= 4.5)
            assertTrue("$scheme onPrimary", contrast(cs.onPrimary, cs.primary) >= 4.5)
            assertTrue(
                "$scheme onPrimaryContainer",
                contrast(cs.onPrimaryContainer, cs.primaryContainer) >= 4.5,
            )
        }
    }

    @Test
    fun monitorTextReachesSevenInEveryScheme() {
        for (scheme in AppScheme.entries) {
            val cs = scheme.colorScheme()
            for (surface in cs.surfaces()) {
                assertTrue("$scheme onSurface", contrast(cs.onSurface, surface) >= 7.0)
            }
        }
    }

    @Test
    fun alertPalettesKeepTheirSlabTextLegible() {
        val palettes =
            listOf(
                SafetyColors.noise,
                SafetyColors.noConnection,
                SafetyColors.loudAlarm,
                SafetyColors.batteryCritical,
            )
        for (p in palettes) {
            assertTrue(contrast(p.onSlab, p.slab) >= 7.0)
            assertTrue(contrast(p.onBackground, p.background) >= 7.0)
        }
    }

    @Test
    fun solidInfoFillCarriesReadableContent() {
        assertTrue(contrast(SafetyColors.onInfo, SafetyColors.info) >= 7.0)
    }
}
