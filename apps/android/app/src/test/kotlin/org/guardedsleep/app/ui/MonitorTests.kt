@file:OptIn(ExperimentalTestApi::class)

package org.guardedsleep.app.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertContentDescriptionContains
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.runComposeUiTest
import androidx.compose.ui.unit.dp
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.common.AlertChipSpec
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.common.ToolRow
import org.guardedsleep.app.ui.model.AlertStyle
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.ListenMode
import org.guardedsleep.app.ui.model.NightMode
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.PeerStatusUi
import org.guardedsleep.app.ui.model.ReadinessIssue
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.SettingsState
import org.guardedsleep.app.ui.model.SilenceStatus
import org.guardedsleep.app.ui.model.TalkEnd
import org.guardedsleep.app.ui.model.TalkUi
import org.guardedsleep.app.ui.model.TalkUnavailable
import org.guardedsleep.app.ui.model.Theme
import org.guardedsleep.app.ui.screens.ParentMonitorScreen
import org.guardedsleep.app.ui.theme.GuardedSleepTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.robolectric.RuntimeEnvironment

/**
 * The calm parent monitor (`redesign-calm-ui` slice 3): the hero, the sound strip, the dock, the
 * tool row, the banner stack, the renames and the two alert screens on the shared template. One
 * test per scenario of the requirement it names.
 */
class MonitorTests : ShotBase() {
    private val monitoring =
        AppState(
            role = Role.PARENT,
            onboardingDone = true,
            paired = true,
            monitoring = true,
            peerName = "Nursery",
        )

    private fun str(id: Int, vararg args: Any) =
        RuntimeEnvironment.getApplication().getString(id, *args)

    private fun ComposeUiTest.show(content: @Composable () -> Unit) = setContent {
        CompositionLocalProvider(LocalUiEnv provides TestEnv) {
            GuardedSleepTheme(Theme.DARK, content = content)
        }
    }

    private fun connected(
        p: ParentMonitorState = ParentMonitorState(),
        settings: SettingsState = SettingsState(talkCoachSeen = true),
    ) =
        FakeAppController(
            app = monitoring,
            parent = p.copy(link = Connected),
            settings = settings,
        )

    private fun ComposeUiTest.headline(text: String) =
        onNodeWithTag("parent_headline").assert(hasText(text, substring = true))

    // ---- hero and strip (3.1, `audio-monitoring` "Parent monitoring screen") ---------------

    @Test
    fun allQuietSinceShowsWhileConnectedAndQuiet() = runComposeUiTest {
        val c = connected(ParentMonitorState(quietSinceWallMs = WALL - 3_600_000L))
        show { ParentMonitorScreen(c, {}) }
        headline(str(R.string.native_ui_parent_all_quiet_since, "").trim())
        onNodeWithTag("parent_strip_alert_line").assertIsDisplayed()
    }

    @Test
    fun heroSaysHowLongTheMonitorHasBeenWatching() = runComposeUiTest {
        val c =
            connected(
                ParentMonitorState(
                    quietSinceWallMs = WALL - 60_000L,
                    monitoringSinceElapsedMs = ELAPSED - 87 * 60_000L,
                )
            )
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_hero_sub")
            .assert(hasText("1 h", substring = true).or(hasText("87", substring = true)))
    }

    @Test
    fun noAllQuietAfterARecoveryWithAnIncompleteReport() = runComposeUiTest {
        // The model keeps quietSinceWallMs null until the report covers the whole outage.
        val c = connected(ParentMonitorState(quietSinceWallMs = null))
        show { ParentMonitorScreen(c, {}) }
        headline(str(R.string.native_ui_parent_monitoring_now))
    }

    @Test
    fun microphoneSilentShowsNoAllQuietAndTheStripSaysNoSoundComesIn() = runComposeUiTest {
        val c =
            connected(
                ParentMonitorState(
                    quietSinceWallMs = WALL - 3_600_000L,
                    peer = PeerStatusUi(micSilent = true),
                )
            )
        show { ParentMonitorScreen(c, {}) }
        headline(str(R.string.native_ui_monitor_cant_hear))
        onNodeWithTag("parent_strip_note")
            .assert(hasText(str(R.string.native_ui_monitor_mic_silent_strip)))
        onNodeWithTag("parent_banner_mic_silent").assertIsDisplayed()
        onNodeWithTag("parent_headline")
            .assert(
                SemanticsMatcher("no All quiet") { n ->
                    n.config.getOrNull(SemanticsProperties.Text)?.none {
                        it.text.contains("quiet", true)
                    } == true
                }
            )
    }

