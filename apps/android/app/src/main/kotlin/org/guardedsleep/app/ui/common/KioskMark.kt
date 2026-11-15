package org.guardedsleep.app.ui.common

import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import org.guardedsleep.app.ui.model.BabyBattery
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.LinkUi
import org.guardedsleep.app.ui.theme.KioskMarks

/**
 * The baby kiosk's link mark, told by shape so it reads without hue (`connection-status` "Baby unit
 * edge indicator"): a short bar when fine, nothing at level 0, a dashed top edge at level 1 and a
 * slow-pulsing frame from level 2.
 */
enum class KioskMarkShape {
    NONE,
    BAR,
    DASHED,
    FRAME,
}

/** The mark for a link. */
fun kioskMarkShape(link: LinkUi): KioskMarkShape =
    when (link.phase) {
        LinkPhase.CONNECTED -> KioskMarkShape.BAR
        // Nothing is wrong, or nobody is monitoring: the words say so, no mark.
        LinkPhase.PEER_STOPPED,
        LinkPhase.IDLE,
        LinkPhase.STOPPED -> KioskMarkShape.NONE
        LinkPhase.FAILED_NEEDS_USER -> KioskMarkShape.FRAME
        LinkPhase.CONNECTING,
        LinkPhase.RECONNECTING ->
            when {
                link.level >= 2 -> KioskMarkShape.FRAME
                link.level == 1 -> KioskMarkShape.DASHED
                else -> KioskMarkShape.NONE
            }
    }

/**
 * Why the baby device needs a hand: its own battery or charger (`connection-status` "Baby
 * needs-attention mark"). Shown as a separate small square and a second status line.
 */
enum class KioskAttention {
    NOT_CHARGING,
    BATTERY_LOW,
    BATTERY_CRITICAL,
}

/**
 * [chargerSeen]: the charger was connected earlier in this monitoring run (`BabyMonitorState`), so
 * a reading that is not plugged in now means it was pulled.
 */
fun kioskAttention(battery: BatteryReading?, chargerSeen: Boolean): KioskAttention? =
    when {
        battery == null -> null
        // Critical counts even while plugged in: a weak charger or a loose cable must not hide it.
        battery.pct < BabyBattery.CRITICAL_PCT -> KioskAttention.BATTERY_CRITICAL
        battery.charging -> null
        battery.pct < BabyBattery.LOW_PCT -> KioskAttention.BATTERY_LOW
        chargerSeen -> KioskAttention.NOT_CHARGING
        else -> null
    }

/**
 * How long the set-up confirmation shows after the first verified contact (`connection-status`).
 */
const val SetupConfirmSeconds = 60

/** Seconds of the set-up confirmation left, or null when none runs (no contact yet, or over). */
fun setupConfirmRemaining(firstContactElapsedMs: Long?, nowElapsedMs: Long): Int? {
    val first = firstContactElapsedMs ?: return null
    val left = SetupConfirmSeconds - (nowElapsedMs - first) / 1000
    return if (left in 1..SetupConfirmSeconds) left.toInt() else null
}

/** Window brightness of the kiosk: the minimum except for the confirmation and a touch-wake. */
const val KioskMinBrightness = 0.01f
const val KioskWakeBrightness = 0.1f

/** A talk keeps the minimum: only the dim talk indicator shows (`device-setup` "Baby kiosk"). */
fun kioskBrightness(awake: Boolean, talking: Boolean): Float =
    if (awake && !talking) KioskWakeBrightness else KioskMinBrightness

private val FrameWidth = 5.dp
private val EdgeHeight = 6.dp

/** Draws [shape] (and the needs-attention square) along the top and edge of the kiosk. */
@Composable
fun KioskMark(
    shape: KioskMarkShape,
    attention: Boolean,
    reduceMotion: Boolean,
    modifier: Modifier = Modifier,
) {
    Box(modifier.fillMaxSize().testTag("baby_kiosk_edge")) {
        // Needs attention keeps the bar and adds a square, both in the warn mark colour.
        val barColor = if (attention) KioskMarks.Warn else KioskMarks.Ok
        if (shape == KioskMarkShape.BAR || attention) {
            Row(
                Modifier.align(Alignment.TopCenter).padding(top = 24.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(
                    Modifier.size(width = 44.dp, height = EdgeHeight)
                        .background(barColor, RoundedCornerShape(3.dp))
                        .testTag("baby_kiosk_mark_bar")
                )
                if (attention) {
                    Box(
                        Modifier.size(10.dp)
                            .background(KioskMarks.Warn, RoundedCornerShape(2.dp))
                            .testTag("baby_kiosk_mark_attention")
                    )
                }
            }
        }
        if (shape == KioskMarkShape.DASHED) {
            Canvas(
                Modifier.align(Alignment.TopCenter)
                    .fillMaxWidth()
                    .height(EdgeHeight)
                    .testTag("baby_kiosk_mark_dashed")
            ) {
                val y = size.height / 2
                drawLine(
                    KioskMarks.Warn,
                    Offset(0f, y),
                    Offset(size.width, y),
                    strokeWidth = size.height,
                    pathEffect =
                        PathEffect.dashPathEffect(floatArrayOf(16.dp.toPx(), 10.dp.toPx())),
                )
            }
        }
        if (shape == KioskMarkShape.FRAME) {
            val alpha =
                if (reduceMotion) 0.9f
                else {
                    val t = rememberInfiniteTransition()
                    val v by
                        t.animateFloat(
                            0.45f,
                            0.9f,
                            // A 4 s period: 2 s up and 2 s down.
                            infiniteRepeatable(
                                tween(2000, easing = CubicBezierEasing(0.37f, 0f, 0.63f, 1f)),
                                RepeatMode.Reverse,
                            ),
                        )
                    v
                }
            Canvas(Modifier.fillMaxSize().testTag("baby_kiosk_mark_frame")) {
                val w = FrameWidth.toPx()
                drawRoundRect(
                    KioskMarks.Alert,
                    topLeft = Offset(w / 2, w / 2),
                    size = Size(size.width - w, size.height - w),
                    cornerRadius = CornerRadius(24.dp.toPx()),
                    alpha = alpha,
                    style = Stroke(w),
                )
            }
        }
    }
}
