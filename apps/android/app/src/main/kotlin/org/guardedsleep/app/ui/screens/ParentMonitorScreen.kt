@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package org.guardedsleep.app.ui.screens

import android.content.res.Configuration
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.common.AdaptiveButtonRow
import org.guardedsleep.app.ui.common.AlertChipSpec
import org.guardedsleep.app.ui.common.AppCard
import org.guardedsleep.app.ui.common.BannerSpec
import org.guardedsleep.app.ui.common.BannerStack
import org.guardedsleep.app.ui.common.Body
import org.guardedsleep.app.ui.common.DockTile
import org.guardedsleep.app.ui.common.DockTiles
import org.guardedsleep.app.ui.common.Glyph
import org.guardedsleep.app.ui.common.GlyphIcon
import org.guardedsleep.app.ui.common.ListenButton
import org.guardedsleep.app.ui.common.MetaLine
import org.guardedsleep.app.ui.common.PauseSnackbar
import org.guardedsleep.app.ui.common.PrimaryButton
import org.guardedsleep.app.ui.common.QuietButton
import org.guardedsleep.app.ui.common.RestoredCard
import org.guardedsleep.app.ui.common.ScreenRoot
import org.guardedsleep.app.ui.common.SecondaryButton
import org.guardedsleep.app.ui.common.SheetOverlay
import org.guardedsleep.app.ui.common.SoundStrip
import org.guardedsleep.app.ui.common.StatusBar
import org.guardedsleep.app.ui.common.StopMonitoringDialog
import org.guardedsleep.app.ui.common.StripState
import org.guardedsleep.app.ui.common.SystemSettings
import org.guardedsleep.app.ui.common.TalkInterruptEffect
import org.guardedsleep.app.ui.common.TalkStatus
import org.guardedsleep.app.ui.common.TalkTile
import org.guardedsleep.app.ui.common.ToolRow
import org.guardedsleep.app.ui.common.formatClock
import org.guardedsleep.app.ui.common.formatDuration
import org.guardedsleep.app.ui.common.formatDurationWords
import org.guardedsleep.app.ui.common.formatTime
import org.guardedsleep.app.ui.common.pauseTileLabel
import org.guardedsleep.app.ui.common.rememberElapsedNow
import org.guardedsleep.app.ui.common.rememberWallNow
import org.guardedsleep.app.ui.model.AlertOutput
import org.guardedsleep.app.ui.model.AlertStyle
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.Banner
import org.guardedsleep.app.ui.model.BannerKind
import org.guardedsleep.app.ui.model.LevelHistory
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.ListenMode
import org.guardedsleep.app.ui.model.MissedEventsUi
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.ReadinessIssue
import org.guardedsleep.app.ui.model.SettingsState
import org.guardedsleep.app.ui.model.SilenceRequest
import org.guardedsleep.app.ui.model.SilenceStatus
import org.guardedsleep.app.ui.model.parentBanners
import org.guardedsleep.app.ui.model.rankBanners
import org.guardedsleep.app.ui.theme.LocalAppColors
import org.guardedsleep.app.ui.theme.StateColors

enum class ParentSheet {
    SILENCE,
    SNOOZE,
    PROBE,
    READINESS,
    HELP,
    MORE,
    STOP,
    CHOOSE_OTHER,
    BANNERS,
}

/** Bars of the last minute in the sound strip: one per second. */
private const val STRIP_BARS = 60

/**
 * S09: the calm parent monitor (`redesign-calm-ui` design sections 3, 6 and 7). The status bar is
 * pinned on top, then the tool row; the middle scrolls (banners, hero line, sound strip, meta
 * line); the dock is pinned at the bottom (Listen, then Talk, Pause and More). Priorities follow
 * `parent-alerts` "Banner priority on the monitor": the ranking is a pure function of the contract,
 * this screen only renders it. [levelHistory] feeds the strip and can be seeded by tests.
 */