    @Test
    fun stoppedBabyAndDisconnectedShowNoAllQuiet() = runComposeUiTest {
        val c =
            FakeAppController(
                app = monitoring,
                parent =
                    ParentMonitorState(
                        link = link(LinkPhase.PEER_STOPPED, agoMs = 30_000),
                        quietSinceWallMs = WALL,
                    ),
            )
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_headline")
            .assert(
                SemanticsMatcher("no All quiet") { n ->
                    n.config.getOrNull(SemanticsProperties.Text)?.none {
                        it.text.startsWith("All quiet")
                    } == true
                }
            )
        onNodeWithTag("parent_strip_note").assert(hasText(str(R.string.native_ui_monitor_no_sound)))
    }

    @Test
    fun theStripIsFlatWithNoSoundComingInWhileNotConnected() = runComposeUiTest {
        val c =
            FakeAppController(
                app = monitoring,
                parent = ParentMonitorState(link = link(LinkPhase.RECONNECTING, 1, 30_000)),
            )
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_strip_note").assert(hasText(str(R.string.native_ui_monitor_no_sound)))
        onNodeWithTag("parent_strip_alert_line").assertDoesNotExist()
    }

    // ---- dock (3.2) -----------------------------------------------------------------------

    @Test
    fun theDockOffersListenTalkPauseAndMoreAtLeast72dpHigh() = runComposeUiTest {
        val c = connected()
        show { ParentMonitorScreen(c, {}) }
        listOf("parent_talk", "parent_snooze", "parent_overflow").forEach {
            val h =
                onNodeWithTag(it).assertIsDisplayed().getUnclippedBoundsInRoot().let { b ->
                    b.bottom - b.top
                }
            assertTrue("$it is $h high", h >= 72.dp)
        }
        val listen = onNodeWithTag("parent_listen").assertIsDisplayed().getUnclippedBoundsInRoot()
        assertTrue(listen.bottom - listen.top >= 72.dp)
    }

    @Test
    fun listenIsUnavailableWhileNotConnectedAndSaysWhy() = runComposeUiTest {
        val c =
            FakeAppController(
                app = monitoring,
                parent = ParentMonitorState(link = link(LinkPhase.RECONNECTING, 1, 30_000)),
            )
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_listen")
            .assertIsNotEnabled()
            .assert(hasText(str(R.string.native_ui_listen_disabled)))
        onNodeWithTag("parent_listen").assert(hasText(str(R.string.native_ui_listen_disabled_sub)))
    }

