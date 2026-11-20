package org.guardedsleep.app.ui.screens

import android.content.Intent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.common.AdaptiveButtonRow
import org.guardedsleep.app.ui.common.AlertAction
import org.guardedsleep.app.ui.common.AlertCounter
import org.guardedsleep.app.ui.common.AlertProgress
import org.guardedsleep.app.ui.common.AlertStage
import org.guardedsleep.app.ui.common.AlertTemplate
import org.guardedsleep.app.ui.common.CauseCard
import org.guardedsleep.app.ui.common.Glyph
import org.guardedsleep.app.ui.common.GlyphIcon
import org.guardedsleep.app.ui.common.ListenButton
import org.guardedsleep.app.ui.common.PrimaryButton
import org.guardedsleep.app.ui.common.QuietButton
import org.guardedsleep.app.ui.common.RestoredCard
import org.guardedsleep.app.ui.common.ScreenRoot
import org.guardedsleep.app.ui.common.SecondaryButton
import org.guardedsleep.app.ui.common.SoundMeter
import org.guardedsleep.app.ui.common.StatusBar
import org.guardedsleep.app.ui.common.StatusKind
import org.guardedsleep.app.ui.common.StopMonitoringDialog
import org.guardedsleep.app.ui.common.SystemSettings
import org.guardedsleep.app.ui.common.alertStageOf
import org.guardedsleep.app.ui.common.customPeerName
import org.guardedsleep.app.ui.common.elapsedToWall
import org.guardedsleep.app.ui.common.formatClock
import org.guardedsleep.app.ui.common.formatTime
import org.guardedsleep.app.ui.common.outageStartElapsedMs
import org.guardedsleep.app.ui.common.rememberElapsedNow
import org.guardedsleep.app.ui.common.rememberWallNow
import org.guardedsleep.app.ui.common.spokenDuration
import org.guardedsleep.app.ui.common.statusHeadlineText
import org.guardedsleep.app.ui.common.statusKindOf
import org.guardedsleep.app.ui.model.AlertStyle
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.BabyBattery
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.ReadinessIssue
import org.guardedsleep.app.ui.theme.LocalSafetyColors
import org.guardedsleep.app.ui.theme.StateColors

/**
 * S10: the full-screen noise alert on the shared template (frame 08): a flat status strip, the caps
 * label that says what the phone does, the headline, the time since the sound began, one sentence,
 * the live level and one pinned "Stop sound" or "Stop vibration"; Listen comes first among the
 * quiet controls, then "Pause 10 min" and "Pause 30 min". The wine palette is fixed and never
 * follows the colour scheme, and no Undo is offered here: a pause from a running alert has
 * acknowledged the event (`parent-alerts` "Pause noise alerts").
 */
