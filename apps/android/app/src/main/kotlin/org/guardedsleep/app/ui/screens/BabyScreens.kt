package org.guardedsleep.app.ui.screens

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.waitForUpOrCancellation
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.invisibleToUser
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import kotlinx.coroutines.withTimeoutOrNull
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.common.AdaptiveButtonRow
import org.guardedsleep.app.ui.common.AmbientBar
import org.guardedsleep.app.ui.common.AppCard
import org.guardedsleep.app.ui.common.BabyQuiet
import org.guardedsleep.app.ui.common.BatteryReading
import org.guardedsleep.app.ui.common.Body
import org.guardedsleep.app.ui.common.ConfirmDialog
import org.guardedsleep.app.ui.common.Glyph
import org.guardedsleep.app.ui.common.GlyphIcon
import org.guardedsleep.app.ui.common.KioskAttention
import org.guardedsleep.app.ui.common.KioskMark
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.common.PrimaryButton
import org.guardedsleep.app.ui.common.QuietButton
import org.guardedsleep.app.ui.common.ScreenRoot
import org.guardedsleep.app.ui.common.ScrollColumn
import org.guardedsleep.app.ui.common.SecondaryButton
import org.guardedsleep.app.ui.common.Tone
import org.guardedsleep.app.ui.common.WindowBrightness
import org.guardedsleep.app.ui.common.formatDuration
import org.guardedsleep.app.ui.common.formatTime
import org.guardedsleep.app.ui.common.kioskAttention
import org.guardedsleep.app.ui.common.kioskBrightness
import org.guardedsleep.app.ui.common.kioskMarkShape
import org.guardedsleep.app.ui.common.rememberElapsedNow
import org.guardedsleep.app.ui.common.setupConfirmRemaining
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.BabyMonitorState
import org.guardedsleep.app.ui.model.LastTalkUi
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.LinkUi
import org.guardedsleep.app.ui.model.Sensitivity
import org.guardedsleep.app.ui.theme.LocalAppColors
import org.guardedsleep.app.ui.theme.StateColors

private const val AutoLockSeconds = 30

/** The baby phone's own battery as the platform reported it; null before the first reading. */
internal fun ownBattery(b: BabyMonitorState): BatteryReading? =
    b.ownBatteryPct?.let { BatteryReading(it, b.ownPlugged) }

/** The words for the baby-side link state (S07 and the kiosk). */
internal fun babyStatusRes(link: LinkUi): Int =
    when (link.phase) {
        LinkPhase.CONNECTED -> R.string.native_ui_baby_status_connected
        LinkPhase.PEER_STOPPED -> R.string.native_ui_baby_status_parent_stopped
        LinkPhase.FAILED_NEEDS_USER -> R.string.core_pairing_key_mismatch
        LinkPhase.IDLE,
        LinkPhase.STOPPED -> R.string.native_ui_baby_status_waiting
        LinkPhase.CONNECTING,
        LinkPhase.RECONNECTING ->
            when {
                link.level >= 1 -> R.string.native_ui_baby_status_not_responding
                link.lastAliveElapsedMs == null -> R.string.native_ui_baby_status_waiting
                else -> R.string.native_ui_baby_status_reconnecting
            }
    }

/** Text for the baby-side link line (S07). */
@Composable internal fun babyStatusText(link: LinkUi): String = stringResource(babyStatusRes(link))

/**
 * The kiosk's status line: the state in words, "State · Name" when the connection has a name (a
 * label pattern, never a name inside a sentence). "Listening" while connected.
 */
@Composable
internal fun kioskStatusText(link: LinkUi, name: String?): String {
    val state =
        stringResource(
            if (link.phase == LinkPhase.CONNECTED) R.string.native_ui_kiosk_listening
            else babyStatusRes(link)
        )
    // The key-mismatch text is a whole sentence of its own; a name would only clutter it.
    return if (name == null || link.phase == LinkPhase.FAILED_NEEDS_USER) state
    else stringResource(R.string.native_ui_kiosk_with_name, state, name)
}

/** The needs-attention line under the status line. */
@Composable
internal fun kioskAttentionText(reason: KioskAttention, pct: Int): String =
    stringResource(
        when (reason) {
            KioskAttention.NOT_CHARGING -> R.string.native_ui_kiosk_attention_not_charging
            KioskAttention.BATTERY_LOW -> R.string.native_ui_kiosk_attention_battery_low
            KioskAttention.BATTERY_CRITICAL -> R.string.native_ui_kiosk_attention_battery_critical
        },
        pct.toString(),
    )

