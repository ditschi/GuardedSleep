@file:OptIn(ExperimentalTestApi::class)

package org.guardedsleep.app.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertContentDescriptionContains
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.runComposeUiTest
import org.guardedsleep.app.ui.common.AlertStage
import org.guardedsleep.app.ui.common.Glyph
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.common.MissedResult
import org.guardedsleep.app.ui.common.RestoredCard
import org.guardedsleep.app.ui.common.StatusBar
import org.guardedsleep.app.ui.common.StatusKind
import org.guardedsleep.app.ui.common.alertStageOf
import org.guardedsleep.app.ui.common.elapsedToWall
import org.guardedsleep.app.ui.common.formatTime
import org.guardedsleep.app.ui.common.glyph
import org.guardedsleep.app.ui.common.loud
import org.guardedsleep.app.ui.common.missedResultOf
import org.guardedsleep.app.ui.common.statusKindOf
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.CauseHint
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.LinkUi
import org.guardedsleep.app.ui.model.MissedEventsUi
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.ReadinessIssue
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.Theme
import org.guardedsleep.app.ui.screens.SettingsScreen
import org.guardedsleep.app.ui.theme.GuardedSleepTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.robolectric.RuntimeEnvironment

/**
 * The status bar and the connection alert (`connection-status` and `link-supervision` of
 * `redesign-calm-ui`): one scenario per requirement scenario, on the shared clocks of [TestEnv].
 */
class StatusAndAlertTests : ShotBase() {
    private val monitoring =
        AppState(
            role = Role.PARENT,
            onboardingDone = true,
            paired = true,
            monitoring = true,
            peerName = "Nursery",
        )

    private fun ComposeUiTest.show(content: @Composable () -> Unit) = setContent {
        CompositionLocalProvider(LocalUiEnv provides TestEnv) {
            GuardedSleepTheme(Theme.DARK, content = content)
        }
    }

    private fun outage(phase: LinkPhase, level: Int, downMs: Long, cause: CauseHint? = null) =
        link(phase, level, downMs, cause = cause).let {
            // A first connection has no last contact, only the run start.
            if (phase == LinkPhase.CONNECTING) it.copy(lastAliveElapsedMs = null) else it
        }

    private fun alarmController(link: LinkUi, readiness: List<ReadinessIssue> = emptyList()) =
        FakeAppController(
            app = monitoring,
            parent = ParentMonitorState(link = link, readiness = readiness),
        )

    private fun ComposeUiTest.barText(text: String) =
        onNode(hasText(text, substring = true) and hasAnyAncestor(hasTestTag("status_bar")))

    // ---- status bar ------------------------------------------------------------------------

    @Test
    fun theStageTableFollowsTheCoreStateAndLevel() {
        val table =
            listOf(
                outage(LinkPhase.CONNECTING, 0, 5_000) to StatusKind.CONNECTING,
                outage(LinkPhase.RECONNECTING, 0, 5_000) to StatusKind.RECONNECTING,
                outage(LinkPhase.CONNECTING, 1, 25_000) to StatusKind.NOT_CONNECTED_YET,
                outage(LinkPhase.RECONNECTING, 1, 25_000) to StatusKind.CANT_REACH,
                outage(LinkPhase.CONNECTING, 2, 70_000) to StatusKind.COULDNT_CONNECT,
                outage(LinkPhase.RECONNECTING, 2, 70_000) to StatusKind.NO_CONNECTION,
                outage(LinkPhase.RECONNECTING, 3, 200_000) to StatusKind.STILL_NO_CONNECTION,
                link(LinkPhase.PEER_STOPPED) to StatusKind.STOPPED,
                link(LinkPhase.FAILED_NEEDS_USER) to StatusKind.NEEDS_USER,
                Connected to StatusKind.CONNECTED,
            )
        table.forEach { (l, kind) -> assertEquals(kind, statusKindOf(l)) }
        // Red with a duration from level 2, and a symbol per stage.
        val loud =
            setOf(
                StatusKind.COULDNT_CONNECT,
                StatusKind.NO_CONNECTION,
                StatusKind.STILL_NO_CONNECTION,
            )
        StatusKind.entries.forEach { assertEquals(it.toString(), it in loud, it.loud()) }
        assertEquals(Glyph.LINK, StatusKind.CONNECTED.glyph())
        assertEquals(Glyph.REFRESH, StatusKind.CONNECTING.glyph())
        assertEquals(Glyph.REFRESH, StatusKind.RECONNECTING.glyph())
        assertEquals(Glyph.PAUSE, StatusKind.STOPPED.glyph())
        assertEquals(Glyph.PAUSE, StatusKind.IDLE.glyph())
        assertEquals(Glyph.BROKEN_LINK, StatusKind.CANT_REACH.glyph())
        assertEquals(Glyph.BROKEN_LINK, StatusKind.NO_CONNECTION.glyph())
    }

