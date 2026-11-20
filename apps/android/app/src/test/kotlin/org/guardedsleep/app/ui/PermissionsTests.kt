@file:OptIn(ExperimentalTestApi::class)

package org.guardedsleep.app.ui

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runComposeUiTest
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.common.RowState
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.Theme
import org.guardedsleep.app.ui.screens.PermissionRow
import org.guardedsleep.app.ui.screens.PermissionsContent
import org.guardedsleep.app.ui.theme.GuardedSleepTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** `device-setup` "Permissions on first start" and "No later permission prompts". */
class PermissionsTests : ShotBase() {
    private val allRows = PermissionRow.entries

    @Test
    fun aFreshInstallShowsThePermissionScreenBeforeTheRoleChoice() = runComposeUiTest {
        val c = FakeAppController(app = AppState())
        setContent {
            CompositionLocalProvider(LocalUiEnv provides TestEnv) { GuardedSleepRoot(c) }
        }
        onNodeWithTag("permissions_screen").assertIsDisplayed()
        onNodeWithTag("role_title").assertDoesNotExist()
        onNodeWithTag("permissions_continue").performScrollTo().performClick()
        assertTrue(c.calls.contains("completePermissionsIntro"))
        // The app state then moves on: the role choice, with no checklist in between.
        onNodeWithTag("permissions_screen").assertDoesNotExist()
        onNodeWithTag("role_title").assertIsDisplayed()
    }

    @Test
    fun theScreenListsEveryPermissionWithItsReasonAndAction() = runComposeUiTest {
        setContent {
            CompositionLocalProvider(LocalUiEnv provides TestEnv) {
                GuardedSleepTheme(Theme.DARK) {
                    PermissionsContent(
                        allRows,
                        allRows.associateWith { RowState.TODO },
                        emptySet(),
                        {},
                        {},
                        {},
                        {},
                    )
                }
            }
        }
        onNodeWithTag("permissions_allow_all").performScrollTo().assertIsDisplayed()
        for (row in allRows) {
            onNodeWithTag("permission_row_${row.tag}").performScrollTo().assertIsDisplayed()
            onNodeWithTag("permission_row_${row.tag}_allow").assertExists()
            onNodeWithTag("permission_row_${row.tag}_later").assertExists()
        }
    }

    @Test
    fun laterAndAllowAreReportedPerRowAndAllowedRowsOfferNothing() = runComposeUiTest {
        val allowed = mutableListOf<PermissionRow>()
        val later = mutableListOf<PermissionRow>()
        setContent {
            CompositionLocalProvider(LocalUiEnv provides TestEnv) {
                GuardedSleepTheme(Theme.DARK) {
                    PermissionsContent(
                        allRows,
                        allRows.associateWith {
                            if (it == PermissionRow.CAMERA) RowState.DONE else RowState.TODO
                        },
                        emptySet(),
                        {},
                        { allowed += it },
                        { later += it },
                        {},
                    )
                }
            }
        }
        onNodeWithTag("permission_row_camera_allow").assertDoesNotExist()
        onNodeWithTag("permission_row_microphone_allow").performScrollTo().performClick()
        onNodeWithTag("permission_row_dnd_later").performScrollTo().performClick()
        assertEquals(listOf(PermissionRow.MICROPHONE), allowed)
        assertEquals(listOf(PermissionRow.DND), later)
    }

    @Test
    fun withEverythingAllowedAllowAllIsGoneAndContinueLeads() = runComposeUiTest {
        var continued = false
        setContent {
            CompositionLocalProvider(LocalUiEnv provides TestEnv) {
                GuardedSleepTheme(Theme.DARK) {
                    PermissionsContent(
                        allRows,
                        allRows.associateWith { RowState.DONE },
                        emptySet(),
                        {},
                        {},
                        {},
                        { continued = true },
                    )
                }
            }
        }
        onNodeWithTag("permissions_allow_all").assertDoesNotExist()
        onNodeWithTag("permissions_continue").performScrollTo().performClick()
        assertTrue(continued)
    }
}