private fun linkGlyph(link: LinkUi) =
    when (link.phase) {
        LinkPhase.CONNECTED -> Glyph.LINK
        LinkPhase.PEER_STOPPED,
        LinkPhase.IDLE,
        LinkPhase.STOPPED -> Glyph.PAUSE
        LinkPhase.FAILED_NEEDS_USER -> Glyph.BROKEN_LINK
        LinkPhase.CONNECTING,
        LinkPhase.RECONNECTING -> if (link.level >= 1) Glyph.BROKEN_LINK else Glyph.REFRESH
    }

/** S07: baby phone awake (after unlock or before the first auto-lock). Dark theme only, silent. */
@Composable
fun BabyMonitorScreen(controller: AppController, onSettings: () -> Unit) {
    val b = controller.baby.collectAsState().value
    val app = controller.app.collectAsState().value
    val env = LocalUiEnv.current
    val now by rememberElapsedNow()
    var lastTouch by remember { mutableLongStateOf(env.elapsedNow()) }
    var confirmStop by remember { mutableStateOf(false) }
    val battery = ownBattery(b)
    val left = (AutoLockSeconds - (now - lastTouch) / 1000).toInt().coerceIn(0, AutoLockSeconds)
    LaunchedEffect(left, env.ticking) {
        if (env.ticking && left == 0) controller.setKioskLocked(true)
    }
    BabyQuiet {
        ScreenRoot(
            "baby_monitor",
            Modifier.pointerInput(Unit) {
                awaitPointerEventScope {
                    while (true) {
                        awaitPointerEvent(PointerEventPass.Initial)
                        lastTouch = env.elapsedNow()
                    }
                }
            },
        ) {
            ScrollColumn(verticalArrangement = Arrangement.spacedBy(14.dp)) {
                Row(
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    GlyphIcon(linkGlyph(b.link), LocalAppColors.current.secondaryText, size = 32.dp)
                    Text(
                        babyStatusText(b.link),
                        Modifier.weight(1f).testTag("baby_status"),
                        style = MaterialTheme.typography.headlineMedium,
                        fontWeight = FontWeight.Bold,
                        color = MaterialTheme.colorScheme.onBackground,
                    )
                }
                if (!app.backgroundProtection) {
                    AmbientBar(
                        Glyph.EXCLAIM,
                        stringResource(R.string.native_ui_fgs_missing_bar),
                        Tone.BAD,
                        "baby_fgs_missing",
                    )
                    Body(stringResource(R.string.native_ui_fgs_missing_body), tag = "baby_fgs_body")
                    PrimaryButton(
                        stringResource(R.string.native_ui_fgs_missing_fix),
                        { controller.startMonitoring() },
                        "baby_fgs_fix",
                    )
                }
                if (b.link.phase == LinkPhase.FAILED_NEEDS_USER) {
                    PrimaryButton(
                        stringResource(R.string.native_ui_link_pair_again),
                        { controller.startPairing() },
                        "baby_pair_again",
                    )
                }
                // The default action: lock the screen again (frame 44).
                PrimaryButton(
                    stringResource(R.string.native_ui_baby_lock),
                    { controller.setKioskLocked(true) },
                    "baby_lock",
                )
                Body(
                    stringResource(R.string.native_ui_baby_autolock_hint, left.toString()),
                    tag = "baby_autolock_hint",
                )
                Body(
                    stringResource(R.string.native_ui_baby_monitoring),
                    tag = "baby_monitoring_line",
                )
                if (b.parentListening)
                    Body(
                        stringResource(R.string.native_ui_baby_parent_listening),
                        tag = "baby_listening",
                    )
                InfoRow(
                    stringResource(R.string.native_ui_kiosk_row_parent),
                    parentDeviceValue(b, app),
                    "baby_peer",
                    labelColor = LocalAppColors.current.secondaryText,
                    valueColor = MaterialTheme.colorScheme.onBackground,
                )
                InfoRow(
                    stringResource(R.string.native_ui_kiosk_row_last_talk),
                    lastTalkValue(b.lastTalk),
                    "baby_last_talk",
                    labelColor = LocalAppColors.current.secondaryText,
                    valueColor = MaterialTheme.colorScheme.onBackground,
                )
                if (battery != null) {
                    Body(
                        stringResource(
                            if (battery.charging) R.string.native_ui_baby_battery_charging
                            else R.string.native_ui_baby_battery,
                            battery.pct.toString(),
                        ),
                        tag = "baby_battery",
                    )
                }
                Body(
                    stringResource(
                        if (b.silenceApplied == true) R.string.native_ui_baby_silence_on
                        else R.string.native_ui_baby_silence_off
                    ),
                    tag = "baby_silence",
                )
                if (b.silenceApplied == true)
                    Body(
                        stringResource(R.string.native_ui_baby_silence_everyday_phone),
                        tag = "baby_silence_everyday",
                    )
                if (!b.dndAccess)
                    Body(
                        stringResource(R.string.native_ui_baby_silence_no_access),
                        tag = "baby_silence_no_access",
                    )
                if (b.silenceRestoredNotice) {
                    AppCard(Modifier.testTag("baby_silence_restored")) {
                        Body(
                            stringResource(R.string.native_ui_baby_silence_restored),
                            color = MaterialTheme.colorScheme.onSurface,
                        )
                        QuietButton(
                            stringResource(R.string.native_ui_common_ok),
                            { controller.dismissSilenceRestoredNotice() },
                            "baby_silence_restored_ok",
                        )
                    }
                }
                if (b.micSilent) {
                    AppCard(Modifier.testTag("baby_mic_silent"), container = StateColors.Orange) {
                        Text(
                            stringResource(R.string.native_ui_baby_mic_silent),
                            color = StateColors.OnState,
                            style = MaterialTheme.typography.bodyLarge,
                        )
                    }
                }
                Body(stringResource(R.string.native_ui_baby_mic_level), tag = "baby_mic_level")
                MicLevelBar(b.micLevel, b.threshold)
                Spacer(Modifier.height(8.dp))
                AdaptiveButtonRow {
                    SecondaryButton(
                        stringResource(R.string.native_ui_common_settings),
                        onSettings,
                        "baby_settings",
                    )
                    SecondaryButton(
                        stringResource(R.string.native_ui_baby_stop),
                        { confirmStop = true },
                        "baby_stop",
                        destructive = true,
                    )
                }
                Body(
                    stringResource(R.string.native_ui_kiosk_unlocked_hint),
                    tag = "baby_unlocked_hint",
                )
            }
            if (confirmStop) {
                ConfirmDialog(
                    tag = "dialog_stop",
                    title = stringResource(R.string.native_ui_stop_title),
                    body = stringResource(R.string.native_ui_stop_baby_body),
                    confirmText = stringResource(R.string.native_ui_stop_confirm),
                    cancelText = stringResource(R.string.native_ui_stop_cancel),
                    confirmTag = "dialog_stop_confirm",
                    cancelTag = "dialog_stop_cancel",
                    onConfirm = {
                        confirmStop = false
                        controller.stopMonitoring()
                    },
                    onDismiss = { confirmStop = false },
                )
            }
        }
    }
}