@Composable
fun ParentMonitorScreen(
    controller: AppController,
    onSettings: () -> Unit,
    initialSheet: ParentSheet? = null,
    /** After "Choose another connection": the caller asks the home to list the connections. */
    onChosenOther: () -> Unit = {},
    levelHistory: LevelHistory = remember { LevelHistory() },
) {
    val p = controller.parent.collectAsState().value
    val app = controller.app.collectAsState().value
    val settings = controller.settings.collectAsState().value
    val name = customPeerName(app.peerName)
    val elapsed by rememberElapsedNow()
    val wall by rememberWallNow()
    var sheet by remember { mutableStateOf(initialSheet) }
    var talkHint by remember { mutableStateOf<Int?>(null) }
    // Minutes of a pause that may still be undone; set only when no noise event ran at its start.
    var undoMinutes by remember { mutableStateOf<Int?>(null) }
    val connected = p.link.phase == LinkPhase.CONNECTED
    // Verified once in this session: the link has had contact with the baby unit.
    val verified = connected || p.link.lastAliveElapsedMs != null
    val chooseOther = {
        controller.chooseAnotherConnection()
        onChosenOther()
    }
    val snoozeLeft = (p.snoozedUntilWallMs ?: 0L) - wall
    val snoozed = snoozeLeft > 0
    val acknowledged = (p.link.acknowledgedUntilElapsedMs ?: 0L) > elapsed
    val windowWidth = LocalWindowInfo.current.containerSize.width
    val wide =
        LocalConfiguration.current.orientation == Configuration.ORIENTATION_LANDSCAPE ||
            with(LocalDensity.current) { windowWidth.toDp() } >= 600.dp

    // A pause that ends, or a noise event that starts, takes its Undo away.
    LaunchedEffect(p.noiseAlertActive, snoozed) {
        if (p.noiseAlertActive || !snoozed) undoMinutes = null
    }

    val stack = rankBanners(parentBanners(p, settings.alertStyle, wall))
    val specOf: @Composable (Banner) -> BannerSpec = { b ->
        bannerSpec(
            b,
            p,
            settings,
            controller,
            wall,
            snoozed,
            onSilence = { sheet = ParentSheet.SILENCE },
            onProbe = { sheet = ParentSheet.PROBE },
        )
    }
    val shownSpecs = stack.shown.map { specOf(it) }
    val foldedSpecs = stack.folded.map { specOf(it) }
    val vibrateOnly = settings.alertStyle == AlertStyle.DISCREET
    val bars =
        remember(elapsed, connected) {
            if (connected) {
                levelHistory.push(elapsed, p.level)
                levelHistory.bars(elapsed, STRIP_BARS)
            } else {
                levelHistory.clear()
                List(STRIP_BARS) { 0f }
            }
        }
    val micSilent = connected && p.peer.micSilent == true

    val middle: @Composable ColumnScope.() -> Unit = {
        val restoredAfterMs = p.restoredAfterMs
        if (restoredAfterMs != null) {
            // Until OK: the card never dismisses itself, a new outage replaces it.
            RestoredCard(restoredAfterMs, p.missed, { controller.dismissRestored() })
        }
        if (!app.backgroundProtection) {
            AppCard(Modifier.testTag("parent_fgs_missing"), container = StateColors.Red) {
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    GlyphIcon(Glyph.EXCLAIM, StateColors.OnState)
                    Text(
                        stringResource(R.string.native_ui_fgs_missing_bar),
                        color = StateColors.OnState,
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                    )
                }
                Text(
                    stringResource(R.string.native_ui_fgs_missing_body),
                    color = StateColors.OnState,
                    style = MaterialTheme.typography.bodyLarge,
                )
                PrimaryButton(
                    stringResource(R.string.native_ui_fgs_missing_fix),
                    { controller.startMonitoring() },
                    "parent_fgs_fix",
                )
            }
        }
        if (p.link.stalePairingHint)
            Body(
                stringResource(R.string.native_ui_link_stale_pairing_hint),
                tag = "parent_stale_pairing_hint",
            )
        if (app.coachPending)
            CoachCard(controller) {
                controller.dismissCoach()
                sheet = ParentSheet.PROBE
            }
        if (restoredAfterMs == null) {
            p.missed?.let { MissedCard(it, wall) { controller.dismissMissedEvents() } }
        }
        BannerStack(shownSpecs, foldedSpecs, onMore = { sheet = ParentSheet.BANNERS })
        Hero(p, elapsed, wall, snoozed, micSilent)
        SoundStrip(
            bars,
            p.babyThreshold ?: settings.threshold,
            when {
                !connected -> StripState.OFFLINE
                micSilent -> StripState.MIC_SILENT
                else -> StripState.LIVE
            },
        )
        metaText(p, settings, connected)?.let { MetaLine(it, { sheet = ParentSheet.SILENCE }) }
        if (p.link.phase == LinkPhase.CONNECTING) {
            // Wrong place (travelling): before the first connection of a run, one tap at levels 0
            // and 1, the stop confirmation from level 2 (`connection-status`).
            QuietButton(
                stringResource(R.string.native_ui_conn_alert_wrong_connection),
                { if (p.link.level >= 2) sheet = ParentSheet.CHOOSE_OTHER else chooseOther() },
                "parent_choose_other_connection",
            )
        }
        if (p.link.phase == LinkPhase.FAILED_NEEDS_USER) {
            PrimaryButton(
                stringResource(R.string.native_ui_link_pair_again),
                { controller.startPairing() },
                "parent_pair_again",
            )
        }
        if (!connected && p.link.phase != LinkPhase.FAILED_NEEDS_USER) {
            AdaptiveButtonRow {
                SecondaryButton(
                    stringResource(R.string.native_ui_help_reconnect_open),
                    { sheet = ParentSheet.HELP },
                    "parent_help_reconnect",
                )
                if (p.link.phase != LinkPhase.PEER_STOPPED) {
                    SecondaryButton(
                        stringResource(R.string.native_ui_link_try_now),
                        { controller.tryReconnectNow() },
                        "parent_try_now",
                    )
                }
            }
        }
        if (
            p.link.level >= 2 &&
                !connected &&
                p.link.phase != LinkPhase.PEER_STOPPED &&
                p.link.phase != LinkPhase.FAILED_NEEDS_USER
        ) {
            if (acknowledged) {
                Body(
                    stringResource(
                        R.string.native_ui_link_alarm_ack_note,
                        formatClock(((p.link.acknowledgedUntilElapsedMs ?: 0L) - elapsed)),
                    ),
                    tag = "parent_ack_note",
                )
            } else {
                PrimaryButton(
                    stringResource(R.string.native_ui_link_alarm_ack),
                    { controller.acknowledgeAlarm() },
                    "parent_ack",
                    container = StateColors.Red,
                    content = StateColors.OnState,
                )
            }
        }
    }
    val dock: @Composable ColumnScope.() -> Unit = {
        TalkStatus(
            p,
            elapsed,
            talkHint,
            coachVisible = connected && !settings.talkCoachSeen && !app.coachPending,
            onKeepListening = { controller.setListen(ListenMode.ON) },
            onPermissionResult = { controller.talkPermissionResult(it) },
            onPermissionDismiss = { controller.dismissTalkPermission() },
            onCoachDismiss = { controller.dismissTalkCoach() },
        )
        val undo = undoMinutes
        if (undo != null && snoozed && !p.noiseAlertActive) {
            PauseSnackbar(
                undo,
                onUndo = {
                    controller.cancelSnooze()
                    undoMinutes = null
                },
                onTimeout = { undoMinutes = null },
            )
        }
        ListenButton(p.listen, connected, { controller.setListen(it) })
        DockTiles { m ->
            TalkTile(
                p,
                elapsed,
                wall,
                onPress = { pressed ->
                    if (pressed) controller.dismissTalkCoach()
                    controller.setTalk(pressed)
                },
                onHint = { talkHint = it },
                modifier = m,
            )
            DockTile(
                Glyph.PAUSE,
                pauseTileLabel(snoozed),
                stringResource(R.string.native_ui_snooze_button),
                "parent_snooze",
                { sheet = ParentSheet.SNOOZE },
                m,
                active = snoozed,
            )
            DockTile(
                Glyph.DOTS,
                stringResource(R.string.native_ui_common_more),
                stringResource(R.string.native_ui_common_more),
                "parent_overflow",
                { sheet = ParentSheet.MORE },
                m,
            )
        }
    }

    TalkInterruptEffect { controller.interruptTalk() }
    ScreenRoot("parent_monitor") {
        Column(Modifier.fillMaxSize()) {
            StatusBar(
                p.link,
                name,
                backgroundProtection = app.backgroundProtection,
                babyBatteryPct = p.peer.batteryPct.takeIf { connected },
                babyCharging = p.peer.charging,
            )
            Spacer(Modifier.height(6.dp))
            ToolRow(
                alert =
                    AlertChipSpec(
                        vibrateOnly = vibrateOnly,
                        iconOnly =
                            vibrateOnly && stack.shown.any { it.kind == BannerKind.VIBRATE_ONLY },
                        onClick = { sheet = ParentSheet.MORE },
                    ),
                // The night control is wired by the night-mode slice.
                night = null,
                onSettings = onSettings,
            )
            // Sheets and dialogs draw inside this box, so their scrim leaves the bar readable.
            Box(Modifier.weight(1f).fillMaxWidth()) {
                Column(Modifier.fillMaxSize()) {
                    if (wide) {
                        Row(
                            Modifier.weight(1f).fillMaxWidth().padding(16.dp),
                            horizontalArrangement = Arrangement.spacedBy(16.dp),
                        ) {
                            Column(
                                Modifier.weight(1f).verticalScroll(rememberScrollState()),
                                verticalArrangement = Arrangement.spacedBy(12.dp),
                                content = middle,
                            )
                            Column(
                                Modifier.weight(1f).verticalScroll(rememberScrollState()),
                                verticalArrangement = Arrangement.spacedBy(8.dp),
                                content = dock,
                            )
                        }
                    } else {
                        Column(
                            Modifier.weight(1f)
                                .fillMaxWidth()
                                .verticalScroll(rememberScrollState())
                                .padding(horizontal = 16.dp, vertical = 8.dp),
                            verticalArrangement = Arrangement.spacedBy(12.dp),
                            content = middle,
                        )
                        Column(
                            Modifier.fillMaxWidth()
                                .padding(horizontal = 16.dp)
                                .padding(bottom = 16.dp, top = 8.dp)
                                .testTag("parent_dock"),
                            verticalArrangement = Arrangement.spacedBy(8.dp),
                            content = dock,
                        )
                    }
                }
                when (sheet) {
                    ParentSheet.SILENCE -> SilenceSheet(p, controller) { sheet = null }
                    ParentSheet.SNOOZE ->
                        SnoozeSheet(
                            snoozed,
                            p.snoozedUntilWallMs,
                            controller,
                            onStarted = { minutes ->
                                sheet = null
                                undoMinutes = minutes.takeIf { !p.noiseAlertActive }
                            },
                        ) {
                            sheet = null
                        }
                    ParentSheet.PROBE ->
                        ProbeSheet(
                            controller,
                            connected,
                            settings.alertOutput,
                            settings.alertStyle,
                            onReadiness = { sheet = ParentSheet.READINESS },
                        ) {
                            sheet = null
                        }
                    ParentSheet.READINESS ->
                        ReadinessSheet(p, controller, onProbe = { sheet = ParentSheet.PROBE }) {
                            sheet = null
                        }
                    ParentSheet.HELP -> HelpSheet { sheet = null }
                    ParentSheet.BANNERS ->
                        SheetOverlay(
                            "sheet_banners",
                            stringResource(R.string.native_ui_banner_list_title),
                            { sheet = null },
                        ) {
                            foldedSpecs.forEach {
                                org.guardedsleep.app.ui.common.BannerCard(it)
                            }
                            PrimaryButton(
                                stringResource(R.string.native_ui_common_ok),
                                { sheet = null },
                                "sheet_banners_ok",
                                Modifier.fillMaxWidth(),
                            )
                        }
                    ParentSheet.MORE ->
                        SheetOverlay(
                            "sheet_more",
                            stringResource(R.string.native_ui_common_more),
                            { sheet = null },
                        ) {
                            AlertStyleChoice(settings.alertStyle) { style ->
                                controller.updateSettings { it.copy(alertStyle = style) }
                            }
                            Spacer(Modifier.height(8.dp))
                            SecondaryButton(
                                stringResource(R.string.native_ui_probe_button),
                                { sheet = ParentSheet.PROBE },
                                "parent_probe",
                                Modifier.fillMaxWidth(),
                            )
                            SecondaryButton(
                                stringResource(R.string.native_ui_common_settings),
                                {
                                    sheet = null
                                    onSettings()
                                },
                                "parent_settings",
                                Modifier.fillMaxWidth(),
                            )
                            if (app.autoStarted && verified) {
                                Spacer(Modifier.height(8.dp))
                                SecondaryButton(
                                    stringResource(R.string.native_ui_connections_choose_another),
                                    { sheet = ParentSheet.CHOOSE_OTHER },
                                    "menu_choose_other_connection",
                                    Modifier.fillMaxWidth(),
                                )
                            }
                            Spacer(Modifier.height(16.dp))
                            SecondaryButton(
                                stringResource(R.string.native_ui_parent_stop),
                                { sheet = ParentSheet.STOP },
                                "parent_stop",
                                Modifier.fillMaxWidth(),
                                destructive = true,
                            )
                        }
                    ParentSheet.STOP ->
                        StopMonitoringDialog(
                            name = name,
                            onConfirm = {
                                sheet = null
                                controller.stopMonitoring()
                            },
                            onDismiss = { sheet = null },
                        )
                    ParentSheet.CHOOSE_OTHER ->
                        StopMonitoringDialog(
                            name = name,
                            tag = "dialog_choose_other",
                            confirmTag = "dialog_choose_other_confirm",
                            cancelTag = "dialog_choose_other_cancel",
                            onConfirm = {
                                sheet = null
                                chooseOther()
                            },
                            onDismiss = { sheet = null },
                        )
                    null -> Unit
                }
            }
        }
    }
}