@Composable
fun NoiseAlertContent(controller: AppController, onClose: () -> Unit) {
    val p = controller.parent.collectAsState().value
    val app = controller.app.collectAsState().value
    val settings = controller.settings.collectAsState().value
    val wall by rememberWallNow()
    val ctx = LocalContext.current
    var soundStopped by remember { mutableStateOf(false) }
    val ended = !p.noiseAlertActive
    val escalatedAfter = p.noiseEscalatedAfterMin
    // Once the core switched the event to sound, the screen speaks of sound again.
    val discreet = settings.alertStyle == AlertStyle.DISCREET && escalatedAfter == null
    val palette = LocalSafetyColors.current.noise
    val since = p.noiseSinceWallMs ?: wall
    val down = (wall - since).coerceAtLeast(0L)
    val sinceText = formatTime(since)
    val name = customPeerName(app.peerName)
    val stripText = statusHeadlineText(statusKindOf(p.link), p.link, name)
    ScreenRoot("parent_noise_alert", background = palette.background) {
        AlertTemplate(
            tag = "noise_alert_template",
            palette = palette,
            label =
                stringResource(
                    when {
                        ended || soundStopped -> R.string.native_ui_noise_alert_label_stopped
                        discreet -> R.string.native_ui_noise_alert_label_vibrate
                        else -> R.string.native_ui_noise_alert_label_sound
                    }
                ),
            headline = stringResource(R.string.native_ui_alert_title),
            headlineTag = "alert_title",
            strip = {
                GlyphIcon(Glyph.LINK, palette.accent, size = 22.dp)
                Text(
                    stripText,
                    style = MaterialTheme.typography.titleSmall,
                    color = palette.onBackground,
                )
            },
            counter =
                AlertCounter(
                    formatClock(down),
                    stringResource(R.string.native_ui_noise_alert_counter_label, sinceText),
                    stringResource(
                        R.string.native_ui_noise_alert_counter_a11y,
                        spokenDuration(down),
                        sinceText,
                    ),
                ),
            counterTag = "alert_since",
            sentence =
                stringResource(
                    when {
                        ended -> R.string.native_ui_alert_ended
                        discreet -> R.string.native_ui_alert_discreet_hint
                        else -> R.string.native_ui_noise_alert_sentence_sound
                    }
                ),
            sentenceTag = if (discreet && !ended) "alert_discreet_hint" else "alert_sentence",
            body = {
                if (p.noiseIsTest) {
                    Text(
                        stringResource(R.string.native_ui_alert_test_badge),
                        Modifier.background(Color(0xFFFFE08A), RoundedCornerShape(8.dp))
                            .padding(horizontal = 12.dp, vertical = 4.dp)
                            .testTag("alert_test_badge"),
                        style = MaterialTheme.typography.titleMedium,
                        color = Color(0xFF3A2A00),
                        fontWeight = FontWeight.Bold,
                    )
                }
                SoundMeter(
                    p.level,
                    p.babyThreshold ?: settings.threshold,
                    live = true,
                    alarm = true,
                    tag = "alert_meter",
                )
                if (escalatedAfter != null && !ended) {
                    Text(
                        stringResource(
                            R.string.native_ui_parent_discreet_escalated,
                            escalatedAfter.toString(),
                        ),
                        Modifier.testTag("alert_escalated"),
                        style = MaterialTheme.typography.bodyLarge,
                        color = palette.onVariant,
                    )
                }
                if (soundStopped && !ended) {
                    Text(
                        stringResource(
                            if (discreet) R.string.native_ui_alert_vibration_stopped
                            else R.string.native_ui_alert_sound_stopped
                        ),
                        Modifier.testTag("alert_sound_stopped"),
                        style = MaterialTheme.typography.bodyLarge,
                        color = palette.onVariant,
                    )
                }
            },
            quiet = {
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    ListenButton(
                        p.listen,
                        p.link.phase == LinkPhase.CONNECTED,
                        { controller.setListen(it) },
                        tag = "alert_listen",
                        compact = true,
                    )
                    AdaptiveButtonRow {
                        SecondaryButton(
                            stringResource(R.string.native_ui_snooze_pause_minutes, "10"),
                            {
                                controller.snooze(10)
                                onClose()
                            },
                            "alert_snooze_10",
                        )
                        SecondaryButton(
                            stringResource(R.string.native_ui_snooze_pause_minutes, "30"),
                            {
                                controller.snooze(30)
                                onClose()
                            },
                            "alert_snooze_30",
                        )
                    }
                    QuietButton(
                        stringResource(R.string.native_ui_alert_open_app),
                        {
                            ctx.packageManager.getLaunchIntentForPackage(ctx.packageName)?.let {
                                ctx.startActivity(
                                    Intent(it).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                                )
                            }
                            onClose()
                        },
                        "alert_open_app",
                    )
                }
            },
            action =
                when {
                    ended -> {
                        {
                            AlertAction(
                                stringResource(R.string.native_ui_common_ok),
                                onClose,
                                "alert_close",
                                palette,
                            )
                        }
                    }
                    soundStopped -> null
                    else -> {
                        {
                            AlertAction(
                                stringResource(
                                    if (discreet) R.string.native_ui_alert_stop_vibration
                                    else R.string.native_ui_alert_stop_sound
                                ),
                                {
                                    soundStopped = true
                                    controller.stopAlertSound()
                                },
                                "alert_stop_sound",
                                palette,
                                sub =
                                    stringResource(
                                        if (discreet)
                                            R.string.native_ui_noise_alert_stop_vibration_sub
                                        else R.string.native_ui_noise_alert_stop_sound_sub
                                    ),
                            )
                        }
                    }
                },
        )
    }
}

