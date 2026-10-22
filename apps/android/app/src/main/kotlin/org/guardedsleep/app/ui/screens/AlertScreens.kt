package org.guardedsleep.app.ui.screens

import android.content.Intent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import org.guardedsleep.app.R
import org.guardedsleep.app.ui.common.AdaptiveButtonRow
import org.guardedsleep.app.ui.common.Body
import org.guardedsleep.app.ui.common.ConfirmDialog
import org.guardedsleep.app.ui.common.LinkBar
import org.guardedsleep.app.ui.common.ListenButton
import org.guardedsleep.app.ui.common.PrimaryButton
import org.guardedsleep.app.ui.common.QuietButton
import org.guardedsleep.app.ui.common.ScreenRoot
import org.guardedsleep.app.ui.common.SecondaryButton
import org.guardedsleep.app.ui.common.SoundMeter
import org.guardedsleep.app.ui.common.formatClock
import org.guardedsleep.app.ui.common.formatTime
import org.guardedsleep.app.ui.common.pulseAlpha
import org.guardedsleep.app.ui.common.rememberElapsedNow
import org.guardedsleep.app.ui.common.rememberWallNow
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.CauseHint
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.theme.StateColors

private val AlertRed = Color(0xFFB3261E)

/** S10: full-screen noise alert. Dark red, never white; the connection bar stays on top. */
@Composable
fun NoiseAlertContent(controller: AppController, onClose: () -> Unit) {
    val p = controller.parent.collectAsState().value
    val app = controller.app.collectAsState().value
    val settings = controller.settings.collectAsState().value
    val wall by rememberWallNow()
    val ctx = LocalContext.current
    var soundStopped by remember { mutableStateOf(false) }
    val ended = !p.noiseAlertActive
    ScreenRoot("parent_noise_alert", background = StateColors.AlertBackground) {
        Column(Modifier.fillMaxSize()) {
            LinkBar(p.link, customPeerName(app.peerName))
            Column(
                Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
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
                Text(
                    stringResource(R.string.native_ui_alert_title),
                    Modifier.fillMaxWidth().testTag("alert_title").semantics {
                        heading()
                        liveRegion = LiveRegionMode.Assertive
                    },
                    style = MaterialTheme.typography.displaySmall,
                    fontWeight = FontWeight.Bold,
                    color = Color.White,
                    textAlign = TextAlign.Center,
                )
                Text(
                    stringResource(
                        if (ended) R.string.native_ui_alert_ended
                        else R.string.native_ui_alert_since,
                        formatTime(p.noiseSinceWallMs ?: wall),
                    ),
                    Modifier.testTag("alert_since"),
                    style = MaterialTheme.typography.titleLarge,
                    color = Color(0xFFFFD9D4),
                )
                SoundMeter(
                    p.level,
                    p.babyThreshold ?: settings.threshold,
                    live = true,
                    alarm = true,
                    tag = "alert_meter",
                )
                if (ended) {
                    PrimaryButton(
                        stringResource(R.string.native_ui_common_ok),
                        onClose,
                        "alert_close",
                        container = AlertRed,
                        content = Color.White,
                    )
                } else if (soundStopped) {
                    Body(
                        stringResource(R.string.native_ui_alert_sound_stopped),
                        tag = "alert_sound_stopped",
                        color = Color(0xFFFFD9D4),
                    )
                } else {
                    PrimaryButton(
                        stringResource(R.string.native_ui_alert_stop_sound),
                        {
                            soundStopped = true
                            controller.stopAlertSound()
                        },
                        "alert_stop_sound",
                        container = AlertRed,
                        content = Color.White,
                    )
                }
                AdaptiveButtonRow {
                    SecondaryButton(
                        stringResource(R.string.native_ui_snooze_minutes, "10"),
                        {
                            controller.snooze(10)
                            onClose()
                        },
                        "alert_snooze_10",
                    )
                    SecondaryButton(
                        stringResource(R.string.native_ui_snooze_minutes, "30"),
                        {
                            controller.snooze(30)
                            onClose()
                        },
                        "alert_snooze_30",
                    )
                }
                Spacer(Modifier.height(8.dp))
                ListenButton(
                    p.listen,
                    p.link.phase == LinkPhase.CONNECTED,
                    { controller.setListen(it) },
                    tag = "alert_listen",
                )
                Spacer(Modifier.height(8.dp))
                QuietButton(
                    stringResource(R.string.native_ui_alert_open_app),
                    {
                        ctx.packageManager.getLaunchIntentForPackage(ctx.packageName)?.let {
                            ctx.startActivity(Intent(it).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                        }
                        onClose()
                    },
                    "alert_open_app",
                )
            }
        }
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
    var help by remember { mutableStateOf(false) }
    var confirmStop by remember { mutableStateOf(false) }
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
                battery < 5 &&
                p.peer.charging != true -> AlarmVariant.BATTERY
            link.phase == LinkPhase.CONNECTED -> AlarmVariant.RESTORED
            else -> AlarmVariant.LOST
        }
    LaunchedEffect(variant) {
        if (variant == AlarmVariant.RESTORED) {
            delay(10_000)
            onClose()
        }
    }
    val acknowledged = (link.acknowledgedUntilElapsedMs ?: 0L) > elapsed
    val urgent = variant == AlarmVariant.LOST && link.level >= 3
    val headingAlpha = if (urgent) pulseAlpha(0.55f, 1f, 1600) else 1f
    val since = link.lastAliveElapsedMs?.let { elapsed - it } ?: 0L
    val background =
        when (variant) {
            AlarmVariant.RESTORED -> Color(0xFF0F3320)
            AlarmVariant.STOPPED -> Color(0xFF1F2227)
            AlarmVariant.LOST -> if (urgent) Color(0xFF5C0D0D) else StateColors.AlertBackground
            else -> StateColors.AlertBackground
        }
    ScreenRoot("parent_link_alarm", background = background) {
        Column(Modifier.fillMaxSize()) {
            LinkBar(
                link,
                name,
                tag = "link_alarm_bar",
                backgroundProtection = app.backgroundProtection,
            )
            Column(
                Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(14.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                val title =
                    when (variant) {
                        AlarmVariant.LOST ->
                            when {
                                link.level >= 3 && name != null ->
                                    stringResource(
                                        R.string.native_ui_link_alarm_lost_title_urgent,
                                        name,
                                    )
                                link.level >= 3 ->
                                    stringResource(
                                        R.string.native_ui_link_alarm_lost_title_urgent_default
                                    )
                                name != null ->
                                    stringResource(R.string.native_ui_link_alarm_lost_title, name)
                                else ->
                                    stringResource(R.string.native_ui_link_alarm_lost_title_default)
                            }
                        AlarmVariant.STOPPED ->
                            if (name != null)
                                stringResource(R.string.native_ui_link_alarm_stopped_title, name)
                            else stringResource(R.string.native_ui_link_alarm_stopped_title_default)
                        AlarmVariant.BATTERY ->
                            stringResource(R.string.native_ui_link_alarm_battery_title)
                        AlarmVariant.NEEDS_PAIRING ->
                            stringResource(R.string.native_ui_link_alarm_needs_pairing_title)
                        AlarmVariant.RESTORED ->
                            stringResource(R.string.native_ui_link_alarm_restored_title)
                        AlarmVariant.NO_BACKGROUND ->
                            stringResource(R.string.native_ui_fgs_missing_bar)
                    }
                Text(
                    title,
                    Modifier.fillMaxWidth()
                        .alpha(headingAlpha)
                        .testTag("link_alarm_title")
                        .semantics {
                            heading()
                            liveRegion = LiveRegionMode.Assertive
                        },
                    style = MaterialTheme.typography.displaySmall,
                    fontWeight = FontWeight.Bold,
                    color = Color.White,
                    textAlign = TextAlign.Center,
                )
                if (variant == AlarmVariant.LOST) {
                    Text(
                        formatClock(since),
                        Modifier.testTag("link_alarm_since"),
                        fontSize = 56.sp,
                        fontFamily = FontFamily.Monospace,
                        fontWeight = FontWeight.Bold,
                        color = Color(0xFFFFD9D4),
                    )
                }
                val body =
                    when (variant) {
                        AlarmVariant.LOST -> stringResource(R.string.native_ui_link_alarm_lost_body)
                        AlarmVariant.STOPPED ->
                            stringResource(R.string.native_ui_link_alarm_stopped_body)
                        AlarmVariant.BATTERY ->
                            stringResource(R.string.native_ui_link_alarm_battery_body)
                        AlarmVariant.NEEDS_PAIRING ->
                            stringResource(R.string.native_ui_link_needs_pairing)
                        AlarmVariant.RESTORED -> null
                        AlarmVariant.NO_BACKGROUND ->
                            stringResource(R.string.native_ui_fgs_missing_body)
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
                if (variant == AlarmVariant.LOST && link.causeHint != null) {
                    Text(
                        stringResource(
                            when (link.causeHint) {
                                CauseHint.SERVER_UNREACHABLE ->
                                    R.string.native_ui_link_alarm_cause_server
                                CauseHint.PEER_ABSENT -> R.string.native_ui_link_alarm_cause_peer
                                CauseHint.MEDIA_BLOCKED -> R.string.native_ui_link_alarm_cause_media
                            }
                        ),
                        Modifier.testTag("link_alarm_cause"),
                        style = MaterialTheme.typography.bodyLarge,
                        color = Color(0xFFFFD9D4),
                        textAlign = TextAlign.Center,
                    )
                }
                if (variant == AlarmVariant.LOST && link.stalePairingHint) {
                    Text(
                        stringResource(R.string.native_ui_link_stale_pairing_hint),
                        Modifier.testTag("link_alarm_stale_hint"),
                        style = MaterialTheme.typography.bodyLarge,
                        color = Color(0xFFFFD9D4),
                        textAlign = TextAlign.Center,
                    )
                }
                when (variant) {
                    AlarmVariant.LOST -> {
                        if (acknowledged) {
                            Body(
                                stringResource(
                                    R.string.native_ui_link_alarm_ack_note,
                                    formatClock((link.acknowledgedUntilElapsedMs ?: 0L) - elapsed),
                                ),
                                tag = "link_alarm_ack_note",
                                color = Color(0xFFFFD9D4),
                            )
                        } else {
                            Body(
                                stringResource(
                                    if (urgent) R.string.native_ui_link_alarm_sound_note_urgent
                                    else R.string.native_ui_link_alarm_sound_note
                                ),
                                tag = "link_alarm_sound_note",
                                color = Color(0xFFFFD9D4),
                            )
                            PrimaryButton(
                                stringResource(R.string.native_ui_link_alarm_ack),
                                { controller.acknowledgeAlarm() },
                                "link_alarm_ack",
                                container = AlertRed,
                                content = Color.White,
                            )
                        }
                        SecondaryButton(
                            stringResource(R.string.native_ui_link_try_now),
                            { controller.tryReconnectNow() },
                            "link_alarm_try_now",
                            Modifier.fillMaxWidth(),
                        )
                        QuietButton(
                            stringResource(R.string.native_ui_help_reconnect_open),
                            { help = true },
                            "link_alarm_help",
                        )
                        Spacer(Modifier.height(16.dp))
                        SecondaryButton(
                            stringResource(R.string.native_ui_parent_stop),
                            { confirmStop = true },
                            "link_alarm_stop",
                            destructive = true,
                        )
                    }
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
                    else ->
                        PrimaryButton(
                            stringResource(R.string.native_ui_common_ok),
                            onClose,
                            "link_alarm_ok",
                        )
                }
            }
        }
        if (help) HelpSheet { help = false }
        if (confirmStop) {
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
                    confirmStop = false
                    controller.stopMonitoring()
                    onClose()
                },
                onDismiss = { confirmStop = false },
            )
        }
    }
}