/** Horizontal level bar with a threshold tick (S07, baby settings preview). */
@Composable
fun MicLevelBar(
    level: Float,
    threshold: Float,
    modifier: Modifier = Modifier,
    tag: String = "baby_mic_bar",
) {
    val colors = LocalAppColors.current
    val track = MaterialTheme.colorScheme.surfaceVariant
    Canvas(modifier.fillMaxWidth().height(20.dp).testTag(tag)) {
        drawRoundRect(
            track,
            size = size,
            cornerRadius = androidx.compose.ui.geometry.CornerRadius(10.dp.toPx()),
        )
        // Level and threshold share the slider's decibel scale (`Sensitivity.meterFraction`).
        val w = size.width * Sensitivity.meterFraction(level)
        if (w > 0f) {
            drawRoundRect(
                if (level >= threshold) colors.warn else colors.ok,
                size = Size(w, size.height),
                cornerRadius = androidx.compose.ui.geometry.CornerRadius(10.dp.toPx()),
            )
        }
        val x = size.width * Sensitivity.meterFraction(threshold)
        drawLine(colors.secondaryText, Offset(x, 0f), Offset(x, size.height), 3.dp.toPx())
    }
}

/** One "label, value" row of the touch-wake panel and the unlocked screen. */
@Composable
private fun InfoRow(
    label: String,
    value: String,
    tag: String,
    modifier: Modifier = Modifier,
    labelColor: Color = StateColors.KioskText,
    valueColor: Color = labelColor,
) {
    Row(
        modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp)
            .semantics(mergeDescendants = true) {}
            .testTag(tag),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.Top,
    ) {
        Text(label, color = labelColor, style = MaterialTheme.typography.bodyMedium)
        Text(
            value,
            Modifier.weight(1f),
            color = valueColor,
            style = MaterialTheme.typography.bodyMedium,
            fontWeight = FontWeight.SemiBold,
            textAlign = TextAlign.End,
        )
    }
}