private enum class AlarmVariant {
    LOST,
    STOPPED,
    BATTERY,
    NEEDS_PAIRING,
    RESTORED,
    NO_BACKGROUND,
}

/** S11: full-screen connection alarm, baby-stopped notice, battery critical, "connected again". */
@Composable
fun LinkAlarmContent(controller: AppController, onClose: () -> Unit) {
    val p = controller.parent.collectAsState().value
    val app = controller.app.collectAsState().value
    val name = customPeerName(app.peerName)
    val elapsed by rememberElapsedNow()
    val link = p.link
    val battery = p.peer.batteryPct
    val variant =
        when {
            !app.backgroundProtection -> AlarmVariant.NO_BACKGROUND
            link.phase == LinkPhase.PEER_STOPPED -> AlarmVariant.STOPPED
            link.phase == LinkPhase.FAILED_NEEDS_USER -> AlarmVariant.NEEDS_PAIRING
            link.phase != LinkPhase.CONNECTED && link.level >= 2 -> AlarmVariant.LOST
            link.phase == LinkPhase.CONNECTED &&
                battery != null &&
                battery < BabyBattery.CRITICAL_PCT &&
                p.peer.charging != true -> AlarmVariant.BATTERY
            link.phase == LinkPhase.CONNECTED -> AlarmVariant.RESTORED
            else -> AlarmVariant.LOST
        }
    if (variant == AlarmVariant.LOST) {
        ConnectionAlert(controller, name, elapsed, onClose)
        return
    }
    if (variant == AlarmVariant.BATTERY) {
        BatteryAlert(p, name, onClose)
        return
    }
    LinkNoticeContent(controller, variant, name, onClose)
}

/**
 * Frames 12, 13, 25 and 26: the connection alert on the shared template. The stage (vibrating,
 * loud, paused) and every time come from the core's level, `next_level_at_ms` and
 * `acknowledged_until`; this screen has no threshold of its own.
 */
