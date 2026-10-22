package org.guardedsleep.app.ui.common

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.waitForUpOrCancellation
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.withTimeoutOrNull
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.LinkUi
import org.guardedsleep.app.ui.model.ListenMode
import org.guardedsleep.app.ui.theme.LocalAppColors
import org.guardedsleep.app.ui.theme.StateColors

/** Visual spec of the connection bar for one link state (alpha-ux-flows §3.6, §6.1). */
private data class BarSpec(
    val glyph: Glyph,
    val extra: Glyph?,
    val tone: Tone,
    val text: String,
    val assertive: Boolean,
    val pulse: Boolean,
)

/**
 * Parent ambient bar: never hidden while monitoring; colour + glyph + text for every phase and
 * level.
 */
@Composable
fun LinkBar(
    link: LinkUi,
    name: String?,
    modifier: Modifier = Modifier,
    tag: String = "parent_ambient_bar",
    backgroundProtection: Boolean = true,
) {
    val now by rememberElapsedNow()
    val sinceMs = link.lastAliveElapsedMs?.let { now - it } ?: 0L
    // A custom unit name is spliced in; without one the copy says "the baby unit" with correct
    // grammar.
    val connected =
        if (name != null) stringResource(R.string.native_ui_link_connected, name)
        else stringResource(R.string.native_ui_link_connected_default)
    val connecting =
        if (name != null) stringResource(R.string.native_ui_link_connecting, name)
        else stringResource(R.string.native_ui_link_connecting_default)
    val reconnecting = stringResource(R.string.native_ui_link_reconnecting)
    val cantReach =
        if (name != null) stringResource(R.string.native_ui_link_cant_reach, name)
        else stringResource(R.string.native_ui_link_cant_reach_default)
    val lostFor = stringResource(R.string.native_ui_link_lost_for, formatDuration(sinceMs))
    val stopped =
        if (name != null) stringResource(R.string.native_ui_link_baby_stopped, name)
        else stringResource(R.string.native_ui_link_baby_stopped_default)
    val needsPairing =
        stringResource(
            if (link.failureKey == "update_required") R.string.native_ui_link_update_required
            else R.string.native_ui_link_needs_pairing
        )
    val idle = stringResource(R.string.native_ui_link_idle)
    val noBackground = stringResource(R.string.native_ui_fgs_missing_bar)
    val spec =
        if (!backgroundProtection)
            BarSpec(Glyph.BROKEN_LINK, Glyph.EXCLAIM, Tone.BAD, noBackground, true, true)
        else
            when (link.phase) {
                LinkPhase.CONNECTED -> BarSpec(Glyph.LINK, null, Tone.OK, connected, false, false)
                LinkPhase.PEER_STOPPED ->
                    BarSpec(Glyph.PAUSE, null, Tone.NEUTRAL, stopped, true, false)
                LinkPhase.FAILED_NEEDS_USER ->
                    BarSpec(Glyph.BROKEN_LINK, Glyph.EXCLAIM, Tone.BAD, needsPairing, true, false)
                LinkPhase.IDLE,
                LinkPhase.STOPPED -> BarSpec(Glyph.PAUSE, null, Tone.NEUTRAL, idle, false, false)
                LinkPhase.CONNECTING,
                LinkPhase.RECONNECTING ->
                    when {
                        link.level >= 2 ->
                            BarSpec(Glyph.BROKEN_LINK, Glyph.EXCLAIM, Tone.BAD, lostFor, true, true)
                        link.level == 1 ->
                            BarSpec(Glyph.BROKEN_LINK, null, Tone.WARN, cantReach, false, false)
                        link.phase == LinkPhase.CONNECTING && link.lastAliveElapsedMs == null ->
                            BarSpec(Glyph.REFRESH, null, Tone.INFO, connecting, false, false)
                        else -> BarSpec(Glyph.REFRESH, null, Tone.INFO, reconnecting, false, false)
                    }
            }
    val alpha = if (spec.pulse) pulseAlpha(0.8f, 1f, 2500) else 1f
    AmbientBar(
        spec.glyph,
        spec.text,
        spec.tone,
        tag,
        modifier.alpha(alpha),
        spec.assertive,
        spec.extra,
    )
}

