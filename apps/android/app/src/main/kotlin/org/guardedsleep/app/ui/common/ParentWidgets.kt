package org.guardedsleep.app.ui.common

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.model.ListenMode
import org.guardedsleep.app.ui.model.Sensitivity
import org.guardedsleep.app.ui.theme.LocalAppColors

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
    // Bars and tick share the slider's decibel scale (`Sensitivity.meterFraction`).
    val shownFraction = Sensitivity.meterFraction(shown)
    val tick = Sensitivity.meterFraction(threshold)
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
        val thresholdY = size.height * (1f - tick)
        for (i in 0 until n) {
            val x = i * (bw + gap)
            drawRoundRect(track, Offset(x, 0f), Size(bw, size.height), CornerRadius(8.dp.toPx()))
            val fill = (shownFraction * weights[i]).coerceIn(0f, 1f)
            if (fill > 0f) {
                val h = size.height * fill
                val c = if (shown >= threshold && live) hot else okColor
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
 * Listen as a tap toggle (`audio-monitoring` "Listen mode", design section 7): one tap starts, the
 * whole bar fills and reads "Listening · tap to stop" with a stop square, another tap stops. There
 * is no hold and no slide, so it works one-handed in the dark and with switch access. Unavailable
 * (disabled, with the reason as its sub-line) while not connected. The 60 minute limit lives in the
 * model, not here.
 */
@Composable
fun ListenButton(
    mode: ListenMode,
    connected: Boolean,
    onMode: (ListenMode) -> Unit,
    modifier: Modifier = Modifier,
    tag: String = "parent_listen",
    compact: Boolean = false,
) {
    val colors = LocalAppColors.current
    val on = connected && mode == ListenMode.ON
    val label =
        stringResource(
            when {
                !connected -> R.string.native_ui_listen_disabled
                on -> R.string.native_ui_listen_on
                else -> R.string.native_ui_listen_idle
            }
        )
    val sub =
        stringResource(
            when {
                !connected -> R.string.native_ui_listen_disabled_sub
                on -> R.string.native_ui_listen_on_sub
                else -> R.string.native_ui_listen_idle_sub
            }
        )
    val state =
        stringResource(
            if (on) R.string.native_ui_listen_state_on else R.string.native_ui_listen_state_off
        )
    val scheme = MaterialTheme.colorScheme
    val container =
        when {
            !connected -> scheme.surfaceVariant
            on -> scheme.primary
            else -> scheme.primaryContainer
        }
    val content =
        when {
            !connected -> colors.secondaryText
            on -> scheme.onPrimary
            else -> scheme.onPrimaryContainer
        }
    val knobBg =
        if (on) scheme.onPrimary
        else if (connected) scheme.primary else scheme.surfaceContainerHighest
    val knobFg =
        if (on) scheme.primary else if (connected) scheme.onPrimary else colors.secondaryText
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = if (compact) PrimaryHeight else 80.dp)
            .background(container, RoundedCornerShape(28.dp))
            .clickable(
                enabled = connected,
                role = Role.Button,
                onClick = { onMode(if (mode == ListenMode.OFF) ListenMode.ON else ListenMode.OFF) },
            )
            .semantics(mergeDescendants = true) { stateDescription = state }
            .padding(horizontal = 20.dp, vertical = 10.dp)
            .testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                label,
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.Bold,
                color = content,
            )
            if (!compact) Text(sub, style = MaterialTheme.typography.bodyMedium, color = content)
        }
        Box(
            Modifier.size(52.dp).background(knobBg, CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            GlyphIcon(if (on) Glyph.STOP else Glyph.HEADPHONES, knobFg, size = 28.dp)
        }
    }
}