/** The parent device's name: what it sent, else the saved one, else the plain words. */
@Composable
private fun parentDeviceValue(b: BabyMonitorState, app: AppState): String =
    b.parentName?.takeIf { it.isNotEmpty() }
        ?: app.peerName?.takeIf { it.isNotEmpty() }
        ?: stringResource(R.string.native_ui_common_parent_unit_default)

@Composable
private fun lastTalkValue(last: LastTalkUi?): String =
    if (last == null) stringResource(R.string.native_ui_kiosk_last_talk_none)
    else
        stringResource(
            R.string.native_ui_kiosk_last_talk_value,
            formatTime(last.atWallMs),
            formatDuration(last.durationMs),
        )

@Composable
private fun thisDeviceValue(battery: BatteryReading): String =
    stringResource(
        if (battery.charging) R.string.native_ui_kiosk_device_charging
        else R.string.native_ui_kiosk_device_battery,
        battery.pct.toString(),
    )

/** How long a touch-wake stays up before the screen goes back to minimum brightness. */
private const val KioskWakeMs = 6000L

/**
 * S08: black locked screen at minimum brightness. A dim status line and one mark along the top edge
 * (a short bar, a dashed edge or a pulsing frame) are always visible; a touch wakes a dim panel and
 * hold-and-slide-up unlocks. The set-up confirmation shows for 60 s from the first contact. During
 * a talk only the talk indicator shows. No sound, no haptics, no flashes.
 */
