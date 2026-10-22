package org.guardedsleep.app.ui.screens

import android.content.Intent
import android.content.IntentFilter
import android.os.BatteryManager
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
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
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.common.PrimaryButton
import org.guardedsleep.app.ui.common.QuietButton
import org.guardedsleep.app.ui.common.ScreenRoot
import org.guardedsleep.app.ui.common.ScrollColumn
import org.guardedsleep.app.ui.common.SecondaryButton
import org.guardedsleep.app.ui.common.Tone
import org.guardedsleep.app.ui.common.WindowBrightness
import org.guardedsleep.app.ui.common.formatDuration
import org.guardedsleep.app.ui.common.rememberElapsedNow
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.LinkUi
import org.guardedsleep.app.ui.theme.LocalAppColors
import org.guardedsleep.app.ui.theme.StateColors

private const val AutoLockSeconds = 30

/** Baby phone's own battery (shown on S07 and on touch-wake only). */
@Composable
internal fun rememberOwnBattery(): BatteryReading? {
    val env = LocalUiEnv.current
    val ctx = LocalContext.current
    val now by rememberElapsedNow()
    return env.battery
        ?: remember(now / 30_000) {
            val i = ctx.registerReceiver(null, IntentFilter(Intent.ACTION_BATTERY_CHANGED))
            val level = i?.getIntExtra(BatteryManager.EXTRA_LEVEL, -1) ?: -1
            val scale = i?.getIntExtra(BatteryManager.EXTRA_SCALE, 100) ?: 100
            val status = i?.getIntExtra(BatteryManager.EXTRA_STATUS, -1) ?: -1
            if (level < 0) null
            else
                BatteryReading(
                    level * 100 / scale,
                    status == BatteryManager.BATTERY_STATUS_CHARGING ||
                        status == BatteryManager.BATTERY_STATUS_FULL,
                )
        }
}

/** Text for the baby-side link line (S07 and kiosk touch-wake). */
@Composable
internal fun babyStatusText(link: LinkUi): String =
    stringResource(
        when (link.phase) {
            LinkPhase.CONNECTED -> R.string.native_ui_baby_status_connected
            LinkPhase.PEER_STOPPED -> R.string.native_ui_baby_status_parent_stopped
            LinkPhase.FAILED_NEEDS_USER -> R.string.native_ui_link_needs_pairing
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
    )

private fun edgeColor(link: LinkUi): Color =
    when (link.phase) {
        LinkPhase.CONNECTED -> StateColors.KioskGreen
        LinkPhase.PEER_STOPPED,
        LinkPhase.IDLE,
        LinkPhase.STOPPED -> StateColors.KioskGrey
        LinkPhase.FAILED_NEEDS_USER -> Color(0xFF4A0A0A)
        LinkPhase.CONNECTING,
        LinkPhase.RECONNECTING ->
            when {
                link.level >= 2 -> Color(0xFF4A0A0A)
                link.level == 1 -> StateColors.KioskOrange
                else -> StateColors.KioskYellow
            }
    }

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
    val battery = rememberOwnBattery()
    val left = (AutoLockSeconds - (now - lastTouch) / 1000).toInt().coerceIn(0, AutoLockSeconds)
    LaunchedEffect(left, env.ticking) {
        if (env.ticking && left == 0) controller.setKioskLocked(true)
    }
    val peer = app.peerName ?: stringResource(R.string.native_ui_common_parent_unit_default)
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
                Body(
                    stringResource(R.string.native_ui_baby_monitoring),
                    tag = "baby_monitoring_line",
                )
                if (b.parentListening)
                    Body(
                        stringResource(R.string.native_ui_baby_parent_listening),
                        tag = "baby_listening",
                    )
                Body(peer, tag = "baby_peer")
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
                PrimaryButton(
                    stringResource(R.string.native_ui_baby_lock),
                    { controller.setKioskLocked(true) },
                    "baby_lock",
                )
                Body(
                    stringResource(R.string.native_ui_baby_autolock_hint, left.toString()),
                    tag = "baby_autolock_hint",
                )
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
        val w = size.width * level.coerceIn(0f, 1f)
        if (w > 0f) {
            drawRoundRect(
                if (level >= threshold) colors.warn else colors.ok,
                size = Size(w, size.height),
                cornerRadius = androidx.compose.ui.geometry.CornerRadius(10.dp.toPx()),
            )
        }
        val x = size.width * threshold.coerceIn(0f, 1f)
        drawLine(colors.secondaryText, Offset(x, 0f), Offset(x, size.height), 3.dp.toPx())
    }
}

/**
 * S08: near-black locked screen. Touch wakes dim status for 3 s; hold 0.5 s and slide up to unlock.
 * Level >= 2 shows a slow dim red pulse (peak #330000, 10 s period). No sound, no haptics, no
 * flashes.
 */