/** The banner of the stack as copy plus action; the readiness fixes open the matching setting. */
@Composable
private fun bannerSpec(
    b: Banner,
    p: ParentMonitorState,
    settings: SettingsState,
    controller: AppController,
    wall: Long,
    snoozed: Boolean,
    onSilence: () -> Unit,
    onProbe: () -> Unit,
): BannerSpec {
    val ctx = LocalContext.current
    val fix = stringResource(R.string.native_ui_banner_fix)
    return when (b.kind) {
        BannerKind.SECURITY ->
            BannerSpec(
                b,
                stringResource(R.string.native_ui_parent_security_title),
                stringResource(R.string.native_ui_parent_security_body),
                Glyph.EXCLAIM,
            )
        BannerKind.MIC_SILENT ->
            if (b.id == "mic_permission_lost")
                BannerSpec(
                    b,
                    stringResource(R.string.native_ui_parent_unit_warning_mic_permission_lost),
                    glyph = Glyph.MIC,
                )
            else
                BannerSpec(
                    b,
                    stringResource(R.string.native_ui_banner_mic_silent_title),
                    stringResource(R.string.native_ui_banner_mic_silent_body),
                    Glyph.MIC,
                )
        BannerKind.READINESS -> {
            val issue = p.readiness.first()
            val (title, body) = readinessCopy(issue, p.ownBatteryPct)
            val action: (() -> Unit)? =
                when (issue) {
                    ReadinessIssue.NOTIFICATIONS_BLOCKED -> { ->
                        SystemSettings.launch(ctx, SystemSettings.notificationSettings(ctx))
                    }
                    ReadinessIssue.FULL_SCREEN_BLOCKED -> { ->
                        SystemSettings.launch(ctx, SystemSettings.fullScreenIntentSettings(ctx))
                    }
                    ReadinessIssue.ALARM_VOLUME_ZERO,
                    ReadinessIssue.ALARM_VOLUME_LOW -> { ->
                        SystemSettings.launch(ctx, SystemSettings.soundSettings())
                    }
                    ReadinessIssue.DND_BYPASS_MISSING -> { ->
                        SystemSettings.launch(ctx, SystemSettings.dndAccess())
                    }
                    ReadinessIssue.BATTERY_OPTIMIZED -> { ->
                        SystemSettings.launch(ctx, SystemSettings.batteryOptimization(ctx))
                    }
                    ReadinessIssue.HEADPHONES_CONNECTED -> { ->
                        controller.updateSettings {
                            it.copy(alertOutput = AlertOutput.SPEAKER_ONLY)
                        }
                    }
                    ReadinessIssue.NEVER_TESTED -> onProbe
                    ReadinessIssue.PARENT_BATTERY_LOW -> null
                }
            BannerSpec(
                b,
                title,
                body,
                if (issue == ReadinessIssue.PARENT_BATTERY_LOW) Glyph.BATTERY else Glyph.BELL,
                fix.takeIf { action != null },
                action,
            )
        }
        BannerKind.BABY_BATTERY_LOW ->
            BannerSpec(
                b,
                stringResource(
                    R.string.native_ui_battery_baby_low,
                    (p.peer.batteryPct ?: 0).toString(),
                ),
                glyph = Glyph.BATTERY,
            )
        BannerKind.BABY_UNPLUGGED ->
            BannerSpec(
                b,
                stringResource(R.string.native_ui_parent_unit_warning_not_charging),
                glyph = Glyph.BATTERY,
            )
        BannerKind.SILENCE ->
            BannerSpec(
                b,
                stringResource(
                    when (p.peer.silence) {
                        SilenceStatus.SCREEN_MAY_LIGHT ->
                            R.string.native_ui_silence_screen_may_light
                        SilenceStatus.MAY_BUZZ -> R.string.native_ui_silence_may_vibrate
                        else -> R.string.native_ui_silence_may_ring
                    }
                ),
                glyph = Glyph.BELL,
                actionLabel = fix,
                onAction = onSilence,
            )
        BannerKind.UNIT_WARNING ->
            BannerSpec(
                b,
                stringResource(
                    when (b.id.removePrefix("unit_warning_")) {
                        "page_hidden" -> R.string.native_ui_parent_unit_warning_page_hidden
                        "wake_lock_off" -> R.string.native_ui_parent_unit_warning_wake_lock_off
                        else -> R.string.native_ui_parent_unit_warning_resumed_from_sleep
                    }
                ),
            )
        BannerKind.WEAK_CONNECTION ->
            BannerSpec(b, stringResource(R.string.native_ui_link_weak), glyph = Glyph.BROKEN_LINK)
        BannerKind.PAUSED ->
            BannerSpec(
                b,
                stringResource(
                    R.string.native_ui_snooze_banner_title,
                    formatTime(p.snoozedUntilWallMs ?: wall),
                ),
                stringResource(R.string.native_ui_snooze_banner_body),
                Glyph.PAUSE,
                stringResource(R.string.native_ui_snooze_resume),
                { controller.cancelSnooze() },
            )
        BannerKind.VIBRATE_ONLY -> {
            val minutes = settings.discreetEscalation.minutes
            BannerSpec(
                b,
                stringResource(R.string.native_ui_alert_style_vibrate_title),
                stringResource(
                    when {
                        minutes == null -> R.string.native_ui_alert_style_body_never
                        snoozed -> R.string.native_ui_alert_style_body_after_pause
                        else -> R.string.native_ui_alert_style_body_no_sound
                    },
                    *(if (minutes != null && snoozed) arrayOf<Any>(minutes) else emptyArray()),
                ),
                Glyph.ZZ,
                stringResource(R.string.native_ui_alert_style_turn_sound_on),
                { controller.updateSettings { it.copy(alertStyle = AlertStyle.SOUND) } },
            )
        }
    }
}

