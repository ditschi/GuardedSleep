@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package org.guardedsleep.app.ui.common

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.model.Banner
import org.guardedsleep.app.ui.model.Sensitivity
import org.guardedsleep.app.ui.model.BannerKind
import org.guardedsleep.app.ui.model.BannerLevel
import org.guardedsleep.app.ui.model.NightMode
import org.guardedsleep.app.ui.model.bannerLevel
import org.guardedsleep.app.ui.theme.LocalAppColors
import org.guardedsleep.app.ui.theme.LocalSafetyColors

/*
 * The calm monitor's parts (`redesign-calm-ui` design sections 3, 6, 7 and 11): the banner stack,
 * the sound strip, the tool row, the dock tiles and the pause snackbar. They render what the
 * screen hands them; ranking, thresholds and timers live in the contract and the core.
 */

// ---- banners -----------------------------------------------------------------------------------

/** One banner as shown: copy and action already resolved by the screen. */
data class BannerSpec(
    val banner: Banner,
    val title: String,
    val body: String? = null,
    val glyph: Glyph = Glyph.EXCLAIM,
    val actionLabel: String? = null,
    val onAction: (() -> Unit)? = null,
)

private class BannerColors(val container: Color, val content: Color)

/**
 * Banners take the fixed safety colours, never the colour scheme, so night mode cannot recolour a
 * warning. A paused alert is neutral (blue is reserved for connecting), "Vibrate only" is a note.
 */
@Composable
private fun bannerColors(kind: BannerKind): BannerColors {
    val safety = LocalSafetyColors.current
    val scheme = MaterialTheme.colorScheme
    return when (kind) {
        BannerKind.SECURITY -> BannerColors(safety.alertContainer, safety.alert)
        BannerKind.MIC_SILENT,
        BannerKind.READINESS,
        BannerKind.BABY_BATTERY_LOW,
        BannerKind.SILENCE,
        BannerKind.UNIT_WARNING,
        BannerKind.WEAK_CONNECTION -> BannerColors(safety.warnContainer, safety.warn)
        BannerKind.PAUSED -> BannerColors(scheme.surfaceContainerHighest, scheme.onSurface)
        BannerKind.VIBRATE_ONLY -> BannerColors(safety.cautionContainer, safety.caution)
    }
}

@Composable
fun BannerCard(spec: BannerSpec, modifier: Modifier = Modifier) {
    val colors = bannerColors(spec.banner.kind)
    val level = bannerLevel(spec.banner.kind)
    val tag = "parent_banner_" + spec.banner.id
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 62.dp)
            .background(colors.container, RoundedCornerShape(18.dp))
            .padding(start = 14.dp, end = 10.dp, top = 8.dp, bottom = 8.dp)
            .semantics(mergeDescendants = true) {
                // A new banner of warning level or higher is announced once, when it appears.
                when (level) {
                    BannerLevel.ALERT -> liveRegion = LiveRegionMode.Assertive
                    BannerLevel.WARN -> liveRegion = LiveRegionMode.Polite
                    BannerLevel.INFO,
                    BannerLevel.NOTE -> Unit
                }
            }
            .testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        GlyphIcon(spec.glyph, colors.content, size = 24.dp)
        Column(Modifier.weight(1f)) {
            Text(
                spec.title,
                style = MaterialTheme.typography.titleSmall,
                fontWeight = FontWeight.Bold,
                color = colors.content,
            )
            if (spec.body != null)
                Text(spec.body, style = MaterialTheme.typography.bodyMedium, color = colors.content)
        }
        if (spec.actionLabel != null && spec.onAction != null) {
            Box(
                Modifier.heightIn(min = MinTouch)
                    .widthIn(min = MinTouch)
                    .border(BorderStroke(1.5.dp, colors.content), RoundedCornerShape(24.dp))
                    .clickable(role = Role.Button, onClick = spec.onAction)
                    .padding(horizontal = 14.dp)
                    .testTag(tag + "_action"),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    spec.actionLabel,
                    style = MaterialTheme.typography.labelLarge,
                    fontWeight = FontWeight.Bold,
                    color = colors.content,
                    maxLines = 1,
                )
            }
        }
    }
}