/** Slow alpha pulse; static when the user (or a test) asks for reduced motion. */
@Composable
fun pulseAlpha(from: Float, to: Float, periodMs: Int): Float {
    if (LocalUiEnv.current.reduceMotion) return to
    val t = rememberInfiniteTransition()
    val v by
        t.animateFloat(
            from,
            to,
            infiniteRepeatable(
                tween(periodMs / 2, easing = androidx.compose.animation.core.FastOutSlowInEasing),
                RepeatMode.Reverse,
            ),
        )
    return v
}

/** Level label for the screen reader and the legend: never raw numbers (§6.1). */
@Composable
fun levelLabel(level: Float, threshold: Float): String =
    stringResource(
        when {
            level < threshold * 0.5f -> R.string.native_ui_parent_level_quiet
            level < threshold -> R.string.native_ui_parent_level_some
            else -> R.string.native_ui_parent_level_loud
        }
    )

/** Bar meter (7 bars, >= 120 dp) with a dashed threshold line; flat when [live] is false. */
@Composable
fun SoundMeter(
    level: Float,
    threshold: Float,
    live: Boolean,
    modifier: Modifier = Modifier,
    tag: String = "parent_meter",
    alarm: Boolean = false,
    height: androidx.compose.ui.unit.Dp = 120.dp,
) {
    val shown = if (live) level.coerceIn(0f, 1f) else 0f
    val desc = stringResource(R.string.native_ui_parent_meter_a11y, levelLabel(shown, threshold))
    val colors = LocalAppColors.current
    val weights = remember { floatArrayOf(0.6f, 0.8f, 1.0f, 0.9f, 1.0f, 0.8f, 0.6f) }
    val track = MaterialTheme.colorScheme.surfaceVariant
    val okColor = colors.ok
    val hot = if (alarm) colors.bad else colors.warn
    Canvas(
        modifier
            .fillMaxWidth()
            .height(maxOf(height, 120.dp))
            .semantics { contentDescription = desc }
            .testTag(tag)
    ) {
        val n = weights.size
        val gap = 10.dp.toPx()
        val bw = (size.width - gap * (n - 1)) / n
        val thresholdY = size.height * (1f - threshold.coerceIn(0f, 1f))
        for (i in 0 until n) {
            val x = i * (bw + gap)
            drawRoundRect(track, Offset(x, 0f), Size(bw, size.height), CornerRadius(8.dp.toPx()))
            val fill = (shown * 1.6f * weights[i]).coerceIn(0f, 1f)
            if (fill > 0f) {
                val h = size.height * fill
                val c = if (size.height - h < thresholdY && live) hot else okColor
                drawRoundRect(c, Offset(x, size.height - h), Size(bw, h), CornerRadius(8.dp.toPx()))
            }
        }
        drawLine(
            colors.secondaryText,
            Offset(0f, thresholdY),
            Offset(size.width, thresholdY),
            2.dp.toPx(),
            pathEffect = PathEffect.dashPathEffect(floatArrayOf(14f, 10f)),
        )
    }
}

/**
 * Push-to-listen: hold = listen, slide up >= 72 dp = lock, tap while locked = stop. TalkBack
 * double-tap toggles locked listening (no gesture needed). Disabled while not connected.
 */