@Composable
private fun ConnectionAlert(
    controller: AppController,
    name: String?,
    elapsed: Long,
    onClose: () -> Unit,
) {
    val p = controller.parent.collectAsState().value
    val link = p.link
    val wall by rememberWallNow()
    val ctx = LocalContext.current
    var confirmStop by remember { mutableStateOf(false) }
    var confirmWrong by remember { mutableStateOf(false) }
    val stage = alertStageOf(link, elapsed)
    val palette =
        if (stage == AlertStage.LOUD) LocalSafetyColors.current.loudAlarm
        else LocalSafetyColors.current.noConnection
    val firstConnection = link.phase == LinkPhase.CONNECTING
    // The core's times, read on the same clock as the core and shown as the device formats them.
    val nextLoudAt = link.nextLevelAtElapsedMs?.let { formatTime(elapsedToWall(it, elapsed, wall)) }
    val pausedAt =
        link.acknowledgedUntilElapsedMs?.let { formatTime(elapsedToWall(it, elapsed, wall)) }
    val headline =
        when (stage) {
            AlertStage.VIBRATING ->
                if (firstConnection) statusHeadlineText(StatusKind.COULDNT_CONNECT, link, name)
                else stringResource(R.string.native_ui_conn_alert_headline_check)
            AlertStage.LOUD -> statusHeadlineText(StatusKind.STILL_NO_CONNECTION, link, name)
            AlertStage.PAUSED ->
                stringResource(
                    R.string.native_ui_conn_alert_next_back,
                    pausedAt ?: formatTime(wall),
                )
        }
    val sentence =
        when (stage) {
            AlertStage.VIBRATING -> {
                val lead =
                    stringResource(
                        if (firstConnection) R.string.native_ui_conn_alert_never_started
                        else R.string.native_ui_conn_alert_cant_hear
                    )
                if (nextLoudAt != null)
                    lead + " " + stringResource(R.string.native_ui_conn_alert_next_loud, nextLoudAt)
                else lead
            }
            AlertStage.LOUD -> stringResource(R.string.native_ui_conn_alert_go_now)
            AlertStage.PAUSED -> stringResource(R.string.native_ui_conn_alert_paused_body)
        }
    val start = link.outageStartElapsedMs()
    val down = ((start?.let { elapsed - it }) ?: 0L).coerceAtLeast(0L)
    val spoken = spokenDuration(down)
    val counter =
        AlertCounter(
            formatClock(down),
            stringResource(R.string.native_ui_conn_alert_counter_label),
            stringResource(R.string.native_ui_conn_alert_counter_a11y, spoken),
        )
    val progress =
        when (stage) {
            AlertStage.VIBRATING ->
                AlertProgress(
                    3,
                    4,
                    stringResource(R.string.native_ui_conn_alert_progress_vibrating),
                )
            AlertStage.LOUD ->
                AlertProgress(4, 4, stringResource(R.string.native_ui_conn_alert_progress_loud))
            AlertStage.PAUSED ->
                AlertProgress(3, 4, stringResource(R.string.native_ui_conn_alert_progress_paused))
        }
    val stripText = statusHeadlineText(statusKindOf(link), link, name)
    ScreenRoot("parent_link_alarm", background = palette.background) {
        AlertTemplate(
            tag = "link_alarm_template",
            palette = palette,
            label =
                stringResource(
                    when (stage) {
                        AlertStage.VIBRATING -> R.string.native_ui_conn_alert_label_vibrating
                        AlertStage.LOUD -> R.string.native_ui_conn_alert_label_loud
                        AlertStage.PAUSED -> R.string.native_ui_conn_alert_label_paused
                    }
                ),
            headline = headline,
            headlineTag = "link_alarm_title",
            hazard = stage == AlertStage.LOUD,
            strip = {
                GlyphIcon(Glyph.BROKEN_LINK, palette.accent, size = 22.dp)
                Text(
                    stripText,
                    style = MaterialTheme.typography.titleSmall,
                    color = palette.onBackground,
                )
            },
            counter = counter,
            counterTag = "link_alarm_since",
            sentence = sentence,
            sentenceTag = "link_alarm_sentence",
            progress = progress,
            body = {
                CauseCard(
                    link.causeHint,
                    palette,
                    onTryNow = { controller.tryReconnectNow() },
                )
                if (link.stalePairingHint) {
                    Text(
                        stringResource(R.string.native_ui_link_stale_pairing_hint),
                        Modifier.testTag("link_alarm_stale_hint"),
                        style = MaterialTheme.typography.bodyLarge,
                        color = palette.onVariant,
                    )
                }
                if (stage == AlertStage.LOUD) {
                    // The loud alarm only passes Do Not Disturb with the bypass grant.
                    if (ReadinessIssue.DND_BYPASS_MISSING in p.readiness) {
                        Text(
                            stringResource(R.string.native_ui_conn_alert_dnd_missing),
                            Modifier.testTag("link_alarm_dnd_note"),
                            style = MaterialTheme.typography.bodyLarge,
                            color = palette.onVariant,
                        )
                        QuietButton(
                            stringResource(R.string.native_ui_conn_alert_dnd_open),
                            { SystemSettings.launch(ctx, SystemSettings.dndAccess()) },
                            "link_alarm_dnd_open",
                        )
                    } else {
                        Text(
                            stringResource(R.string.native_ui_conn_alert_dnd_note),
                            Modifier.testTag("link_alarm_dnd_note"),
                            style = MaterialTheme.typography.bodyLarge,
                            color = palette.onVariant,
                        )
                    }
                }
                if (firstConnection) {
                    // The way back to the saved connections; from this screen (level 2 and up) it
                    // goes through the stop confirmation, so a running alarm is never left by one
                    // tap.
                    QuietButton(
                        stringResource(R.string.native_ui_conn_alert_wrong_connection),
                        { confirmWrong = true },
                        "link_alarm_wrong_connection",
                    )
                }
            },
            quiet = {
                QuietButton(
                    stringResource(R.string.native_ui_parent_stop),
                    { confirmStop = true },
                    "link_alarm_stop",
                )
            },
            action =
                if (stage == AlertStage.PAUSED) null
                else {
                    {
                        AlertAction(
                            stringResource(R.string.native_ui_link_alarm_ack),
                            { controller.acknowledgeAlarm() },
                            "link_alarm_ack",
                            palette,
                        )
                    }
                },
        )
        if (confirmStop) {
            StopMonitoringDialog(
                name = name,
                onConfirm = {
                    confirmStop = false
                    controller.stopMonitoring()
                    onClose()
                },
                onDismiss = { confirmStop = false },
            )
        }
        if (confirmWrong) {
            StopMonitoringDialog(
                name = name,
                tag = "dialog_choose_other",
                confirmTag = "dialog_choose_other_confirm",
                cancelTag = "dialog_choose_other_cancel",
                onConfirm = {
                    confirmWrong = false
                    controller.chooseAnotherConnection()
                    onClose()
                },
                onDismiss = { confirmWrong = false },
            )
        }
    }
}