/** Title and sub-line of a readiness item (banner copy). */
@Composable
private fun readinessCopy(issue: ReadinessIssue, ownBatteryPct: Int?): Pair<String, String> {
    val (t, b) =
        when (issue) {
            ReadinessIssue.NOTIFICATIONS_BLOCKED ->
                R.string.native_ui_banner_ready_notifications_title to
                    R.string.native_ui_banner_ready_notifications_body
            ReadinessIssue.FULL_SCREEN_BLOCKED ->
                R.string.native_ui_banner_ready_full_screen_title to
                    R.string.native_ui_banner_ready_full_screen_body
            ReadinessIssue.ALARM_VOLUME_ZERO ->
                R.string.native_ui_banner_ready_alarm_volume_zero_title to
                    R.string.native_ui_banner_ready_alarm_volume_zero_body
            ReadinessIssue.ALARM_VOLUME_LOW ->
                R.string.native_ui_banner_ready_alarm_volume_low_title to
                    R.string.native_ui_banner_ready_alarm_volume_low_body
            ReadinessIssue.DND_BYPASS_MISSING ->
                R.string.native_ui_banner_ready_dnd_title to
                    R.string.native_ui_banner_ready_dnd_body
            ReadinessIssue.BATTERY_OPTIMIZED ->
                R.string.native_ui_banner_ready_battery_title to
                    R.string.native_ui_banner_ready_battery_body
            ReadinessIssue.PARENT_BATTERY_LOW ->
                R.string.native_ui_banner_ready_parent_battery_title to
                    R.string.native_ui_banner_ready_parent_battery_body
            ReadinessIssue.HEADPHONES_CONNECTED ->
                R.string.native_ui_banner_ready_headphones_title to
                    R.string.native_ui_banner_ready_headphones_body
            ReadinessIssue.NEVER_TESTED ->
                R.string.native_ui_banner_ready_never_tested_title to
                    R.string.native_ui_banner_ready_never_tested_body
        }
    return stringResource(t) to stringResource(b)
}