    @Test
    fun theTalkStatusLineIsAStatusNotADisabledButton() = runComposeUiTest {
        val c = connected(ParentMonitorState(talk = TalkUi.Talking(ELAPSED + 20_000)))
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_talk_listening_paused")
            .assertIsDisplayed()
            .assert(hasText(str(R.string.native_ui_talk_paused_listening)))
        // No live region: nothing is spoken into the open microphone.
        onNodeWithTag("parent_talk_listening_paused")
            .assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.LiveRegion))
    }

    @Test
    fun talkAgainAtShowsTheTimeAsAFullContrastStatusSlot() = runComposeUiTest {
        val c =
            connected(
                ParentMonitorState(
                    talk = TalkUi.Unavailable(TalkUnavailable.COOLDOWN, ELAPSED + 60_000L),
                    talkEnd = TalkEnd.LIMIT,
                )
            )
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_talk").assert(hasText(str(R.string.native_ui_talk_again)))
        onNodeWithTag("parent_talk")
            .assert(hasText("02:15", substring = true).or(hasText("2:15", substring = true)))
    }

    @Test
    fun theDockLabelsNeverWrap() = runComposeUiTest {
        val c = connected()
        RuntimeEnvironment.setQualifiers("+de")
        try {
            show { ParentMonitorScreen(c, {}) }
            // The German tile says "Pausieren"; the full label stays the accessibility label.
            onNode(
                    hasText("Pausieren") and hasAnyAncestor(hasTestTag("parent_snooze")),
                    useUnmergedTree = true,
                )
                .assertExists()
            onNodeWithTag("parent_snooze").assertIsDisplayed()
            onNodeWithTag("parent_snooze")
                .assertContentDescriptionContains("pausieren", substring = true, ignoreCase = true)
        } finally {
            RuntimeEnvironment.setQualifiers("+en")
        }
    }

    // ---- tool row (3.3) -------------------------------------------------------------------

    private fun toolRow(
        night: NightMode?,
        vibrateOnly: Boolean = false,
        iconOnly: Boolean = false,
    ) =
        @Composable {
            ToolRow(
                alert = AlertChipSpec(vibrateOnly, iconOnly, {}),
                night = night?.let { it to {} },
                onSettings = {},
            )
        }

    private fun ComposeUiTest.rowAt(
        fontScale: Float,
        locale: String,
        content: @Composable () -> Unit,
    ) {
        RuntimeEnvironment.setQualifiers("+$locale")
        RuntimeEnvironment.setFontScale(fontScale)
        show(content)
    }

    @Test
    fun theToolRowKeepsTheGearAndOrdersTheControls() {
        for (locale in listOf("en", "de")) {
            for (scale in listOf(1.0f, 1.3f, 2.0f)) {
                runComposeUiTest {
                    try {
                        rowAt(scale, locale, toolRow(NightMode.ON))
                        val chip =
                            onNodeWithTag("tool_alert_chip")
                                .assertIsDisplayed()
                                .getUnclippedBoundsInRoot()
                        val night =
                            onNodeWithTag("tool_night")
                                .assertIsDisplayed()
                                .getUnclippedBoundsInRoot()
                        val gear =
                            onNodeWithTag("tool_settings")
                                .assertIsDisplayed()
                                .getUnclippedBoundsInRoot()
                        val msg = "$locale at $scale"
                        // Reading order is the traversal order: chip, night, settings.
                        assertTrue(msg, night.left < gear.left)
                        if (scale >= 1.5f) {
                            assertTrue("$msg: chip on row 1", chip.bottom <= night.top)
                            assertTrue("$msg: night and gear share row 2", night.top == gear.top)
                        } else {
                            assertTrue(
                                "$msg: one row",
                                chip.top == gear.top || chip.top - gear.top in (-8).dp..8.dp,
                            )
                            assertTrue(msg, chip.left < night.left)
                        }
                        assertTrue("$msg: gear is 48 dp", gear.right - gear.left >= 48.dp)
                    } finally {
                        RuntimeEnvironment.setQualifiers("+en")
                        RuntimeEnvironment.setFontScale(1f)
                    }
                }
            }
        }
    }

    @Test
    fun theNightControlIsIconOnlyWhenOffAndAboveOnePointOneFive() = runComposeUiTest {
        show(toolRow(NightMode.OFF))
        onNodeWithTag("tool_night")
            .assertContentDescriptionContains(str(R.string.native_ui_tool_night_a11y_off))
        onNodeWithTag("tool_night").assert(hasText(str(R.string.native_ui_tool_night_on)).not())
    }

    @Test
    fun theNightControlShowsOnAndAutoAtNormalTextSize() {
        for ((state, label) in
            listOf(
                NightMode.ON to R.string.native_ui_tool_night_on,
                NightMode.AUTO to R.string.native_ui_tool_night_auto,
            )) {
            runComposeUiTest {
                show(toolRow(state))
                onNodeWithTag("tool_night").assert(hasText(str(label)))
            }
        }
        runComposeUiTest {
            try {
                rowAt(1.3f, "en", toolRow(NightMode.ON))
                onNodeWithTag("tool_night")
                    .assert(hasText(str(R.string.native_ui_tool_night_on)).not())
            } finally {
                RuntimeEnvironment.setFontScale(1f)
            }
        }
    }

    @Test
    fun theAlertChipIsLabelledWhileNoBannerCoversVibrateOnly() = runComposeUiTest {
        show(toolRow(null, vibrateOnly = true, iconOnly = false))
        onNodeWithTag("tool_alert_chip")
            .assert(hasText(str(R.string.native_ui_alert_style_chip_vibrate)))
        onNodeWithTag("tool_alert_chip")
            .assertContentDescriptionContains(str(R.string.native_ui_alert_style_chip_a11y_vibrate))
    }

    // ---- banners (3.4, 3.5, 3.6) ----------------------------------------------------------

    private val fourNotices =
        ParentMonitorState(
            peer = PeerStatusUi(micSilent = true, batteryPct = 14, charging = false),
            readiness = listOf(ReadinessIssue.ALARM_VOLUME_ZERO),
        )

    @Test
    fun fourNoticesShowTwoBannersAndAPlusTwoMoreRow() = runComposeUiTest {
        val c =
            connected(
                fourNotices,
                SettingsState(alertStyle = AlertStyle.DISCREET, talkCoachSeen = true),
            )
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_banner_mic_silent").assertIsDisplayed()
        onNodeWithTag("parent_banner_alarm_volume_zero").assertIsDisplayed()
        onNodeWithTag("parent_banner_baby_battery_low").assertDoesNotExist()
        onNodeWithTag("parent_banner_vibrate_only").assertDoesNotExist()
        onNodeWithTag("parent_banner_more")
            .assertIsDisplayed()
            .assertContentDescriptionContains("2", substring = true)
        onNodeWithTag("parent_banner_more").performClick()
        onNodeWithTag("sheet_banners").assertIsDisplayed()
        onNodeWithTag("parent_banner_baby_battery_low").assertExists()
        onNodeWithTag("parent_banner_vibrate_only").assertExists()
    }

    @Test
    fun indicatorFoldedTheChipReturnsToItsLabelledForm() = runComposeUiTest {
        val c =
            connected(
                fourNotices,
                SettingsState(alertStyle = AlertStyle.DISCREET, talkCoachSeen = true),
            )
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("tool_alert_chip")
            .assert(hasText(str(R.string.native_ui_alert_style_chip_vibrate)))
    }

    @Test
    fun theChipIsIconOnlyWhileItsBannerShows() = runComposeUiTest {
        val c =
            connected(
                ParentMonitorState(),
                SettingsState(alertStyle = AlertStyle.DISCREET, talkCoachSeen = true),
            )
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_banner_vibrate_only").assertIsDisplayed()
        onNodeWithTag("tool_alert_chip")
            .assert(hasText(str(R.string.native_ui_alert_style_chip_vibrate)).not())
    }

    @Test
    fun warningBannersAreLiveRegionsAndNotesAreNot() = runComposeUiTest {
        val c =
            connected(
                ParentMonitorState(peer = PeerStatusUi(micSilent = true)),
                SettingsState(alertStyle = AlertStyle.DISCREET, talkCoachSeen = true),
            )
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_banner_mic_silent")
            .assert(
                SemanticsMatcher.expectValue(SemanticsProperties.LiveRegion, LiveRegionMode.Polite)
            )
        onNodeWithTag("parent_banner_vibrate_only")
            .assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.LiveRegion))
    }

    @Test
    fun notificationsSwitchedOffShowTheWorstReadinessItemWithFix() = runComposeUiTest {
        val c =
            connected(
                ParentMonitorState(
                    readiness =
                        listOf(ReadinessIssue.NOTIFICATIONS_BLOCKED, ReadinessIssue.NEVER_TESTED)
                )
            )
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_banner_notifications_blocked")
            .assert(
                hasText(str(R.string.native_ui_banner_ready_notifications_title), substring = true)
            )
        onNodeWithTag("parent_banner_notifications_blocked_action").assertIsDisplayed()
        onNodeWithTag("parent_banner_never_tested").assertDoesNotExist()
    }

    @Test
    fun theLowVolumeHintShowsWhenNothingElseFails() = runComposeUiTest {
        val c = connected(ParentMonitorState(readiness = listOf(ReadinessIssue.ALARM_VOLUME_LOW)))
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_banner_alarm_volume_low")
            .assert(
                hasText(
                    str(R.string.native_ui_banner_ready_alarm_volume_low_title),
                    substring = true,
                )
            )
    }

    @Test
    fun vibrateOnlyBannerSaysNoSoundAndOffersTurnSoundOn() = runComposeUiTest {
        val c =
            connected(
                settings = SettingsState(alertStyle = AlertStyle.DISCREET, talkCoachSeen = true)
            )
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_banner_vibrate_only")
            .assert(hasText(str(R.string.native_ui_alert_style_vibrate_title), substring = true))
            .assert(hasText(str(R.string.native_ui_alert_style_body_no_sound), substring = true))
        onNodeWithTag("parent_banner_vibrate_only_action").performClick()
        waitForIdle()
        assertEquals(AlertStyle.SOUND, c.settingsFlow.value.alertStyle)
    }

    @Test
    fun withEscalationOffTheBannerSaysNoiseAlertsNeverMakeASound() = runComposeUiTest {
        val c =
            connected(
                settings =
                    SettingsState(
                        alertStyle = AlertStyle.DISCREET,
                        discreetEscalation = org.guardedsleep.app.ui.model.DiscreetEscalation.OFF,
                        talkCoachSeen = true,
                    )
            )
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_banner_vibrate_only")
            .assert(hasText(str(R.string.native_ui_alert_style_body_never), substring = true))
    }

    // ---- pause (3.6) ----------------------------------------------------------------------

    @Test
    fun pausedByMistakeUndoEndsThePause() = runComposeUiTest {
        val c = connected()
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_snooze").performClick()
        onNodeWithTag("sheet_snooze_10").performClick()
        onNodeWithTag("parent_pause_snackbar").assertIsDisplayed()
        onNodeWithTag("parent_banner_paused").assertIsDisplayed()
        onNodeWithTag("parent_pause_undo").performClick()
        waitForIdle()
        assertEquals(listOf("snooze:10", "cancelSnooze"), c.calls.filter { it.contains("nooze") })
        onNodeWithTag("parent_pause_snackbar").assertDoesNotExist()
        onNodeWithTag("parent_banner_paused").assertDoesNotExist()
    }

    @Test
    fun theUndoGoesAwayAfterEightSeconds() = runComposeUiTest {
        val c = connected()
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_snooze").performClick()
        onNodeWithTag("sheet_snooze_30").performClick()
        onNodeWithTag("parent_pause_snackbar").assertIsDisplayed()
        mainClock.advanceTimeBy(8_500)
        onNodeWithTag("parent_pause_snackbar").assertDoesNotExist()
        // The pause itself stays.
        assertTrue(c.calls.none { it == "cancelSnooze" })
    }

    @Test
    fun aPauseWhileANoiseEventRunsOffersNoUndo() = runComposeUiTest {
        val c =
            connected(
                ParentMonitorState(
                    noiseAlertActive = true,
                    noiseSinceWallMs = WALL - 5_000L,
                    level = 0.6f,
                ),
                SettingsState(alertStyle = AlertStyle.DISCREET, talkCoachSeen = true),
            )
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_snooze").performClick()
        onNodeWithTag("sheet_snooze_10").performClick()
        waitForIdle()
        onNodeWithTag("parent_pause_snackbar").assertDoesNotExist()
        assertTrue(c.calls.contains("snooze:10"))
    }

    @Test
    fun aPauseFromARunningVibrateOnlyAlertOffersNoUndoAndClosesTheAlert() = runComposeUiTest {
        var closed = false
        val c =
            connected(
                ParentMonitorState(noiseAlertActive = true, level = 0.8f),
                SettingsState(alertStyle = AlertStyle.DISCREET, talkCoachSeen = true),
            )
        show { NoiseAlertScreen(c) { closed = true } }
        onNodeWithTag("alert_snooze_10").performClick()
        assertTrue(closed)
        onNodeWithTag("parent_pause_snackbar").assertDoesNotExist()
    }

    @Test
    fun theOldStoredDiscreetValueRendersAsVibrateOnly() = runComposeUiTest {
        // The stored identifier is unchanged ("discreet"); only the copy says Vibrate only.
        val c =
            connected(
                settings =
                    SettingsState(alertStyle = AlertStyle.valueOf("DISCREET"), talkCoachSeen = true)
            )
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_banner_vibrate_only")
            .assert(hasText(str(R.string.native_ui_alert_style_vibrate_title), substring = true))
        assertEquals("Vibrate only", str(R.string.native_ui_alert_style_vibrate_title))
    }

    // ---- alert screens on the template (3.7) -----------------------------------------------

    @Test
    fun theNoiseAlertUsesTheTemplateWithListenFirst() = runComposeUiTest {
        val c =
            connected(
                ParentMonitorState(
                    noiseAlertActive = true,
                    noiseSinceWallMs = WALL - 12_000L,
                    level = 0.8f,
                )
            )
        show { NoiseAlertScreen(c) {} }
        onNodeWithTag("noise_alert_template").assertIsDisplayed()
        onNodeWithTag("alert_title").assert(hasText(str(R.string.native_ui_alert_title)))
        onNodeWithTag("alert_stop_sound").assertIsDisplayed()
        val listen = onNodeWithTag("alert_listen").getUnclippedBoundsInRoot()
        val pause = onNodeWithTag("alert_snooze_10").getUnclippedBoundsInRoot()
        assertTrue("Listen comes before the pause buttons", listen.top < pause.top)
        onNodeWithTag("alert_listen").performClick()
        assertEquals(ListenMode.ON, c.parentFlow.value.listen)
    }

    @Test
    fun theNoiseAlertKeepsItsActionVisibleAtTwoHundredPercent() = runComposeUiTest {
        RuntimeEnvironment.setFontScale(2f)
        try {
            val c =
                connected(
                    ParentMonitorState(
                        noiseAlertActive = true,
                        noiseSinceWallMs = WALL - 12_000L,
                        level = 0.8f,
                    )
                )
            show { NoiseAlertScreen(c) {} }
            onNodeWithTag("alert_stop_sound").assertIsDisplayed()
        } finally {
            RuntimeEnvironment.setFontScale(1f)
        }
    }

    @Test
    fun babyBatteryCriticalIsAmberNotSnoozableAndNotARedScreen() = runComposeUiTest {
        val c = connected(ParentMonitorState(peer = PeerStatusUi(batteryPct = 4, charging = false)))
        show { LinkAlarmScreen(c) {} }
        onNodeWithTag("battery_alert_template").assertIsDisplayed()
        onNodeWithTag("battery_alert_ok").assertIsDisplayed()
        onNodeWithTag("alert_snooze_10").assertDoesNotExist()
        onNodeWithTag("battery_alert_counter")
            .assertContentDescriptionContains("4", substring = true)
    }

    @Test
    fun lowBatteryAndUnpluggedAreBannersNotScreens() = runComposeUiTest {
        val c =
            connected(
                ParentMonitorState(
                    peer =
                        PeerStatusUi(
                            batteryPct = 14,
                            charging = false,
                            unitWarnings = listOf("not_charging"),
                        )
                )
            )
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_banner_baby_battery_low").assertIsDisplayed()
        onNodeWithTag("parent_banner_baby_unplugged").assertIsDisplayed()
    }

    @Test
    fun theSilenceBannerOffersTheFixAndTheMetaLineNamesQuietMode() = runComposeUiTest {
        val c = connected(ParentMonitorState(peer = PeerStatusUi(silence = SilenceStatus.MAY_RING)))
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_banner_silence_action").performClick()
        onNodeWithTag("sheet_silence").assertIsDisplayed()
        onNodeWithTag("parent_meta").assertDoesNotExist()
    }

    // ---- More sheet (4.3) -----------------------------------------------------------------

    @Test
    fun theMoreSheetHasCloseNoConnectionSwitchAndStopApartWithConfirmation() = runComposeUiTest {
        val c = connected()
        show { ParentMonitorScreen(c, {}) }
        onNodeWithTag("parent_overflow").performClick()
        onNodeWithTag("sheet_more").assertIsDisplayed()
        onNodeWithTag("sheet_close").assertIsDisplayed()
        // No way to switch the connection while monitoring (`device-setup` "Switching mode").
        onNodeWithTag("menu_choose_other_connection").assertDoesNotExist()
        onNodeWithTag("connection_choice_1").assertDoesNotExist()
        val probe = onNodeWithTag("parent_probe").getUnclippedBoundsInRoot()
        val stop = onNodeWithTag("parent_stop").getUnclippedBoundsInRoot()
        assertTrue("Stop sits below a gap", stop.top - probe.bottom >= 32.dp)
        onNodeWithTag("parent_stop").performClick()
        onNodeWithTag("dialog_stop").assertIsDisplayed()
        assertTrue(c.calls.none { it == "stopMonitoring" })
        onNodeWithTag("dialog_stop_cancel").performClick()
        onNodeWithTag("sheet_more").assertDoesNotExist()
    }
}