/**
 * Frame 31: the baby device's battery is almost empty (below 5 %, not charging). The amber palette
 * keeps red for "you can't hear the child"; this alert is not snoozable and has one action that
 * closes it. Low battery (below 15 %) and "no longer charging" are banners on the monitor.
 */
@Composable
private fun BatteryAlert(p: ParentMonitorState, name: String?, onClose: () -> Unit) {
    val palette = LocalSafetyColors.current.batteryCritical
    val pct = p.peer.batteryPct ?: 0
    val stripText = statusHeadlineText(statusKindOf(p.link), p.link, name)
    ScreenRoot("parent_link_alarm", background = palette.background) {
        AlertTemplate(
            tag = "battery_alert_template",
            palette = palette,
            label = stringResource(R.string.native_ui_battery_alert_label),
            headline = stringResource(R.string.native_ui_battery_alert_headline),
            headlineTag = "link_alarm_title",
            strip = {
                GlyphIcon(Glyph.BATTERY, palette.accent, size = 22.dp)
                Text(
                    stripText,
                    style = MaterialTheme.typography.titleSmall,
                    color = palette.onBackground,
                )
            },
            counter =
                AlertCounter(
                    stringResource(R.string.native_ui_monitor_battery_short, pct),
                    stringResource(R.string.native_ui_battery_alert_counter_label),
                    stringResource(R.string.native_ui_battery_alert_counter_a11y, pct),
                ),
            counterTag = "battery_alert_counter",
            sentence = stringResource(R.string.native_ui_battery_alert_sentence),
            sentenceTag = "link_alarm_body",
            body = {
                Text(
                    stringResource(R.string.native_ui_battery_alert_silent_note),
                    Modifier.testTag("battery_alert_silent_note"),
                    style = MaterialTheme.typography.bodyLarge,
                    color = palette.onVariant,
                )
            },
            action = {
                AlertAction(
                    stringResource(R.string.native_ui_battery_alert_action),
                    onClose,
                    "battery_alert_ok",
                    palette,
                    sub = stringResource(R.string.native_ui_battery_alert_action_sub),
                )
            },
        )
    }
}

