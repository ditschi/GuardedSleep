@file:OptIn(ExperimentalTestApi::class)

package org.guardedsleep.app.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runComposeUiTest
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.common.RowState
import org.guardedsleep.app.ui.model.DeviceCheck
import org.guardedsleep.app.ui.model.DeviceCheckReason
import org.guardedsleep.app.ui.model.MailboxReach
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.Theme
import org.guardedsleep.app.ui.screens.SetupChecks
import org.guardedsleep.app.ui.screens.SetupContent
import org.guardedsleep.app.ui.screens.SetupRow
import org.guardedsleep.app.ui.theme.GuardedSleepTheme
import org.junit.Test
import org.robolectric.RuntimeEnvironment

/** `device-setup` "Baby device check": one plain line, the host names only in the details. */
class DeviceCheckCopyTest : ShotBase() {
    private fun str(id: Int, vararg args: Any) =
        RuntimeEnvironment.getApplication().getString(id, *args)

    private fun ComposeUiTest.showCheck(mailboxes: List<MailboxReach>) = setContent {
        CompositionLocalProvider(LocalUiEnv provides TestEnv) {
            GuardedSleepTheme(Theme.DARK) {
                Content(mailboxes)
            }
        }
    }

    @Composable
    private fun Content(mailboxes: List<MailboxReach>) {
        val states = SetupRow.values().associateWith { RowState.DONE }
        SetupContent(
            Role.BABY,
            SetupChecks(states),
            DeviceCheck.LIMITED,
            {},
            {},
            deviceCheckReasons = listOf(DeviceCheckReason.SOME_MAILBOXES_UNREACHABLE),
            deviceCheckMailboxes = mailboxes,
        )
    }

    @Test
    fun oneReachableAndOneUnreachableMailboxShowTheHostsOnlyInTheDetails() = runComposeUiTest {
        showCheck(listOf(MailboxReach("gs.mydata.rocks", true), MailboxReach("own.example", false)))
        onNodeWithTag("setup_device_check_connection").assertIsDisplayed()
        onNodeWithText(str(R.string.native_ui_setup_device_check_connection_works_some, 1, 2))
            .assertIsDisplayed()
        onNodeWithText("gs.mydata.rocks", substring = true).assertDoesNotExist()
        onNodeWithText("own.example", substring = true).assertDoesNotExist()
        onNodeWithTag("setup_device_check_details").performClick()
        onNodeWithText(
                str(R.string.native_ui_setup_device_check_mailbox_reachable, "gs.mydata.rocks")
            )
            .assertIsDisplayed()
        onNodeWithText(
                str(R.string.native_ui_setup_device_check_mailbox_unreachable, "own.example")
            )
            .assertIsDisplayed()
    }

    @Test
    fun worksWhenEveryMailboxIsReachable() = runComposeUiTest {
        showCheck(listOf(MailboxReach("gs.mydata.rocks", true)))
        onNodeWithText(str(R.string.native_ui_setup_device_check_connection_works))
            .assertIsDisplayed()
    }

    @Test
    fun noReachableMailboxSaysItDoesNotWork() = runComposeUiTest {
        showCheck(listOf(MailboxReach("gs.mydata.rocks", false)))
        onNodeWithText(str(R.string.native_ui_setup_device_check_connection_fails))
            .assertIsDisplayed()
        onNodeWithText("gs.mydata.rocks", substring = true).assertDoesNotExist()
    }

    @Test
    fun theBabyChecklistShowsTheBatterySafetyAdvice() = runComposeUiTest {
        showCheck(emptyList())
        onNodeWithTag("setup_row_safety").performScrollTo().assertIsDisplayed()
    }
}