@Composable
fun BabyKioskScreen(controller: AppController) {
    val kioskApp = controller.app.collectAsState().value
    val b0 = controller.baby.collectAsState().value
    // No background service: the calm dim red pulse of level 2 (never sound), instead of a green
    // edge.
    val b =
        if (!kioskApp.backgroundProtection)
            b0.copy(link = b0.link.copy(phase = LinkPhase.RECONNECTING, level = 2))
        else b0
    val env = LocalUiEnv.current
    val battery = rememberOwnBattery()
    var woken by remember { mutableStateOf(false) }
    var wakeKey by remember { mutableIntStateOf(0) }
    var unlockProgress by remember { mutableFloatStateOf(0f) }
    var unlocking by remember { mutableStateOf(false) }
    val unlockPx = with(LocalDensity.current) { 140.dp.toPx() }
    val onUnlock = rememberUpdatedState { controller.setKioskLocked(false) }
    LaunchedEffect(wakeKey) {
        if (woken && env.ticking) {
            delay(3000)
            woken = false
        }
    }
    WindowBrightness(if (woken || unlocking) 0.1f else 0.01f)
    val fadeSpec = if (env.reduceMotion) snap<Float>() else tween(1200)
    val infoAlpha by animateFloatAsState(if (woken || unlocking) 1f else 0f, fadeSpec)
    val pulsing =
        b.link.level >= 2 &&
            b.link.phase != LinkPhase.PEER_STOPPED &&
            b.link.phase != LinkPhase.CONNECTED &&
            b.link.phase != LinkPhase.IDLE &&
            b.link.phase != LinkPhase.STOPPED
    val pulse: Float =
        if (!pulsing) 0f
        else if (env.reduceMotion) 0.6f
        else {
            val t = rememberInfiniteTransition()
            val v by
                t.animateFloat(
                    0f,
                    1f,
                    infiniteRepeatable(
                        tween(5000, easing = CubicBezierEasing(0.37f, 0f, 0.63f, 1f)),
                        RepeatMode.Reverse,
                    ),
                )
            v
        }
    val a11y = stringResource(R.string.native_ui_kiosk_a11y_label)
    val unlockLabel = stringResource(R.string.native_ui_kiosk_unlock_action)
    BabyQuiet {
        ScreenRoot(
            "baby_kiosk",
            Modifier.semantics {
                    contentDescription = a11y
                    customActions =
                        listOf(
                            CustomAccessibilityAction(unlockLabel) {
                                onUnlock.value()
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
            background = lerp(StateColors.KioskBlack, StateColors.KioskPulsePeak, pulse),
        ) {
            Box(
                Modifier.fillMaxWidth()
                    .height(4.dp)
                    .background(edgeColor(b.link))
                    .testTag("baby_kiosk_edge")
            )
            Column(
                Modifier.fillMaxSize().alpha(infoAlpha).padding(24.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp, Alignment.CenterVertically),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                GlyphIcon(
                    linkGlyph(b.link),
                    StateColors.KioskText,
                    Modifier.testTag("baby_kiosk_link_icon"),
                    size = 12.dp,
                )
                val lastAlive = b.link.lastAliveElapsedMs
                Text(
                    babyStatusText(b.link) +
                        if (
                            b.link.level >= 2 &&
                                lastAlive != null &&
                                b.link.phase != LinkPhase.PEER_STOPPED &&
                                b.link.phase != LinkPhase.CONNECTED
                        ) {
                            " " +
                                stringResource(
                                    R.string.native_ui_baby_for_duration,
                                    formatDuration(env.elapsedNow() - lastAlive),
                                )
                        } else "",
                    Modifier.testTag("baby_status"),
                    style = MaterialTheme.typography.titleMedium,
                    color = StateColors.KioskText,
                    textAlign = TextAlign.Center,
                )
                if (battery != null) {
                    Text(
                        stringResource(
                            if (battery.charging) R.string.native_ui_baby_battery_charging
                            else R.string.native_ui_baby_battery,
                            battery.pct.toString(),
                        ),
                        Modifier.testTag("baby_kiosk_battery"),
                        style = MaterialTheme.typography.bodyMedium,
                        color = StateColors.KioskText,
                    )
                }
                if (b.parentListening) {
                    Text(
                        stringResource(R.string.native_ui_baby_parent_listening),
                        Modifier.testTag("baby_listening"),
                        style = MaterialTheme.typography.bodyMedium,
                        color = StateColors.KioskText,
                    )
                }
                Text(
                    stringResource(R.string.native_ui_kiosk_unlock_hint),
                    Modifier.testTag("baby_kiosk_hint"),
                    style = MaterialTheme.typography.bodyMedium,
                    color = StateColors.KioskText,
                    textAlign = TextAlign.Center,
                )
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
                        color = StateColors.KioskText,
                        style = MaterialTheme.typography.bodyMedium,
                    )
                }
            }
        }
    }
}