/**
 * The ranked banners: the first [shown] and one "+N more" row for the rest (a plain 48 dp text link
 * opening the list; its accessibility label names the folded notices).
 */
@Composable
fun BannerStack(
    shown: List<BannerSpec>,
    folded: List<BannerSpec>,
    onMore: () -> Unit,
    modifier: Modifier = Modifier,
) {
    if (shown.isEmpty()) return
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        shown.forEach { BannerCard(it) }
        if (folded.isNotEmpty()) {
            val notices = folded.joinToString(", ") { it.title }
            val a11y =
                pluralStringResource(
                    R.plurals.native_ui_banner_more_a11y,
                    folded.size,
                    folded.size,
                    notices,
                )
            Box(
                Modifier.heightIn(min = MinTouch)
                    .clickable(role = Role.Button, onClick = onMore)
                    .semantics { contentDescription = a11y }
                    .padding(horizontal = 8.dp)
                    .testTag("parent_banner_more"),
                contentAlignment = Alignment.CenterStart,
            ) {
                Text(
                    stringResource(R.string.native_ui_banner_more, folded.size),
                    Modifier.clearAndSetSemantics {},
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.Bold,
                    color = MaterialTheme.colorScheme.primary,
                )
            }
        }
    }
}

// ---- sound strip -------------------------------------------------------------------------------

enum class StripState {
    LIVE,
    /** Not connected: flat, with "No sound coming in". */
    OFFLINE,
    /** `mic_silent`: hatched, with "Microphone silent". */
    MIC_SILENT,
}

/**
 * The last minute of sound rising from a horizon line, with the dashed alert line labelled "A sound
 * above this line sends an alert" (frames 03 to 06). No text sits inside the bars, so nothing
 * collides at large text. [bars] are oldest first, 0..1.
 */
@Composable
fun SoundStrip(
    bars: List<Float>,
    threshold: Float,
    state: StripState,
    modifier: Modifier = Modifier,
    tag: String = "parent_meter",
) {
    val scheme = MaterialTheme.colorScheme
    val safety = LocalSafetyColors.current
    val level = bars.lastOrNull() ?: 0f
    val live = state == StripState.LIVE
    val note =
        when (state) {
            StripState.LIVE -> null
            StripState.OFFLINE -> stringResource(R.string.native_ui_monitor_no_sound)
            StripState.MIC_SILENT -> stringResource(R.string.native_ui_monitor_mic_silent_strip)
        }
    val a11y =
        note
            ?: stringResource(
                R.string.native_ui_parent_meter_a11y,
                levelLabel(level, threshold),
            )
    val base = scheme.surfaceContainer
    val hatch = scheme.surfaceContainerHigh
    val low = scheme.onSurfaceVariant.copy(alpha = 0.45f)
    val top = scheme.primary
    val over = safety.warn
    val line = scheme.onSurfaceVariant
    val horizon = scheme.outline
    Column(
        modifier
            .fillMaxWidth()
            .background(base, RoundedCornerShape(24.dp))
            .padding(horizontal = 16.dp, vertical = 12.dp)
            .semantics { contentDescription = a11y }
            .testTag(tag),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Box(Modifier.fillMaxWidth().height(150.dp)) {
            Canvas(Modifier.fillMaxWidth().height(150.dp)) {
                if (state == StripState.MIC_SILENT) {
                    // Hatched: nothing comes in, which is not the same as quiet.
                    val step = 14.dp.toPx()
                    var x = -size.height
                    while (x < size.width) {
                        drawLine(
                            hatch,
                            Offset(x, size.height),
                            Offset(x + size.height, 0f),
                            strokeWidth = 6.dp.toPx(),
                        )
                        x += step * 2
                    }
                }
                val n = bars.size.coerceAtLeast(1)
                val gap = 2.dp.toPx()
                val bw = (size.width - gap * (n - 1)) / n
                val baseY = size.height - 6.dp.toPx()
                val maxH = baseY
                // Bars and line share the slider's decibel scale (`Sensitivity.meterFraction`).
                val thresholdY = baseY - maxH * Sensitivity.meterFraction(threshold)
                bars.forEachIndexed { i, v ->
                    val h =
                        if (live) (maxH * Sensitivity.meterFraction(v)).coerceAtLeast(3.dp.toPx())
                        else 3.dp.toPx()
                    val c =
                        when {
                            !live -> low.copy(alpha = 0.3f)
                            v >= threshold && v > 0f -> over
                            i == n - 1 -> top
                            else -> low
                        }
                    drawRoundRect(
                        c,
                        Offset(i * (bw + gap), baseY - h),
                        Size(bw, h),
                        CornerRadius(3.dp.toPx()),
                    )
                }
                drawLine(horizon, Offset(0f, baseY), Offset(size.width, baseY), 2.dp.toPx())
                if (live) {
                    drawLine(
                        line,
                        Offset(0f, thresholdY),
                        Offset(size.width, thresholdY),
                        2.dp.toPx(),
                        pathEffect = PathEffect.dashPathEffect(floatArrayOf(14f, 10f)),
                    )
                }
            }
            if (note != null) {
                Text(
                    note,
                    Modifier.align(Alignment.Center)
                        .background(base, RoundedCornerShape(8.dp))
                        .padding(horizontal = 8.dp, vertical = 2.dp)
                        .testTag("parent_strip_note"),
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.SemiBold,
                    color = scheme.onSurface,
                )
            }
        }
        if (live) {
            Text(
                stringResource(R.string.native_ui_monitor_alert_line),
                Modifier.testTag("parent_strip_alert_line"),
                style = MaterialTheme.typography.bodyMedium,
                color = scheme.onSurfaceVariant,
            )
        }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(
                stringResource(R.string.native_ui_monitor_last_minute),
                style = MaterialTheme.typography.labelMedium,
                color = scheme.onSurfaceVariant,
            )
            Text(
                stringResource(R.string.native_ui_monitor_now),
                style = MaterialTheme.typography.labelMedium,
                color = scheme.onSurfaceVariant,
            )
        }
    }
}