/** The other full screens: baby stopped, needs pairing, restored, no service. */
@Composable
private fun LinkNoticeContent(
    controller: AppController,
    variant: AlarmVariant,
    name: String?,
    onClose: () -> Unit,
) {
    val p = controller.parent.collectAsState().value
    val app = controller.app.collectAsState().value
    var confirmStop by remember { mutableStateOf(false) }
    val link = p.link
    val background =
        when (variant) {
            AlarmVariant.STOPPED -> Color(0xFF1F2227)
            else -> StateColors.AlertBackground
        }
    ScreenRoot("parent_link_alarm", background = background) {
        Column(Modifier.fillMaxSize()) {
            StatusBar(link, name, backgroundProtection = app.backgroundProtection)
            Column(
                Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(14.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                if (variant == AlarmVariant.RESTORED) {
                    RestoredCard(
                        p.restoredAfterMs ?: 0L,
                        p.missed,
                        {
                            controller.dismissRestored()
                            onClose()
                        },
                    )
                    return@Column
                }
                val title =
                    when (variant) {
                        AlarmVariant.STOPPED ->
                            if (name != null)
                                stringResource(R.string.native_ui_link_alarm_stopped_title, name)
                            else stringResource(R.string.native_ui_link_alarm_stopped_title_default)
                        AlarmVariant.NEEDS_PAIRING ->
                            stringResource(R.string.native_ui_link_alarm_needs_pairing_title)
                        AlarmVariant.NO_BACKGROUND ->
                            stringResource(R.string.native_ui_fgs_missing_bar)
                        else -> ""
                    }
                Text(
                    title,
                    Modifier.fillMaxWidth().testTag("link_alarm_title").semantics {
                        heading()
                        liveRegion = LiveRegionMode.Assertive
                    },
                    style = MaterialTheme.typography.displaySmall,
                    fontWeight = FontWeight.Bold,
                    color = Color.White,
                    textAlign = TextAlign.Center,
                )
                val body =
                    when (variant) {
                        AlarmVariant.STOPPED ->
                            stringResource(R.string.native_ui_link_alarm_stopped_body)
                        AlarmVariant.NEEDS_PAIRING ->
                            stringResource(R.string.core_pairing_key_mismatch)
                        AlarmVariant.NO_BACKGROUND ->
                            stringResource(R.string.native_ui_fgs_missing_body)
                        else -> null
                    }
                if (body != null) {
                    Text(
                        body,
                        Modifier.testTag("link_alarm_body"),
                        style = MaterialTheme.typography.titleMedium,
                        color = Color(0xFFFFE9E6),
                        textAlign = TextAlign.Center,
                    )
                }
                when (variant) {
                    AlarmVariant.NO_BACKGROUND -> {
                        PrimaryButton(
                            stringResource(R.string.native_ui_fgs_missing_fix),
                            { controller.startMonitoring() },
                            "link_alarm_fgs_fix",
                        )
                        QuietButton(
                            stringResource(R.string.native_ui_common_ok),
                            onClose,
                            "link_alarm_ok",
                        )
                    }
                    AlarmVariant.NEEDS_PAIRING -> {
                        PrimaryButton(
                            stringResource(R.string.native_ui_link_pair_again),
                            {
                                controller.startPairing()
                                onClose()
                            },
                            "link_alarm_pair_again",
                        )
                        QuietButton(
                            stringResource(R.string.native_ui_common_ok),
                            onClose,
                            "link_alarm_ok",
                        )
                    }
                    AlarmVariant.STOPPED -> {
                        // The baby device stopped on purpose: the primary answer ends monitoring
                        // here too, through the core (which ignores a stale tap). "OK" alone
                        // keeps monitoring and waits for the next session.
                        PrimaryButton(
                            stringResource(R.string.native_ui_link_alarm_stopped_stop_here),
                            {
                                controller.acknowledgePeerStopped()
                                onClose()
                            },
                            "link_alarm_stop_here",
                        )
                        QuietButton(
                            stringResource(R.string.native_ui_common_ok),
                            onClose,
                            "link_alarm_ok",
                        )
                    }
                    else -> {
                        PrimaryButton(
                            stringResource(R.string.native_ui_common_ok),
                            onClose,
                            "link_alarm_ok",
                        )
                        QuietButton(
                            stringResource(R.string.native_ui_parent_stop),
                            { confirmStop = true },
                            "link_alarm_stop",
                        )
                    }
                }
            }
        }
        if (confirmStop) {
            StopMonitoringDialog(
                name = name,
                onConfirm = {
                    confirmStop = false
                    controller.stopMonitoring()
                    onClose()
                },
                onDismiss = { confirmStop = false },
            )
        }
    }
}
