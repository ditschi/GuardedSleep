@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package org.guardedsleep.app.ui.screens

import android.content.res.Configuration
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
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
import org.guardedsleep.app.ui.common.AppCard
import org.guardedsleep.app.ui.common.Body
import org.guardedsleep.app.ui.common.ConfirmDialog
import org.guardedsleep.app.ui.common.Glyph
import org.guardedsleep.app.ui.common.GlyphIcon
import org.guardedsleep.app.ui.common.LinkBar
import org.guardedsleep.app.ui.common.ListenButton
import org.guardedsleep.app.ui.common.PrimaryButton
import org.guardedsleep.app.ui.common.QuietButton
import org.guardedsleep.app.ui.common.ScreenRoot
import org.guardedsleep.app.ui.common.SecondaryButton
import org.guardedsleep.app.ui.common.SheetOverlay
import org.guardedsleep.app.ui.common.SoundMeter
import org.guardedsleep.app.ui.common.StatusChip
import org.guardedsleep.app.ui.common.SystemSettings
import org.guardedsleep.app.ui.common.Tone
import org.guardedsleep.app.ui.common.ceilMinutes
import org.guardedsleep.app.ui.common.formatClock
import org.guardedsleep.app.ui.common.formatDuration
import org.guardedsleep.app.ui.common.formatTime
import org.guardedsleep.app.ui.common.rememberElapsedNow
import org.guardedsleep.app.ui.common.rememberWallNow
import org.guardedsleep.app.ui.model.AlertOutput
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.MissedEventsUi
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.ReadinessIssue
import org.guardedsleep.app.ui.model.SilenceRequest
import org.guardedsleep.app.ui.model.SilenceStatus
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
}

private class ChipSpec(
    val priority: Int,
    val tag: String,
    val glyph: Glyph,
    val extra: Glyph?,
    val text: String,
    val tone: Tone,
    val onClick: (() -> Unit)?,
    val stale: Boolean = false,
)

/**
 * S09: the main parent screen. Priorities follow alpha-ux-flows §5: bar colour + headline first.
 */