    @Test
    fun normalNightIsNeutralWithTheConnectionName() = runComposeUiTest {
        show { StatusBar(Connected, "Nursery") }
        onNodeWithTag("status_bar").assertIsDisplayed()
        barText("Connected · Nursery").assertIsDisplayed()
    }

    @Test
    fun shortDropIsBlueAndSaysReconnectingWithoutATimer() = runComposeUiTest {
        show { StatusBar(outage(LinkPhase.RECONNECTING, 0, 10_000), "Nursery") }
        barText("Reconnecting · Nursery").assertIsDisplayed()
        onNodeWithTag("status_bar_duration").assertDoesNotExist()
    }

    @Test
    fun firstConnectionNotAnsweredIsOrangeNotBlue() = runComposeUiTest {
        show { StatusBar(outage(LinkPhase.CONNECTING, 1, 25_000), "Nursery") }
        barText("Not connected yet").assertIsDisplayed()
        assertEquals(
            StatusKind.NOT_CONNECTED_YET,
            statusKindOf(outage(LinkPhase.CONNECTING, 1, 25_000)),
        )
    }

    @Test
    fun outagePassingOneMinuteIsRedWithTheDurationOutsideTheLiveRegion() = runComposeUiTest {
        show { StatusBar(outage(LinkPhase.RECONNECTING, 2, 70_000), "Nursery") }
        barText("No connection · Nursery").assertIsDisplayed()
        onNodeWithTag("status_bar_duration").assertTextContains("1 min 10 s")
        // Only the stage words are announced, assertively from level 2; the duration is not.
        onNode(hasText("No connection · Nursery", substring = true))
            .assert(
                hasAnyAncestorWithLive() or
                    SemanticsMatcher.expectValue(
                        SemanticsProperties.LiveRegion,
                        LiveRegionMode.Assertive,
                    )
            )
        onNodeWithTag("status_bar_duration")
            .assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.LiveRegion))
    }

    private fun hasAnyAncestorWithLive() =
        hasAnyAncestor(
            SemanticsMatcher.expectValue(SemanticsProperties.LiveRegion, LiveRegionMode.Assertive)
        )

    @Test
    fun theBarStaysOnSettingsWhileMonitoring() = runComposeUiTest {
        val c = FakeAppController(app = monitoring, parent = ParentMonitorState(link = Connected))
        show { WithStatusBar(c) { SettingsScreen(c, {}, {}, {}) } }
        onNodeWithTag("status_bar").assertIsDisplayed()
        barText("Connected · Nursery").assertIsDisplayed()
    }

    // ---- connection alert ------------------------------------------------------------------

    @Test
    fun oneMinuteWithoutConnectionVibratesAndSaysWhenTheLoudAlarmStarts() = runComposeUiTest {
        val c = alarmController(outage(LinkPhase.RECONNECTING, 2, 72_000, CauseHint.PEER_ABSENT))
        val nextAt = c.parentFlow.value.link.nextLevelAtElapsedMs!!
        var loudAt = ""
        show {
            loudAt = formatTime(elapsedToWall(nextAt, ELAPSED, WALL))
            LinkAlarmScreen(c, {})
        }
        onNodeWithTag("alert_label").assertTextContains("THIS DEVICE IS VIBRATING")
        onNodeWithTag("link_alarm_title").assertTextContains("Please check on the child")
        onNodeWithTag("link_alarm_since").assertTextContains("1:12")
        // The sentence carries the core's next_level_at_ms as a clock time, nothing computed here.
        onNodeWithTag("link_alarm_sentence").assertTextContains(loudAt, substring = true)
        assertTrue(loudAt.isNotBlank())
        onNodeWithTag("link_alarm_ack").assertIsDisplayed()
        onNodeWithTag("alert_hazard").assertDoesNotExist()
    }

    @Test
    fun theCounterIsSpokenInWordsAndIsNotALiveRegion() = runComposeUiTest {
        show { LinkAlarmScreen(alarmController(outage(LinkPhase.RECONNECTING, 2, 72_000)), {}) }
        onNodeWithTag("link_alarm_since")
            .assertContentDescriptionContains("1 minute 12 seconds", substring = true)
            .assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.LiveRegion))
    }

    @Test
    fun loudAlarmNamesTheStageWithoutALevelNumber() = runComposeUiTest {
        show { LinkAlarmScreen(alarmController(outage(LinkPhase.RECONNECTING, 3, 200_000)), {}) }
        onNodeWithTag("alert_label").assertTextContains("LOUD ALARM SOUNDING")
        onNodeWithTag("link_alarm_title").assertTextContains("Still no connection · Nursery")
        onNodeWithTag("link_alarm_sentence")
            .assertTextContains("Go to the child now.", substring = true)
        onNodeWithTag("alert_hazard").assertIsDisplayed()
    }

    @Test
    fun loudAlarmWithoutTheDndGrantWarnsAndOffersTheSetting() = runComposeUiTest {
        show {
            LinkAlarmScreen(
                alarmController(
                    outage(LinkPhase.RECONNECTING, 3, 200_000),
                    listOf(ReadinessIssue.DND_BYPASS_MISSING),
                ),
                {},
            )
        }
        onNodeWithTag("link_alarm_dnd_note")
            .assertTextContains("Do Not Disturb may block this alarm.")
        onNodeWithTag("link_alarm_dnd_open").assertIsDisplayed()
    }

    @Test
    fun loudAlarmWithTheGrantSaysItAlsoSoundsInDnd() = runComposeUiTest {
        show { LinkAlarmScreen(alarmController(outage(LinkPhase.RECONNECTING, 3, 200_000)), {}) }
        onNodeWithTag("link_alarm_dnd_note")
            .assertTextContains("also sounds in Do Not Disturb", substring = true)
        onNodeWithTag("link_alarm_dnd_open").assertDoesNotExist()
    }

    @Test
    fun neverConnectedSaysMonitoringNeverStarted() = runComposeUiTest {
        show { LinkAlarmScreen(alarmController(outage(LinkPhase.CONNECTING, 2, 70_000)), {}) }
        onNodeWithTag("link_alarm_title").assertTextContains("Couldn't connect · Nursery")
        onNodeWithTag("link_alarm_sentence")
            .assertTextContains("Monitoring never started.", substring = true)
        onNodeWithTag("link_alarm_wrong_connection").assertIsDisplayed()
    }

    @Test
    fun acknowledgedStaysRedSaysWhenSoundReturnsAndHasNoFilledAction() = runComposeUiTest {
        val l = link(LinkPhase.RECONNECTING, 2, 95_000, ack = true)
        assertEquals(AlertStage.PAUSED, alertStageOf(l, ELAPSED))
        var backAt = ""
        show {
            backAt = formatTime(elapsedToWall(l.acknowledgedUntilElapsedMs!!, ELAPSED, WALL))
            LinkAlarmScreen(alarmController(l), {})
        }
        onNodeWithTag("alert_label").assertTextContains("ALARM PAUSED")
        onNodeWithTag("link_alarm_title")
            .assertTextContains("Comes back at $backAt", substring = true)
        onNodeWithTag("link_alarm_ack").assertDoesNotExist()
    }

    @Test
    fun theTroubleshooterShowsStepOneAndFoldsTheRest() = runComposeUiTest {
        val c = alarmController(outage(LinkPhase.RECONNECTING, 2, 70_000, CauseHint.PEER_ABSENT))
        show { LinkAlarmScreen(c, {}) }
        onNodeWithTag("link_alarm_cause").assertTextContains("The baby device isn't online.")
        onNodeWithTag("link_alarm_step").assertTextContains("plugged in", substring = true)
        onNodeWithTag("link_alarm_more_0").assertDoesNotExist()
        onNodeWithTag("link_alarm_more_steps").performClick()
        onNodeWithTag("link_alarm_more_0").assertTextContains("Is it on Wi-Fi?", substring = true)
        onNodeWithTag("link_alarm_more_steps").assertTextContains("Fewer steps")
        onNodeWithTag("link_alarm_try_now").performClick()
        assertEquals(listOf("tryReconnectNow"), c.calls)
    }

    @Test
    fun hotelWifiSuggestsMobileData() = runComposeUiTest {
        show {
            LinkAlarmScreen(
                alarmController(outage(LinkPhase.RECONNECTING, 2, 70_000, CauseHint.MEDIA_BLOCKED)),
                {},
            )
        }
        onNodeWithTag("link_alarm_cause")
            .assertTextContains("blocked on this network", substring = true)
        onNodeWithTag("link_alarm_step").assertTextContains("mobile data", substring = true)
    }

    @Test
    fun confirmationOverARunningAlarmSilencesAndAcknowledgesNothing() = runComposeUiTest {
        val c = alarmController(outage(LinkPhase.RECONNECTING, 3, 200_000))
        show { LinkAlarmScreen(c, {}) }
        onNodeWithTag("link_alarm_stop").performClick()
        onNodeWithTag("dialog_stop").assertIsDisplayed()
        // The alarm feedback is the platform's: nothing in the contract was called.
        assertTrue(c.calls.isEmpty())
        onNodeWithTag("dialog_stop_cancel").performClick()
        onNodeWithTag("dialog_stop").assertDoesNotExist()
        onNodeWithTag("link_alarm_title").assertIsDisplayed()
        assertTrue(c.calls.isEmpty())
        onNodeWithTag("link_alarm_stop").performClick()
        onNodeWithTag("dialog_stop_confirm").performClick()
        assertEquals(listOf("stopMonitoring"), c.calls)
    }

    @Test
    fun wrongConnectionOnTheRedScreenAsksFirst() = runComposeUiTest {
        val c = alarmController(outage(LinkPhase.CONNECTING, 2, 70_000))
        show { LinkAlarmScreen(c, {}) }
        onNodeWithTag("link_alarm_wrong_connection").performClick()
        onNodeWithTag("dialog_choose_other").assertIsDisplayed()
        assertFalse(c.calls.contains("chooseAnotherConnection"))
        onNodeWithTag("dialog_choose_other_confirm").performClick()
        assertEquals(listOf("chooseAnotherConnection"), c.calls)
    }

    @Test
    fun thePinnedActionIsFullyVisibleAtFontScale2() = runComposeUiTest {
        RuntimeEnvironment.setFontScale(2.0f)
        show {
            LinkAlarmScreen(
                alarmController(outage(LinkPhase.RECONNECTING, 2, 72_000, CauseHint.PEER_ABSENT)),
                {},
            )
        }
        val root = onRoot().fetchSemanticsNode().boundsInRoot
        val action =
            onNodeWithTag("link_alarm_ack").assertIsDisplayed().fetchSemanticsNode().boundsInRoot
        assertTrue("action inside the screen: $action in $root", action.top >= root.top - 0.5f)
        assertTrue(
            "action inside the screen: $action in $root",
            action.bottom <= root.bottom + 0.5f,
        )
        assertTrue(action.left >= root.left - 0.5f && action.right <= root.right + 0.5f)
    }

    // ---- restored card ---------------------------------------------------------------------

    private fun missed(count: Int?, complete: Boolean = true) =
        MissedEventsUi(downMs = 108_000, count = count, complete = complete)

    @Test
    fun theResultIsNoneSomeOrUnknown() {
        assertEquals(MissedResult.NONE, missedResultOf(missed(0)))
        assertEquals(MissedResult.SOME, missedResultOf(missed(2)))
        assertEquals(MissedResult.UNKNOWN, missedResultOf(missed(2, complete = false)))
        assertEquals(MissedResult.UNKNOWN, missedResultOf(missed(null)))
        assertEquals(MissedResult.UNKNOWN, missedResultOf(null))
    }

    @Test
    fun restoredCardNoSoundDurationInWordsUntilOk() = runComposeUiTest {
        var ok = 0
        show { RestoredCard(108_000, missed(0), { ok++ }) }
        onNodeWithTag("restored_body")
            .assertTextContains("interrupted for 1 min 48 s.", substring = true)
        onNodeWithTag("restored_result").assertTextContains("heard no sound", substring = true)
        onNodeWithTag("restored_ok").performClick()
        assertEquals(1, ok)
    }

    @Test
    fun restoredCardWithSoundsAndWithoutAReport() = runComposeUiTest {
        show { RestoredCard(90_000, missed(2), {}) }
        onNodeWithTag("restored_result").assertTextContains("heard 2 sounds", substring = true)
    }

    @Test
    fun incompleteReportNeverShowsACount() = runComposeUiTest {
        show { RestoredCard(90_000, missed(2, complete = false), {}) }
        onNodeWithTag("restored_result")
            .assertTextContains("don't know whether there were sounds", substring = true)
    }
}