/** "Baby device in Quiet mode · Alerts tested 21:30"; null when there is nothing to say. */
@Composable
private fun metaText(p: ParentMonitorState, settings: SettingsState, connected: Boolean): String? {
    if (!connected) return null
    val parts = mutableListOf<String>()
    val s = if (p.peer.isWebClient) SilenceStatus.UNKNOWN else p.peer.silence
    when (s) {
        SilenceStatus.SILENT -> parts += stringResource(R.string.native_ui_silence_silent)
        SilenceStatus.UNKNOWN ->
            parts +=
                stringResource(
                    if (p.peer.isWebClient) R.string.native_ui_silence_web_unknown
                    else R.string.native_ui_silence_unknown
                )
        // The banner says it, with its fix.
        else -> Unit
    }
    settings.lastTestWallMs?.let {
        parts += stringResource(R.string.native_ui_monitor_alerts_tested, formatTime(it))
    }
    return parts.joinToString(" · ").ifEmpty { null }
}

/**
 * The hero: one line that says what the monitor knows, then how long it has been watching. Never
 * "All quiet" while disconnected, while the microphone is silent or when the report is incomplete
 * (the model leaves [ParentMonitorState.quietSinceWallMs] null then).
 */
@Composable
private fun Hero(
    p: ParentMonitorState,
    elapsed: Long,
    wall: Long,
    snoozed: Boolean,
    micSilent: Boolean,
) {
    val connected = p.link.phase == LinkPhase.CONNECTED
    val colors = LocalAppColors.current
    val noise = connected && p.noiseAlertActive
    val quietSince = p.quietSinceWallMs
    val lastSound = p.lastSound
    val text =
        when {
            !connected -> {
                val last = p.link.lastAliveElapsedMs
                if (last != null)
                    stringResource(
                        R.string.native_ui_parent_last_heard,
                        formatTime(wall - (elapsed - last)),
                    )
                else stringResource(R.string.native_ui_parent_no_contact_yet)
            }
            micSilent -> stringResource(R.string.native_ui_monitor_cant_hear)
            noise && snoozed ->
                stringResource(
                    R.string.native_ui_parent_sound_snoozed,
                    formatTime(p.noiseSinceWallMs ?: wall),
                )
            noise ->
                stringResource(
                    R.string.native_ui_parent_sound_since,
                    formatTime(p.noiseSinceWallMs ?: wall),
                )
            quietSince != null ->
                stringResource(
                    R.string.native_ui_parent_all_quiet_since,
                    formatTime(quietSince),
                )
            lastSound != null ->
                stringResource(
                    R.string.native_ui_parent_sound_at,
                    formatTime(lastSound.atWallMs),
                    formatDuration(lastSound.durationMs),
                )
            else -> stringResource(R.string.native_ui_parent_monitoring_now)
        }
    Column(Modifier.fillMaxWidth().padding(horizontal = 6.dp)) {
        Text(
            text,
            Modifier.fillMaxWidth().testTag("parent_headline").semantics { heading() },
            style = MaterialTheme.typography.headlineMedium.copy(fontWeight = FontWeight.Bold),
            color = if (noise && !snoozed) colors.bad else MaterialTheme.colorScheme.onBackground,
        )
        val since = p.monitoringSinceElapsedMs
        if (connected && since != null) {
            Text(
                stringResource(
                    R.string.native_ui_monitor_monitoring_for,
                    formatDurationWords((elapsed - since).coerceAtLeast(0L)),
                ),
                Modifier.testTag("parent_hero_sub"),
                style = MaterialTheme.typography.bodyLarge,
                color = colors.secondaryText,
            )
        }
    }
}