@Composable
fun ListenButton(
    mode: ListenMode,
    connected: Boolean,
    onMode: (ListenMode) -> Unit,
    modifier: Modifier = Modifier,
    tag: String = "parent_listen",
) {
    val modeState = rememberUpdatedState(mode)
    val onModeState = rememberUpdatedState(onMode)
    val haptic = LocalHapticFeedback.current
    val lockPx = with(LocalDensity.current) { 72.dp.toPx() }
    val colors = LocalAppColors.current
    val label =
        stringResource(
            when {
                !connected -> R.string.native_ui_listen_disabled
                mode == ListenMode.HOLD -> R.string.native_ui_listen_holding
                mode == ListenMode.LOCKED -> R.string.native_ui_listen_locked
                else -> R.string.native_ui_listen_idle
            }
        )
    val a11yState =
        if (connected && mode == ListenMode.LOCKED)
            stringResource(R.string.native_ui_listen_a11y_locked)
        else label
    val container =
        when {
            !connected -> MaterialTheme.colorScheme.surfaceVariant
            mode == ListenMode.HOLD -> MaterialTheme.colorScheme.primary
            mode == ListenMode.LOCKED -> StateColors.Green
            else -> MaterialTheme.colorScheme.primaryContainer
        }
    val content =
        when {
            !connected -> colors.secondaryText
            mode == ListenMode.HOLD -> MaterialTheme.colorScheme.onPrimary
            mode == ListenMode.LOCKED -> StateColors.OnState
            else -> MaterialTheme.colorScheme.onPrimaryContainer
        }
    Column(
        modifier.fillMaxWidth(),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        if (connected && mode == ListenMode.HOLD) {
            Text(
                stringResource(R.string.native_ui_listen_lock_hint),
                Modifier.testTag(tag + "_lock_target"),
                style = MaterialTheme.typography.titleMedium,
                color = colors.ok,
                textAlign = TextAlign.Center,
            )
        }
        Box(
            Modifier.fillMaxWidth(0.92f)
                .heightIn(min = if (mode == ListenMode.HOLD) 96.dp else 72.dp)
                .background(container, RoundedCornerShape(24.dp))
                .semantics(mergeDescendants = true) {
                    role = Role.Button
                    stateDescription = a11yState
                    if (!connected) disabled()
                    onClick {
                        onModeState.value(
                            if (modeState.value == ListenMode.OFF) ListenMode.LOCKED
                            else ListenMode.OFF
                        )
                        true
                    }
                }
                .pointerInput(connected) {
                    if (!connected) return@pointerInput
                    awaitEachGesture {
                        val down = awaitFirstDown()
                        if (modeState.value == ListenMode.LOCKED) {
                            waitForUpOrCancellation()
                            onModeState.value(ListenMode.OFF)
                            return@awaitEachGesture
                        }
                        val early = withTimeoutOrNull(150) { waitForUpOrCancellation() }
                        if (early != null || currentEvent.changes.none { it.pressed })
                            return@awaitEachGesture
                        onModeState.value(ListenMode.HOLD)
                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        var locked = false
                        while (true) {
                            val change = awaitPointerEvent().changes.firstOrNull() ?: break
                            if (!change.pressed) break
                            if (!locked && change.position.y - down.position.y <= -lockPx) {
                                locked = true
                                onModeState.value(ListenMode.LOCKED)
                                haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                            }
                            change.consume()
                        }
                        if (!locked) onModeState.value(ListenMode.OFF)
                    }
                }
                .testTag(tag),
            contentAlignment = Alignment.Center,
        ) {
            Column(
                Modifier.padding(horizontal = 20.dp, vertical = 12.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                androidx.compose.foundation.layout.Row(
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    GlyphIcon(
                        when (mode) {
                            ListenMode.OFF -> Glyph.HEADPHONES
                            ListenMode.HOLD -> Glyph.WAVE
                            ListenMode.LOCKED -> Glyph.STOP
                        },
                        content,
                        size = 32.dp,
                    )
                    Text(
                        label,
                        style = MaterialTheme.typography.titleLarge,
                        fontWeight = FontWeight.SemiBold,
                        color = content,
                        textAlign = TextAlign.Center,
                    )
                }
            }
        }
    }
}