// ---- tool row ----------------------------------------------------------------------------------

/** The alert chip: opens the choice between "Sound" and "Vibrate only". */
data class AlertChipSpec(
    val vibrateOnly: Boolean,
    /** Icon only while its "Vibrate only" banner shows; labelled again when the banner folds. */
    val iconOnly: Boolean,
    val onClick: () -> Unit,
)

/**
 * The tool row under the status bar (frames 45 and 47): the alert chip (it ellipsizes before
 * anything else is cut), the night control (icon only when off or above 1.15x text, icon plus "On"
 * or "Auto" otherwise) and the settings gear, which is laid out first among the fixed controls and
 * never shrinks. From 1.5x text the chip takes row 1 and night plus gear sit right-aligned on
 * row 2. Traversal order: alert chip, night, settings (the status bar comes before this row).
 */
@Composable
fun ToolRow(
    alert: AlertChipSpec?,
    night: Pair<NightMode, () -> Unit>?,
    onSettings: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val fontScale = LocalDensity.current.fontScale
    val stacked = fontScale >= 1.5f
    val chip: @Composable (Modifier) -> Unit = { m ->
        if (alert != null) AlertChip(alert, m)
    }
    val controls: @Composable () -> Unit = {
        if (night != null) NightButton(night.first, night.second, labelled = fontScale <= 1.15f)
        ToolIconButton(
            Glyph.GEAR,
            stringResource(R.string.native_ui_common_settings),
            "tool_settings",
            onSettings,
        )
    }
    if (stacked) {
        Column(
            modifier.fillMaxWidth().padding(horizontal = 16.dp).testTag("parent_tool_row"),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            chip(Modifier)
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                controls()
            }
        }
    } else {
        Row(
            modifier.fillMaxWidth().padding(horizontal = 16.dp).testTag("parent_tool_row"),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            // The fixed controls are measured before the weighted chip, so the chip gives way.
            chip(Modifier.weight(1f, fill = false))
            Box(Modifier.weight(1f))
            controls()
        }
    }
}