@Composable
private fun MissedCard(m: MissedEventsUi, wall: Long, onOk: () -> Unit) {
    val end = m.endWallMs ?: wall
    val start = end - m.downMs
    val count = m.count
    val text =
        when {
            m.complete && m.count == 0 -> stringResource(R.string.native_ui_missed_none)
            m.complete && count != null && count > 0 ->
                pluralStringResource(
                    R.plurals.native_ui_missed_some,
                    count,
                    count,
                    formatTime(start),
                    formatTime(end),
                    stringResource(
                        when {
                            (m.loudest ?: 0f) < 0.15f -> R.string.native_ui_parent_level_quiet
                            (m.loudest ?: 0f) < 0.5f -> R.string.native_ui_parent_level_some
                            else -> R.string.native_ui_parent_level_loud
                        }
                    ),
                )
            else -> {
                val startText = formatTime(start)
                val endText = formatTime(end)
                // A first-trust report has no gap, so both times match: name the time once.
                if (startText == endText) {
                    stringResource(R.string.native_ui_missed_unknown_at, endText)
                } else {
                    stringResource(R.string.native_ui_missed_unknown, startText, endText)
                }
            }
        }
    AppCard(Modifier.testTag("card_missed_events")) {
        Text(
            text,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurface,
        )
        QuietButton(stringResource(R.string.native_ui_common_ok), onOk, "card_missed_events_ok")
    }
}

@Composable
private fun CoachCard(controller: AppController, onRunTest: () -> Unit) {
    AppCard(
        Modifier.testTag("coach_first_run"),
        container = MaterialTheme.colorScheme.primaryContainer,
    ) {
        val c = MaterialTheme.colorScheme.onPrimaryContainer
        Text(
            stringResource(R.string.native_ui_coach_bar),
            color = c,
            style = MaterialTheme.typography.bodyLarge,
        )
        Text(
            stringResource(R.string.native_ui_coach_listen),
            color = c,
            style = MaterialTheme.typography.bodyLarge,
        )
        Text(
            stringResource(R.string.native_ui_coach_test),
            color = c,
            style = MaterialTheme.typography.bodyLarge,
        )
        PrimaryButton(
            stringResource(R.string.native_ui_coach_run_test),
            onRunTest,
            "coach_run_test",
        )
        QuietButton(
            stringResource(R.string.native_ui_common_later),
            { controller.dismissCoach() },
            "coach_later",
        )
    }
}

// ----- sheets ------------------------------------------------------------------------------------