@Composable
fun BabyKioskScreen(controller: AppController) {
    val kioskApp = controller.app.collectAsState().value
    val b0 = controller.baby.collectAsState().value
    // No background service: the calm dim frame of level 2 (never sound), instead of the bar.
    val b =
        if (!kioskApp.backgroundProtection)
            b0.copy(link = b0.link.copy(phase = LinkPhase.RECONNECTING, level = 2))
        else b0
    val env = LocalUiEnv.current
    val now by rememberElapsedNow()
    val battery = ownBattery(b)
    val attention = kioskAttention(battery, b.chargerSeen)
    var woken by remember { mutableStateOf(false) }
    var wakeKey by remember { mutableIntStateOf(0) }
    var unlockProgress by remember { mutableFloatStateOf(0f) }
    var unlocking by remember { mutableStateOf(false) }
    var confirmUnlock by remember { mutableStateOf(false) }
    val unlockPx = with(LocalDensity.current) { 140.dp.toPx() }
    val onUnlock = rememberUpdatedState { controller.setKioskLocked(false) }
    LaunchedEffect(wakeKey) {
        if (woken && env.ticking) {
            delay(KioskWakeMs)
            woken = false
        }
    }
    val talking = b.parentTalking
    val confirmLeft = setupConfirmRemaining(b.firstContactElapsedMs, now)
    val touched = woken || unlocking || confirmUnlock
    val awake = touched || confirmLeft != null
    // The minimum, except the confirmation and a touch-wake; a talk keeps the minimum.
    WindowBrightness(kioskBrightness(awake, talking))
    val fadeSpec = if (env.reduceMotion) snap<Float>() else tween(1200)
    val showDetails = awake && !talking
    val infoAlpha by animateFloatAsState(if (showDetails) 1f else 0f, fadeSpec)
    val a11y = stringResource(R.string.native_ui_kiosk_a11y_label)
    val unlockLabel = stringResource(R.string.native_ui_kiosk_unlock_action)
    val connection = kioskApp.connections.firstOrNull { it.selected }?.name
    val text = StateColors.KioskText
    BabyQuiet {
        ScreenRoot(
            "baby_kiosk",
            Modifier.semantics {
                    contentDescription = a11y
                    // A screen reader cannot hold and slide, so its action asks first: a stray
                    // double tap must not open the controls.
                    customActions =
                        listOf(
                            CustomAccessibilityAction(unlockLabel) {
                                woken = true
                                wakeKey++
                                confirmUnlock = true
                                true
                            }
                        )
                }
                .pointerInput(Unit) {
                    awaitEachGesture {
                        val down = awaitFirstDown()
                        woken = true
                        wakeKey++
                        val early = withTimeoutOrNull(500) { waitForUpOrCancellation() }
                        if (early != null || currentEvent.changes.none { it.pressed })
                            return@awaitEachGesture
                        unlocking = true
                        while (true) {
                            val c = awaitPointerEvent().changes.firstOrNull() ?: break
                            if (!c.pressed) break
                            unlockProgress =
                                ((down.position.y - c.position.y) / unlockPx).coerceIn(0f, 1f)
                            if (unlockProgress >= 1f) {
                                onUnlock.value()
                                break
                            }
                            c.consume()
                        }
                        unlocking = false
                        unlockProgress = 0f
                    }
                },
            background = StateColors.KioskBlack,
        ) {
            if (talking) {
                // Static and dim and alone: it never wakes the screen, pulses or animates
                // (`talk-back` "Baby device shows who talks"); the status line and the mark hide.
                Row(
                    Modifier.align(Alignment.TopCenter)
                        .padding(top = 12.dp)
                        .testTag("baby_talking"),
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    GlyphIcon(Glyph.MIC, text, size = 14.dp)
                    Text(
                        stringResource(R.string.native_ui_talk_baby_talking),
                        style = MaterialTheme.typography.bodySmall,
                        color = text,
                    )
                }
            } else {
                KioskMark(kioskMarkShape(b.link), attention != null, env.reduceMotion)
                // The always-visible line; the touch-wake panel replaces it while it shows.
                Column(
                    Modifier.align(Alignment.BottomCenter)
                        .fillMaxWidth()
                        .padding(start = 20.dp, end = 20.dp, bottom = 44.dp)
                        .alpha(1f - infoAlpha)
                        .then(
                            if (showDetails) Modifier.semantics { invisibleToUser() } else Modifier
                        ),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        GlyphIcon(
                            linkGlyph(b.link),
                            text,
                            Modifier.testTag("baby_kiosk_link_icon"),
                            size = 20.dp,
                        )
                        Text(
                            kioskStatusText(b.link, connection),
                            Modifier.testTag("baby_status"),
                            style = MaterialTheme.typography.titleSmall,
                            fontWeight = FontWeight.SemiBold,
                            color = text,
                            textAlign = TextAlign.Center,
                        )
                    }
                    if (attention != null && battery != null) {
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            GlyphIcon(Glyph.BATTERY, text, size = 20.dp)
                            Text(
                                kioskAttentionText(attention, battery.pct),
                                Modifier.testTag("baby_kiosk_attention"),
                                style = MaterialTheme.typography.titleSmall,
                                fontWeight = FontWeight.SemiBold,
                                color = text,
                                textAlign = TextAlign.Center,
                            )
                        }
                    }
                    Text(
                        stringResource(R.string.native_ui_kiosk_touch_hint),
                        Modifier.testTag("baby_kiosk_touch_hint"),
                        style = MaterialTheme.typography.bodyMedium,
                        color = text,
                    )
                }
            }
            val lastAlive = b.link.lastAliveElapsedMs
            val connected = b.link.phase == LinkPhase.CONNECTED
            Column(
                Modifier.fillMaxSize()
                    .alpha(infoAlpha)
                    .padding(horizontal = 24.dp)
                    .padding(top = 56.dp, bottom = 24.dp)
                    .then(if (showDetails) Modifier else Modifier.semantics { invisibleToUser() }),
                verticalArrangement = Arrangement.spacedBy(8.dp, Alignment.Top),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                GlyphIcon(
                    if (connected) Glyph.CHECK else linkGlyph(b.link),
                    text,
                    Modifier.testTag("baby_kiosk_detail_icon"),
                    size = 40.dp,
                )
                // Connected: the set-up confirmation, naming the parent device; otherwise the
                // state.
                val parentName = b.parentName?.takeIf { it.isNotEmpty() }
                Text(
                    if (connected) {
                        if (parentName != null)
                            stringResource(R.string.native_ui_kiosk_connected_to, parentName)
                        else stringResource(R.string.native_ui_kiosk_connected_to_default)
                    } else babyStatusText(b.link),
                    Modifier.testTag("baby_kiosk_headline"),
                    style = MaterialTheme.typography.headlineSmall,
                    fontWeight = FontWeight.SemiBold,
                    color = text,
                    textAlign = TextAlign.Center,
                )
                if (connected) {
                    Text(
                        kioskStatusText(b.link, connection),
                        Modifier.testTag("baby_kiosk_substatus"),
                        style = MaterialTheme.typography.titleMedium,
                        color = text,
                        textAlign = TextAlign.Center,
                    )
                }
                if (
                    b.link.level >= 2 &&
                        lastAlive != null &&
                        b.link.phase != LinkPhase.PEER_STOPPED &&
                        !connected
                ) {
                    Text(
                        stringResource(
                            R.string.native_ui_kiosk_lost_body,
                            formatDuration(env.elapsedNow() - lastAlive),
                        ),
                        Modifier.testTag("baby_kiosk_lost_body"),
                        style = MaterialTheme.typography.bodyMedium,
                        color = text,
                        textAlign = TextAlign.Center,
                    )
                }
                // The countdown is plain text, never a live region: it must not talk at night.
                if (confirmLeft != null) {
                    Text(
                        stringResource(R.string.native_ui_kiosk_dims_in, confirmLeft.toString()),
                        Modifier.testTag("baby_kiosk_dims_in"),
                        style = MaterialTheme.typography.bodyMedium,
                        color = text,
                        textAlign = TextAlign.Center,
                    )
                }
                if (touched) {
                    Spacer(Modifier.height(8.dp))
                    HorizontalDivider(color = text.copy(alpha = 0.3f))
                    InfoRow(
                        stringResource(R.string.native_ui_kiosk_row_connection),
                        if (connection != null)
                            stringResource(R.string.native_ui_kiosk_connection_value, connection)
                        else stringResource(R.string.native_ui_common_baby_unit_default),
                        "baby_row_connection",
                    )
                    InfoRow(
                        stringResource(R.string.native_ui_kiosk_row_parent),
                        parentDeviceValue(b, kioskApp),
                        "baby_peer",
                    )
                    InfoRow(
                        stringResource(R.string.native_ui_kiosk_row_last_talk),
                        lastTalkValue(b.lastTalk),
                        "baby_last_talk",
                    )
                    if (battery != null) {
                        InfoRow(
                            stringResource(R.string.native_ui_kiosk_row_this_device),
                            thisDeviceValue(battery),
                            "baby_kiosk_battery",
                        )
                    }
                    if (b.parentListening) {
                        Text(
                            stringResource(R.string.native_ui_baby_parent_listening),
                            Modifier.testTag("baby_listening"),
                            style = MaterialTheme.typography.bodyMedium,
                            color = text,
                        )
                    }
                    Spacer(Modifier.weight(1f))
                    Text(
                        stringResource(R.string.native_ui_kiosk_unlock_hint),
                        Modifier.testTag("baby_kiosk_hint"),
                        style = MaterialTheme.typography.bodyMedium,
                        color = text,
                        textAlign = TextAlign.Center,
                    )
                }
            }
            if (unlocking) {
                Box(
                    Modifier.align(Alignment.BottomCenter)
                        .padding(bottom = 48.dp)
                        .size(width = 160.dp, height = 56.dp)
                        .background(
                            lerp(Color(0xFF1A1A1A), Color(0xFF3A3A3A), unlockProgress),
                            RoundedCornerShape(28.dp),
                        )
                        .testTag("baby_kiosk_unlock_zone"),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        stringResource(R.string.native_ui_kiosk_unlock_zone),
                        color = text,
                        style = MaterialTheme.typography.bodyMedium,
                    )
                }
            }
            if (confirmUnlock) {
                ConfirmDialog(
                    tag = "dialog_unlock",
                    title = stringResource(R.string.native_ui_kiosk_unlock_confirm_title),
                    body = stringResource(R.string.native_ui_kiosk_unlock_confirm_body),
                    confirmText = stringResource(R.string.native_ui_kiosk_unlock_confirm),
                    cancelText = stringResource(R.string.native_ui_kiosk_unlock_cancel),
                    confirmTag = "dialog_unlock_confirm",
                    cancelTag = "dialog_unlock_cancel",
                    onConfirm = {
                        confirmUnlock = false
                        onUnlock.value()
                    },
                    onDismiss = { confirmUnlock = false },
                    destructive = false,
                )
            }
        }
    }
}