@Composable
private fun AlertChip(spec: AlertChipSpec, modifier: Modifier) {
    val scheme = MaterialTheme.colorScheme
    val a11y =
        stringResource(
            if (spec.vibrateOnly) R.string.native_ui_alert_style_chip_a11y_vibrate
            else R.string.native_ui_alert_style_chip_a11y_sound
        )
    val text =
        stringResource(
            if (spec.vibrateOnly) R.string.native_ui_alert_style_chip_vibrate
            else R.string.native_ui_alert_style_chip_sound
        )
    Row(
        modifier
            .heightIn(min = MinTouch)
            .background(scheme.surfaceContainer, RoundedCornerShape(16.dp))
            .clickable(role = Role.Button, onClick = spec.onClick)
            .semantics { contentDescription = a11y }
            .padding(horizontal = 14.dp)
            .testTag("tool_alert_chip"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        GlyphIcon(if (spec.vibrateOnly) Glyph.ZZ else Glyph.BELL, scheme.onSurface, size = 20.dp)
        if (!spec.iconOnly)
            Text(
                text,
                Modifier.weight(1f, fill = false),
                style = MaterialTheme.typography.labelLarge,
                fontWeight = FontWeight.SemiBold,
                color = scheme.onSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
    }
}

@Composable
private fun ToolIconButton(glyph: Glyph, label: String, tag: String, onClick: () -> Unit) {
    val scheme = MaterialTheme.colorScheme
    Box(
        Modifier.size(MinTouch)
            .background(scheme.surfaceContainer, CircleShape)
            .clickable(role = Role.Button, onClick = onClick)
            .semantics { contentDescription = label }
            .testTag(tag),
        contentAlignment = Alignment.Center,
    ) {
        GlyphIcon(glyph, scheme.onSurfaceVariant, size = 24.dp)
    }
}

@Composable
private fun NightButton(state: NightMode, onClick: () -> Unit, labelled: Boolean) {
    val scheme = MaterialTheme.colorScheme
    val a11y =
        stringResource(
            when (state) {
                NightMode.OFF -> R.string.native_ui_tool_night_a11y_off
                NightMode.ON -> R.string.native_ui_tool_night_a11y_on
                NightMode.AUTO -> R.string.native_ui_tool_night_a11y_auto
            }
        )
    if (state == NightMode.OFF || !labelled) {
        // Icon only: the state travels in the description, and a tint marks "on".
        val tint =
            if (state == NightMode.OFF) scheme.onSurfaceVariant else scheme.onPrimaryContainer
        Box(
            Modifier.size(MinTouch)
                .background(
                    if (state == NightMode.OFF) scheme.surfaceContainer
                    else scheme.primaryContainer,
                    CircleShape,
                )
                .clickable(role = Role.Button, onClick = onClick)
                .semantics { contentDescription = a11y }
                .testTag("tool_night"),
            contentAlignment = Alignment.Center,
        ) {
            GlyphIcon(Glyph.MOON, tint, size = 24.dp)
        }
    } else {
        Row(
            Modifier.heightIn(min = MinTouch)
                .background(scheme.primaryContainer, RoundedCornerShape(24.dp))
                .clickable(role = Role.Button, onClick = onClick)
                .semantics { contentDescription = a11y }
                .padding(horizontal = 14.dp)
                .testTag("tool_night"),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            GlyphIcon(Glyph.MOON, scheme.onPrimaryContainer, size = 20.dp)
            Text(
                stringResource(
                    if (state == NightMode.ON) R.string.native_ui_tool_night_on
                    else R.string.native_ui_tool_night_auto
                ),
                style = MaterialTheme.typography.labelLarge,
                fontWeight = FontWeight.SemiBold,
                color = scheme.onPrimaryContainer,
                maxLines = 1,
            )
        }
    }
}

// ---- dock --------------------------------------------------------------------------------------

/** A dock tile: icon above a one-line 14 sp label, at least 72 dp high. */
@Composable
fun DockTile(
    glyph: Glyph,
    label: String,
    accessibilityLabel: String,
    tag: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    active: Boolean = false,
) {
    val scheme = MaterialTheme.colorScheme
    val container = if (active) scheme.primaryContainer else scheme.surfaceContainerHigh
    val content = if (active) scheme.onPrimaryContainer else scheme.onSurface
    Column(
        modifier
            .heightIn(min = 72.dp)
            .background(container, RoundedCornerShape(22.dp))
            .clickable(role = Role.Button, onClick = onClick)
            .semantics { contentDescription = accessibilityLabel }
            .padding(horizontal = 4.dp, vertical = 8.dp)
            .testTag(tag),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        GlyphIcon(glyph, content, size = 26.dp)
        Text(
            label,
            style = MaterialTheme.typography.labelLarge,
            fontWeight = FontWeight.Bold,
            color = content,
            textAlign = TextAlign.Center,
            // From 1.5x text a long label may take a second line rather than be cut.
            maxLines = if (LocalDensity.current.fontScale >= 1.5f) 2 else 1,
            softWrap = LocalDensity.current.fontScale >= 1.5f,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/** The three tiles under Listen: one row, or 2 + 1 from 1.5x text so no label is cut. */
@Composable
fun DockTiles(
    modifier: Modifier = Modifier,
    content: @Composable (weighted: (Float) -> Modifier) -> Unit,
) {
    val stacked = LocalDensity.current.fontScale >= 1.5f
    if (stacked) {
        FlowRow(
            modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
            maxItemsInEachRow = 2,
        ) {
            content { Modifier.weight(1f) }
        }
    } else {
        Row(modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            content { Modifier.weight(it) }
        }
    }
}

// ---- pause snackbar ----------------------------------------------------------------------------

/** How long the Undo of a pause is offered. */
const val PAUSE_UNDO_MS = 8_000L

/**
 * "Noise alerts paused for {minutes} min" with Undo, shown for [PAUSE_UNDO_MS] (design section 8).
 * The screen offers it only when the pause starts while no noise event runs, because in the core a
 * pause acknowledges a running event and Undo could not restore that.
 */
@Composable
fun PauseSnackbar(
    minutes: Int,
    onUndo: () -> Unit,
    onTimeout: () -> Unit,
    modifier: Modifier = Modifier,
) {
    LaunchedEffect(minutes) {
        delay(PAUSE_UNDO_MS)
        onTimeout()
    }
    val scheme = MaterialTheme.colorScheme
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 52.dp)
            .background(scheme.inverseSurface, RoundedCornerShape(14.dp))
            .padding(start = 16.dp, end = 6.dp)
            .testTag("parent_pause_snackbar"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Text(
            stringResource(R.string.native_ui_snooze_snackbar, minutes),
            Modifier.weight(1f),
            style = MaterialTheme.typography.bodyMedium,
            fontWeight = FontWeight.SemiBold,
            color = scheme.inverseOnSurface,
        )
        Box(
            Modifier.heightIn(min = MinTouch)
                .widthIn(min = MinTouch)
                .clickable(role = Role.Button, onClick = onUndo)
                .padding(horizontal = 14.dp)
                .testTag("parent_pause_undo"),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                stringResource(R.string.native_ui_snooze_undo),
                style = MaterialTheme.typography.labelLarge,
                fontWeight = FontWeight.ExtraBold,
                color = scheme.inverseOnSurface,
            )
        }
    }
}

/**
 * True while the pause tile is the labelled "Paused" state; kept here so the tile text is one rule.
 */
@Composable
fun pauseTileLabel(paused: Boolean): String =
    stringResource(
        if (paused) R.string.native_ui_snooze_tile_paused else R.string.native_ui_snooze_tile
    )

/** The quiet line under the strip: Quiet mode and the last test. */
@Composable
fun MetaLine(text: String, onClick: (() -> Unit)?, modifier: Modifier = Modifier) {
    val base = modifier.fillMaxWidth().heightIn(min = 32.dp)
    Text(
        text,
        (if (onClick != null) base.clickable(role = Role.Button, onClick = onClick) else base)
            .testTag("parent_meta"),
        style = MaterialTheme.typography.bodyMedium,
        color = LocalAppColors.current.secondaryText,
    )
}