@Composable
private fun SilenceSheet(p: ParentMonitorState, controller: AppController, onDismiss: () -> Unit) {
    val s = if (p.peer.isWebClient) SilenceStatus.UNKNOWN else p.peer.silence
    val title =
        stringResource(
            when (s) {
                SilenceStatus.SILENT -> R.string.native_ui_silence_silent
                SilenceStatus.SCREEN_MAY_LIGHT -> R.string.native_ui_silence_screen_may_light
                SilenceStatus.MAY_BUZZ -> R.string.native_ui_silence_may_vibrate
                SilenceStatus.MAY_RING,
                SilenceStatus.NO_ACCESS -> R.string.native_ui_silence_may_ring
                SilenceStatus.UNKNOWN ->
                    if (p.peer.isWebClient) R.string.native_ui_silence_web_unknown
                    else R.string.native_ui_silence_unknown
            }
        )
    SheetOverlay("sheet_silence", title, onDismiss) {
        when (s) {
            SilenceStatus.SILENT ->
                Body(stringResource(R.string.native_ui_silence_alarms_still_ring))
            SilenceStatus.NO_ACCESS ->
                Body(
                    stringResource(R.string.native_ui_silence_no_access),
                    tag = "sheet_silence_no_access",
                )
            SilenceStatus.UNKNOWN -> {
                Body(
                    stringResource(R.string.native_ui_silence_manual),
                    tag = "sheet_silence_manual",
                )
                if (p.peer.isWebClient)
                    Body(
                        stringResource(R.string.native_ui_silence_web_hint),
                        tag = "sheet_silence_web_hint",
                    )
            }
            else -> Unit
        }
        val canApply =
            s == SilenceStatus.SCREEN_MAY_LIGHT ||
                s == SilenceStatus.MAY_BUZZ ||
                s == SilenceStatus.MAY_RING
        when (p.silenceRequest) {
            SilenceRequest.PENDING ->
                Body(
                    stringResource(R.string.native_ui_silence_pending),
                    tag = "sheet_silence_pending",
                )
            SilenceRequest.FAILED -> {
                Body(
                    stringResource(R.string.native_ui_silence_failed),
                    tag = "sheet_silence_failed",
                    color = LocalAppColors.current.bad,
                )
                Body(stringResource(R.string.native_ui_silence_manual))
            }
            else -> Unit
        }
        if (canApply && p.silenceRequest != SilenceRequest.PENDING) {
            PrimaryButton(
                stringResource(
                    if (s == SilenceStatus.SCREEN_MAY_LIGHT) R.string.native_ui_silence_apply_full
                    else R.string.native_ui_silence_apply
                ),
                { controller.requestSilenceBaby() },
                if (s == SilenceStatus.SCREEN_MAY_LIGHT) "sheet_silence_apply_full"
                else "sheet_silence_apply",
            )
        }
    }
}

@Composable
private fun SnoozeSheet(
    snoozed: Boolean,
    untilWallMs: Long?,
    controller: AppController,
    /** A pause was started; the screen decides whether Undo is offered. */
    onStarted: (minutes: Int) -> Unit,
    onDismiss: () -> Unit,
) {
    SheetOverlay("sheet_snooze", stringResource(R.string.native_ui_snooze_title), onDismiss) {
        Body(stringResource(R.string.native_ui_snooze_help))
        PrimaryButton(
            stringResource(R.string.native_ui_snooze_minutes, "10"),
            {
                controller.snooze(10)
                onStarted(10)
            },
            "sheet_snooze_10",
        )
        PrimaryButton(
            stringResource(R.string.native_ui_snooze_minutes, "30"),
            {
                controller.snooze(30)
                onStarted(30)
            },
            "sheet_snooze_30",
        )
        if (snoozed) {
            SecondaryButton(
                stringResource(R.string.native_ui_snooze_end),
                {
                    controller.cancelSnooze()
                    onDismiss()
                },
                "sheet_snooze_end",
                Modifier.fillMaxWidth(),
            )
            Body(
                stringResource(
                    R.string.native_ui_snooze_banner_title,
                    formatTime(untilWallMs ?: 0L),
                )
            )
        }
    }
}