@Composable
fun ParentMonitorScreen(
    controller: AppController,
    onSettings: () -> Unit,
    initialSheet: ParentSheet? = null,
) {
    val p = controller.parent.collectAsState().value
    val app = controller.app.collectAsState().value
    val settings = controller.settings.collectAsState().value
    val name = customPeerName(app.peerName)
    val elapsed by rememberElapsedNow()
    val wall by rememberWallNow()
    var sheet by remember { mutableStateOf(initialSheet) }
    var expandChips by remember { mutableStateOf(false) }
    val connected = p.link.phase == LinkPhase.CONNECTED
    val snoozeLeft = (p.snoozedUntilWallMs ?: 0L) - wall
    val snoozed = snoozeLeft > 0
    val acknowledged = (p.link.acknowledgedUntilElapsedMs ?: 0L) > elapsed
    val windowWidth = LocalWindowInfo.current.containerSize.width
    val wide =
        LocalConfiguration.current.orientation == Configuration.ORIENTATION_LANDSCAPE ||
            with(LocalDensity.current) { windowWidth.toDp() } >= 600.dp

    val lastContactWall = p.link.lastAliveElapsedMs?.let { wall - (elapsed - it) }
    val chips =
        buildChips(
            p,
            snoozed,
            snoozeLeft,
            !connected,
            lastContactWall,
            onSilence = { sheet = ParentSheet.SILENCE },
            onReadiness = { sheet = ParentSheet.READINESS },
            onSnooze = { sheet = ParentSheet.SNOOZE },
            onProbe = { sheet = ParentSheet.PROBE },
        )
    val visible = if (expandChips) chips else chips.take(2)
    val hidden = chips.size - visible.size

    val top: @Composable ColumnScope.() -> Unit = {
        val restoredAfterMs = p.restoredAfterMs
        if (restoredAfterMs != null) {
            Row(
                Modifier.fillMaxWidth()
                    .background(StateColors.Green, RoundedCornerShape(12.dp))
                    .padding(12.dp)
                    .testTag("parent_restored_toast"),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                GlyphIcon(Glyph.CHECK, StateColors.OnState)
                Text(
                    stringResource(
                        R.string.native_ui_link_restored_after,
                        formatDuration(restoredAfterMs),
                    ),
                    color = StateColors.OnState,
                    style = MaterialTheme.typography.titleMedium,
                )
            }
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
        if (p.securityWarning) {
            AppCard(Modifier.testTag("parent_security_warning"), container = StateColors.Red) {
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    GlyphIcon(Glyph.EXCLAIM, StateColors.OnState)
                    Text(
                        stringResource(R.string.native_ui_parent_security_title),
                        color = StateColors.OnState,
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                    )
                }
                Text(
                    stringResource(R.string.native_ui_parent_security_body),
                    color = StateColors.OnState,
                    style = MaterialTheme.typography.bodyLarge,
                )
            }
        }
        p.missed?.let { MissedCard(it, wall) { controller.dismissMissedEvents() } }
        Headline(p, elapsed, wall, snoozed)
        SoundMeter(
            p.level,
            p.babyThreshold ?: settings.threshold,
            live = connected,
            alarm = p.noiseAlertActive && !snoozed,
        )
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
    }
    val middle: @Composable ColumnScope.() -> Unit = {
        if (visible.isNotEmpty()) {
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                visible.forEach { c ->
                    StatusChip(
                        c.glyph,
                        c.text,
                        c.tone,
                        c.tag,
                        onClick = c.onClick,
                        extraGlyph = c.extra,
                        dimmed = c.stale,
                    )
                }
                if (hidden > 0) {
                    StatusChip(
                        Glyph.DOTS,
                        stringResource(R.string.native_ui_common_more_count, hidden),
                        Tone.NEUTRAL,
                        "parent_chip_more",
                        onClick = { expandChips = true },
                    )
                }
            }
        }
    }
    val bottom: @Composable ColumnScope.() -> Unit = {
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
        ListenButton(p.listen, connected, { controller.setListen(it) })
        Spacer(Modifier.height(24.dp))
        AdaptiveButtonRow {
            SecondaryButton(
                stringResource(R.string.native_ui_snooze_button),
                { sheet = ParentSheet.SNOOZE },
                "parent_snooze",
            )
            SecondaryButton(
                stringResource(R.string.native_ui_probe_button),
                { sheet = ParentSheet.PROBE },
                "parent_probe",
            )
            SecondaryButton(
                stringResource(R.string.native_ui_common_more),
                { sheet = ParentSheet.MORE },
                "parent_overflow",
            )
        }
    }

    ScreenRoot("parent_monitor") {
        Column(Modifier.fillMaxSize()) {
            LinkBar(p.link, name, backgroundProtection = app.backgroundProtection)
            if (wide) {
                Row(
                    Modifier.weight(1f).fillMaxWidth().padding(16.dp),
                    horizontalArrangement = Arrangement.spacedBy(16.dp),
                ) {
                    Column(
                        Modifier.weight(1f).verticalScroll(rememberScrollState()),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                        content = top,
                    )
                    Column(
                        Modifier.weight(1f).verticalScroll(rememberScrollState()),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        middle()
                        Spacer(Modifier.height(8.dp))
                        bottom()
                    }
                }
            } else {
                Column(
                    Modifier.weight(1f)
                        .fillMaxWidth()
                        .verticalScroll(rememberScrollState())
                        .padding(horizontal = 16.dp, vertical = 12.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    top()
                    middle()
                }
                Column(
                    Modifier.fillMaxWidth()
                        .padding(horizontal = 16.dp)
                        .padding(bottom = 16.dp, top = 8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                    content = bottom,
                )
            }
        }
        when (sheet) {
            ParentSheet.SILENCE -> SilenceSheet(p, controller) { sheet = null }
            ParentSheet.SNOOZE -> SnoozeSheet(snoozed, snoozeLeft, controller) { sheet = null }
            ParentSheet.PROBE ->
                ProbeSheet(
                    controller,
                    connected,
                    settings.alertOutput,
                    onReadiness = { sheet = ParentSheet.READINESS },
                ) {
                    sheet = null
                }
            ParentSheet.READINESS ->
                ReadinessSheet(p, controller, onProbe = { sheet = ParentSheet.PROBE }) {
                    sheet = null
                }
            ParentSheet.HELP -> HelpSheet { sheet = null }
            ParentSheet.MORE ->
                SheetOverlay(
                    "sheet_more",
                    stringResource(R.string.native_ui_common_more),
                    { sheet = null },
                ) {
                    SecondaryButton(
                        stringResource(R.string.native_ui_common_settings),
                        {
                            sheet = null
                            onSettings()
                        },
                        "parent_settings",
                        Modifier.fillMaxWidth(),
                    )
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
                ConfirmDialog(
                    tag = "dialog_stop",
                    title = stringResource(R.string.native_ui_stop_title),
                    body =
                        if (name != null) stringResource(R.string.native_ui_stop_parent_body, name)
                        else stringResource(R.string.native_ui_stop_parent_body_default),
                    confirmText = stringResource(R.string.native_ui_stop_confirm),
                    cancelText = stringResource(R.string.native_ui_stop_cancel),
                    confirmTag = "dialog_stop_confirm",
                    cancelTag = "dialog_stop_cancel",
                    onConfirm = {
                        sheet = null
                        controller.stopMonitoring()
                    },
                    onDismiss = { sheet = null },
                )
            null -> Unit
        }
    }
}

@Composable
private fun buildChips(
    p: ParentMonitorState,
    snoozed: Boolean,
    snoozeLeft: Long,
    stale: Boolean,
    staleAtWallMs: Long?,
    onSilence: () -> Unit,
    onReadiness: () -> Unit,
    onSnooze: () -> Unit,
    onProbe: () -> Unit,
): List<ChipSpec> {
    val out = mutableListOf<ChipSpec>()
    val issues = p.readiness
    // Peer-reported chips are last-known values while the link is down: dim them and say when they
    // were true.
    val suffix =
        if (stale && staleAtWallMs != null)
            " " + stringResource(R.string.native_ui_parent_stale_suffix, formatTime(staleAtWallMs))
        else ""
    @Composable
    fun issueText(i: ReadinessIssue): String =
        stringResource(
            when (i) {
                ReadinessIssue.NOTIFICATIONS_BLOCKED ->
                    R.string.native_ui_readiness_notifications_off
                ReadinessIssue.ALARM_VOLUME_ZERO -> R.string.native_ui_readiness_alarm_volume_zero
                ReadinessIssue.FULL_SCREEN_BLOCKED -> R.string.native_ui_readiness_lock_screen
                ReadinessIssue.DND_BYPASS_MISSING -> R.string.native_ui_readiness_dnd
                ReadinessIssue.BATTERY_OPTIMIZED -> R.string.native_ui_readiness_battery
                ReadinessIssue.HEADPHONES_CONNECTED -> R.string.native_ui_readiness_headphones
                ReadinessIssue.PARENT_BATTERY_LOW -> R.string.native_ui_battery_parent_low_generic
                ReadinessIssue.NEVER_TESTED -> R.string.native_ui_readiness_never_tested
            }
        )
    for (i in issues) {
        when (i) {
            ReadinessIssue.NOTIFICATIONS_BLOCKED,
            ReadinessIssue.ALARM_VOLUME_ZERO ->
                out +=
                    ChipSpec(
                        1,
                        "parent_readiness_chip",
                        Glyph.BELL,
                        null,
                        issueText(i),
                        Tone.BAD,
                        onReadiness,
                    )
            ReadinessIssue.FULL_SCREEN_BLOCKED,
            ReadinessIssue.DND_BYPASS_MISSING,
            ReadinessIssue.BATTERY_OPTIMIZED ->
                out +=
                    ChipSpec(
                        5,
                        "parent_readiness_chip",
                        Glyph.BELL,
                        null,
                        issueText(i),
                        Tone.WARN,
                        onReadiness,
                    )
            ReadinessIssue.HEADPHONES_CONNECTED ->
                out +=
                    ChipSpec(
                        8,
                        "parent_readiness_chip",
                        Glyph.HEADPHONES,
                        null,
                        issueText(i),
                        Tone.INFO,
                        onReadiness,
                    )
            ReadinessIssue.NEVER_TESTED ->
                out +=
                    ChipSpec(
                        9,
                        "parent_test_chip",
                        Glyph.BELL,
                        null,
                        issueText(i),
                        Tone.INFO,
                        onProbe,
                    )
            ReadinessIssue.PARENT_BATTERY_LOW -> Unit
        }
    }
    val own = p.ownBatteryPct
    if (ReadinessIssue.PARENT_BATTERY_LOW in issues || (own != null && own < 20)) {
        val text =
            if (own != null) stringResource(R.string.native_ui_battery_parent_low, own.toString())
            else issueText(ReadinessIssue.PARENT_BATTERY_LOW)
        out +=
            ChipSpec(
                3,
                "parent_own_battery_chip",
                Glyph.BATTERY,
                null,
                text,
                if (own != null && own < 10) Tone.BAD else Tone.WARN,
                onReadiness,
            )
    }
    val pct = p.peer.batteryPct
    if (pct != null) {
        val charging = p.peer.charging == true
        when {
            pct < 5 && !charging ->
                out +=
                    ChipSpec(
                        2,
                        "parent_baby_battery_chip",
                        Glyph.BATTERY,
                        null,
                        stringResource(R.string.native_ui_battery_baby_low, pct.toString()) +
                            suffix,
                        Tone.BAD,
                        null,
                        stale,
                    )
            pct < 15 && !charging ->
                out +=
                    ChipSpec(
                        4,
                        "parent_baby_battery_chip",
                        Glyph.BATTERY,
                        null,
                        stringResource(R.string.native_ui_battery_baby_low, pct.toString()) +
                            suffix,
                        Tone.WARN,
                        null,
                        stale,
                    )
            else ->
                out +=
                    ChipSpec(
                        11,
                        "parent_baby_battery_chip",
                        Glyph.BATTERY,
                        null,
                        stringResource(R.string.native_ui_battery_baby, pct.toString()) + suffix,
                        Tone.NEUTRAL,
                        null,
                        stale,
                    )
        }
    }
    val s = if (p.peer.isWebClient) SilenceStatus.UNKNOWN else p.peer.silence
    val silenceRes =
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
    val (sg, sx, st, sp) =
        when (s) {
            SilenceStatus.SILENT -> Quad(Glyph.MOON, null, Tone.NEUTRAL, 10)
            SilenceStatus.SCREEN_MAY_LIGHT -> Quad(Glyph.MOON, Glyph.SUN, Tone.WARN, 5)
            SilenceStatus.MAY_BUZZ,
            SilenceStatus.MAY_RING,
            SilenceStatus.NO_ACCESS -> Quad(Glyph.BELL, null, Tone.WARN, 5)
            SilenceStatus.UNKNOWN -> Quad(Glyph.QUESTION, null, Tone.NEUTRAL, 10)
        }
    out +=
        ChipSpec(
            sp,
            "parent_silence_chip",
            sg,
            sx,
            stringResource(silenceRes) + suffix,
            st,
            onSilence,
            stale,
        )
    for (w in p.peer.unitWarnings) {
        val (res, prio, tone) =
            when (w) {
                "mic_permission_lost" ->
                    Triple(R.string.native_ui_parent_unit_warning_mic_permission_lost, 2, Tone.BAD)
                "not_charging" ->
                    Triple(R.string.native_ui_parent_unit_warning_not_charging, 4, Tone.WARN)
                "page_hidden" ->
                    Triple(R.string.native_ui_parent_unit_warning_page_hidden, 5, Tone.WARN)
                "wake_lock_off" ->
                    Triple(R.string.native_ui_parent_unit_warning_wake_lock_off, 5, Tone.WARN)
                "resumed_from_sleep" ->
                    Triple(R.string.native_ui_parent_unit_warning_resumed_from_sleep, 5, Tone.WARN)
                else -> continue
            }
        out +=
            ChipSpec(
                prio,
                "parent_unit_warning_chip_$w",
                Glyph.EXCLAIM,
                null,
                stringResource(res) + suffix,
                tone,
                null,
                stale,
            )
    }
    if (snoozed) {
        out +=
            ChipSpec(
                6,
                "parent_snooze_chip",
                Glyph.ZZ,
                null,
                stringResource(
                    R.string.native_ui_snooze_active,
                    ceilMinutes(snoozeLeft).toString(),
                ),
                Tone.SNOOZE,
                onSnooze,
            )
    }
    if (
        p.link.quality == org.guardedsleep.app.ui.model.LinkQuality.POOR &&
            p.link.phase == LinkPhase.CONNECTED
    ) {
        out +=
            ChipSpec(
                7,
                "parent_quality_chip",
                Glyph.BROKEN_LINK,
                null,
                stringResource(R.string.native_ui_link_weak),
                Tone.WARN,
                null,
            )
    }
    return out.sortedBy { it.priority }
}

private data class Quad<A, B, C, D>(val a: A, val b: B, val c: C, val d: D)

@Composable
private fun Headline(p: ParentMonitorState, elapsed: Long, wall: Long, snoozed: Boolean) {
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
    Text(
        text,
        Modifier.fillMaxWidth().testTag("parent_headline").semantics { heading() },
        style = MaterialTheme.typography.headlineMedium.copy(fontWeight = FontWeight.Bold),
        color = if (noise && !snoozed) colors.bad else MaterialTheme.colorScheme.onBackground,
    )
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
            else ->
                stringResource(
                    R.string.native_ui_missed_unknown,
                    formatTime(start),
                    formatTime(end),
                )
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
    leftMs: Long,
    controller: AppController,
    onDismiss: () -> Unit,
) {
    SheetOverlay("sheet_snooze", stringResource(R.string.native_ui_snooze_title), onDismiss) {
        Body(stringResource(R.string.native_ui_snooze_help))
        PrimaryButton(
            stringResource(R.string.native_ui_snooze_minutes, "10"),
            {
                controller.snooze(10)
                onDismiss()
            },
            "sheet_snooze_10",
        )
        PrimaryButton(
            stringResource(R.string.native_ui_snooze_minutes, "30"),
            {
                controller.snooze(30)
                onDismiss()
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
            Body(stringResource(R.string.native_ui_snooze_active, ceilMinutes(leftMs).toString()))
        }
    }
}

@Composable
private fun ProbeSheet(
    controller: AppController,
    connected: Boolean,
    output: AlertOutput,
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
                Body(stringResource(R.string.native_ui_probe_body))
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