@Composable
private fun ProbeSheet(
    controller: AppController,
    connected: Boolean,
    output: AlertOutput,
    style: AlertStyle,
    onReadiness: () -> Unit,
    onDismiss: () -> Unit,
) {
    var stage by remember { mutableIntStateOf(0) }
    LaunchedEffect(stage) {
        if (stage == 1) {
            delay(5000)
            stage = 2
        }
    }
    SheetOverlay("sheet_probe", stringResource(R.string.native_ui_probe_title), onDismiss) {
        when (stage) {
            0 -> {
                val discreet = style == AlertStyle.DISCREET
                Body(
                    stringResource(
                        if (discreet) R.string.native_ui_probe_body_discreet
                        else R.string.native_ui_probe_body
                    )
                )
                Body(
                    stringResource(
                        if (discreet) R.string.native_ui_probe_style_discreet
                        else R.string.native_ui_probe_style_sound
                    ),
                    tag = "sheet_probe_style",
                )
                Body(
                    stringResource(
                        R.string.native_ui_probe_output,
                        stringResource(
                            if (output == AlertOutput.SPEAKER_ONLY)
                                R.string.native_ui_settings_output_speaker
                            else R.string.native_ui_settings_output_default
                        ),
                    ),
                    tag = "sheet_probe_output",
                )
                if (!connected)
                    Body(
                        stringResource(R.string.native_ui_probe_local_only),
                        tag = "sheet_probe_local_only",
                        color = LocalAppColors.current.warn,
                    )
                PrimaryButton(
                    stringResource(R.string.native_ui_probe_run),
                    {
                        controller.requestTestAlert()
                        stage = 1
                    },
                    "sheet_probe_run",
                )
            }
            1 -> Body(stringResource(R.string.native_ui_probe_running), tag = "sheet_probe_running")
            else -> {
                Text(
                    stringResource(R.string.native_ui_probe_question),
                    Modifier.testTag("sheet_probe_question"),
                    style = MaterialTheme.typography.titleLarge,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                AdaptiveButtonRow {
                    PrimaryButton(
                        stringResource(R.string.native_ui_common_yes),
                        {
                            controller.reportTestResult(true)
                            onDismiss()
                        },
                        "sheet_probe_yes",
                    )
                    PrimaryButton(
                        stringResource(R.string.native_ui_common_no),
                        {
                            controller.reportTestResult(false)
                            onReadiness()
                        },
                        "sheet_probe_no",
                    )
                }
            }
        }
    }
}

@Composable
private fun ReadinessSheet(
    p: ParentMonitorState,
    controller: AppController,
    onProbe: () -> Unit,
    onDismiss: () -> Unit,
) {
    val ctx = LocalContext.current
    SheetOverlay("sheet_readiness", stringResource(R.string.native_ui_readiness_title), onDismiss) {
        if (p.readiness.isEmpty()) {
            Body(
                stringResource(R.string.native_ui_readiness_ok),
                tag = "sheet_readiness_ok",
                color = LocalAppColors.current.ok,
            )
        }
        p.readiness.forEach { issue ->
            val text =
                stringResource(
                    when (issue) {
                        ReadinessIssue.NOTIFICATIONS_BLOCKED ->
                            R.string.native_ui_readiness_notifications_off
                        ReadinessIssue.ALARM_VOLUME_ZERO ->
                            R.string.native_ui_readiness_alarm_volume_zero
                        ReadinessIssue.ALARM_VOLUME_LOW ->
                            R.string.native_ui_banner_ready_alarm_volume_low_title
                        ReadinessIssue.FULL_SCREEN_BLOCKED ->
                            R.string.native_ui_readiness_lock_screen
                        ReadinessIssue.DND_BYPASS_MISSING -> R.string.native_ui_readiness_dnd
                        ReadinessIssue.BATTERY_OPTIMIZED -> R.string.native_ui_readiness_battery
                        ReadinessIssue.HEADPHONES_CONNECTED ->
                            R.string.native_ui_readiness_headphones
                        ReadinessIssue.PARENT_BATTERY_LOW ->
                            R.string.native_ui_battery_parent_low_generic
                        ReadinessIssue.NEVER_TESTED -> R.string.native_ui_readiness_never_tested
                    }
                )
            val fix: Pair<Int, () -> Unit>? =
                when (issue) {
                    ReadinessIssue.NOTIFICATIONS_BLOCKED ->
                        R.string.native_ui_readiness_fix_notifications to
                            {
                                SystemSettings.launch(ctx, SystemSettings.notificationSettings(ctx))
                            }
                    ReadinessIssue.ALARM_VOLUME_ZERO,
                    ReadinessIssue.ALARM_VOLUME_LOW ->
                        R.string.native_ui_banner_fix to
                            {
                                SystemSettings.launch(ctx, SystemSettings.soundSettings())
                            }
                    ReadinessIssue.NEVER_TESTED -> R.string.native_ui_probe_button to onProbe
                    ReadinessIssue.FULL_SCREEN_BLOCKED ->
                        R.string.native_ui_common_allow to
                            {
                                SystemSettings.launch(
                                    ctx,
                                    SystemSettings.fullScreenIntentSettings(ctx),
                                )
                            }
                    ReadinessIssue.DND_BYPASS_MISSING ->
                        R.string.native_ui_readiness_fix_dnd to
                            {
                                SystemSettings.launch(ctx, SystemSettings.dndAccess())
                            }
                    ReadinessIssue.BATTERY_OPTIMIZED ->
                        R.string.native_ui_readiness_fix_battery to
                            {
                                SystemSettings.launch(ctx, SystemSettings.batteryOptimization(ctx))
                            }
                    ReadinessIssue.HEADPHONES_CONNECTED ->
                        R.string.native_ui_common_change to
                            {
                                controller.updateSettings {
                                    it.copy(alertOutput = AlertOutput.SPEAKER_ONLY)
                                }
                            }
                    ReadinessIssue.PARENT_BATTERY_LOW -> null
                }
            AppCard(Modifier.testTag("sheet_readiness_" + issue.name.lowercase())) {
                Text(
                    text,
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                if (fix != null)
                    SecondaryButton(
                        stringResource(fix.first),
                        fix.second,
                        "sheet_readiness_fix_" + issue.name.lowercase(),
                    )
            }
        }
    }
}

@Composable
internal fun HelpSheet(onDismiss: () -> Unit) {
    SheetOverlay(
        "sheet_help_reconnect",
        stringResource(R.string.native_ui_help_reconnect_title),
        onDismiss,
    ) {
        Body(
            stringResource(R.string.native_ui_help_reconnect_step1),
            color = MaterialTheme.colorScheme.onSurface,
        )
        Body(
            stringResource(R.string.native_ui_help_reconnect_step2),
            color = MaterialTheme.colorScheme.onSurface,
        )
        Body(
            stringResource(R.string.native_ui_help_reconnect_step3),
            color = MaterialTheme.colorScheme.onSurface,
        )
        QuietButton(stringResource(R.string.native_ui_common_ok), onDismiss, "sheet_help_ok")
    }
}
