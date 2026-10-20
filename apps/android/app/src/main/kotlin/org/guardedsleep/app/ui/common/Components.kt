@file:OptIn(androidx.compose.ui.ExperimentalComposeUiApi::class)

package org.guardedsleep.app.ui.common

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.guardedsleep.app.ui.theme.LocalAppColors
import org.guardedsleep.app.ui.theme.StateColors

/** Inside [AdaptiveButtonRow] labels stay on one line (no mid-word wrapping). */
val LocalSingleLine = androidx.compose.runtime.compositionLocalOf { false }

/**
 * Equal-width buttons side by side when every label fits on one line, otherwise stacked full width.
 * Children must be plain buttons (one layout node each).
 */
@Composable
fun AdaptiveButtonRow(modifier: Modifier = Modifier, gap: Dp = 8.dp, content: @Composable () -> Unit) {
    androidx.compose.ui.layout.Layout(
        content = { androidx.compose.runtime.CompositionLocalProvider(LocalSingleLine provides true, content) },
        modifier = modifier.fillMaxWidth(),
    ) { measurables, constraints ->
        val g = gap.roundToPx()
        val n = measurables.size
        val natural = measurables.maxOfOrNull { it.maxIntrinsicWidth(Int.MAX_VALUE) } ?: 0
        val sideBySide = n * natural + g * (n - 1) <= constraints.maxWidth
        val w = constraints.maxWidth
        if (sideBySide) {
            val cw = (w - g * (n - 1)) / n
            val ps = measurables.map { it.measure(androidx.compose.ui.unit.Constraints.fixedWidth(cw)) }
            val h = ps.maxOfOrNull { it.height } ?: 0
            layout(w, h) { ps.forEachIndexed { i, p -> p.placeRelative(i * (cw + g), 0) } }
        } else {
            val ps = measurables.map { it.measure(androidx.compose.ui.unit.Constraints.fixedWidth(w)) }
            val h = ps.sumOf { it.height } + g * (n - 1).coerceAtLeast(0)
            layout(w, h) { var y = 0; ps.forEach { p -> p.placeRelative(0, y); y += p.height + g } }
        }
    }
}

val MinTouch = 48.dp
val PrimaryHeight = 72.dp
val Gutter = 16.dp

/** Screen root: safe-drawing insets, background, test tags exposed as resource ids (Maestro/UiAutomator). */
@Composable
fun ScreenRoot(
    tag: String,
    modifier: Modifier = Modifier,
    background: Color = MaterialTheme.colorScheme.background,
    content: @Composable androidx.compose.foundation.layout.BoxScope.() -> Unit,
) {
    Box(
        modifier
            .fillMaxSize()
            .background(background)
            .safeDrawingPadding()
            .semantics { testTagsAsResourceId = true }
            .testTag(tag),
    ) { content() }
}

@Composable
fun ScrollColumn(
    modifier: Modifier = Modifier,
    verticalArrangement: Arrangement.Vertical = Arrangement.spacedBy(12.dp),
    content: @Composable ColumnScope.() -> Unit,
) {
    Column(
        modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = Gutter, vertical = Gutter),
        verticalArrangement = verticalArrangement,
        content = content,
    )
}

@Composable
fun Title(text: String, tag: String, modifier: Modifier = Modifier) {
    Text(
        text,
        modifier.semantics { heading() }.testTag(tag),
        style = MaterialTheme.typography.headlineMedium,
        fontWeight = FontWeight.SemiBold,
        color = MaterialTheme.colorScheme.onBackground,
    )
}

@Composable
fun Body(text: String, modifier: Modifier = Modifier, tag: String? = null, color: Color = LocalAppColors.current.secondaryText) {
    Text(
        text,
        (if (tag != null) modifier.testTag(tag) else modifier),
        style = MaterialTheme.typography.bodyLarge,
        color = color,
    )
}

@Composable
fun PrimaryButton(
    text: String,
    onClick: () -> Unit,
    tag: String,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    container: Color = MaterialTheme.colorScheme.primary,
    content: Color = MaterialTheme.colorScheme.onPrimary,
    leading: (@Composable () -> Unit)? = null,
) {
    Button(
        onClick = onClick,
        enabled = enabled,
        modifier = modifier.fillMaxWidth().heightIn(min = PrimaryHeight).testTag(tag),
        shape = RoundedCornerShape(20.dp),
        colors = ButtonDefaults.buttonColors(containerColor = container, contentColor = content),
        contentPadding = PaddingValues(horizontal = 20.dp, vertical = 12.dp),
    ) {
        if (leading != null) { leading(); Spacer(Modifier.size(10.dp)) }
        val single = LocalSingleLine.current
        Text(text, style = MaterialTheme.typography.titleLarge, textAlign = TextAlign.Center, softWrap = !single, maxLines = if (single) 1 else Int.MAX_VALUE)
    }
}

@Composable
fun SecondaryButton(
    text: String,
    onClick: () -> Unit,
    tag: String,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    destructive: Boolean = false,
) {
    val color = if (destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onBackground
    OutlinedButton(
        onClick = onClick,
        enabled = enabled,
        modifier = modifier.heightIn(min = MinTouch).testTag(tag),
        border = BorderStroke(1.5.dp, if (enabled) color.copy(alpha = 0.7f) else MaterialTheme.colorScheme.outline),
        colors = ButtonDefaults.outlinedButtonColors(contentColor = color),
    ) {
        val single = LocalSingleLine.current
        Text(text, style = MaterialTheme.typography.titleMedium, textAlign = TextAlign.Center, softWrap = !single, maxLines = if (single) 1 else Int.MAX_VALUE)
    }
}

@Composable
fun QuietButton(text: String, onClick: () -> Unit, tag: String, modifier: Modifier = Modifier, enabled: Boolean = true) {
    TextButton(onClick = onClick, enabled = enabled, modifier = modifier.heightIn(min = MinTouch).testTag(tag)) {
        Text(text, style = MaterialTheme.typography.titleMedium, textAlign = TextAlign.Center)
    }
}

@Composable
fun AppCard(
    modifier: Modifier = Modifier,
    container: Color = MaterialTheme.colorScheme.surfaceVariant,
    onClick: (() -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    val m = modifier.fillMaxWidth()
    Surface(
        modifier = if (onClick != null) m.clickable(role = Role.Button, onClick = onClick) else m,
        shape = RoundedCornerShape(20.dp),
        color = container,
    ) { Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp), content = content) }
}

/** Visual weight of a state, always paired with a glyph and text. */
enum class Tone { OK, INFO, WARN, BAD, NEUTRAL, SNOOZE }

fun Tone.container(): Color = when (this) {
    Tone.OK -> StateColors.Green
    Tone.INFO -> StateColors.Yellow
    Tone.WARN -> StateColors.Orange
    Tone.BAD -> StateColors.Red
    Tone.NEUTRAL -> StateColors.Grey
    Tone.SNOOZE -> StateColors.Amber
}

/** Full-width ambient bar: colour + glyph + text, a live region for screen readers. */
@Composable
fun AmbientBar(
    glyph: Glyph,
    text: String,
    tone: Tone,
    tag: String,
    modifier: Modifier = Modifier,
    assertive: Boolean = false,
    extra: Glyph? = null,
    subText: String? = null,
) {
    Row(
        modifier
            .fillMaxWidth()
            .background(tone.container())
            .heightIn(min = 56.dp)
            .padding(horizontal = Gutter, vertical = 10.dp)
            .semantics(mergeDescendants = true) {
                liveRegion = if (assertive) LiveRegionMode.Assertive else LiveRegionMode.Polite
            }
            .testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        GlyphIcon(glyph, StateColors.OnState, size = 28.dp)
        if (extra != null) GlyphIcon(extra, StateColors.OnState, size = 22.dp)
        Column(Modifier.weight(1f)) {
            Text(text, style = MaterialTheme.typography.titleLarge, color = StateColors.OnState, fontWeight = FontWeight.SemiBold)
            if (subText != null) Text(subText, style = MaterialTheme.typography.bodyMedium, color = StateColors.OnState)
        }
    }
}

/** Status chip with glyph and text; optionally tappable (opens the matching sheet). */
@Composable
fun StatusChip(
    glyph: Glyph,
    text: String,
    tone: Tone,
    tag: String,
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
    extraGlyph: Glyph? = null,
    dimmed: Boolean = false,
) {
    val colors = LocalAppColors.current
    val (bg, fg) = when (tone) {
        Tone.NEUTRAL -> colors.neutralChip to colors.onNeutralChip
        else -> tone.container() to StateColors.OnState
    }
    val base = modifier.heightIn(min = MinTouch).alpha(if (dimmed) 0.6f else 1f).background(bg, RoundedCornerShape(24.dp))
    Row(
        (if (onClick != null) base.clickable(role = Role.Button, onClick = onClick) else base)
            .padding(horizontal = 14.dp, vertical = 8.dp)
            .testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        GlyphIcon(glyph, fg, size = 20.dp)
        if (extraGlyph != null) GlyphIcon(extraGlyph, fg, size = 20.dp)
        Text(text, style = MaterialTheme.typography.bodyMedium, color = fg, modifier = Modifier.weight(1f, fill = false))
    }
}

/** Setup/permission row state (S02/S03). */
enum class RowState { DONE, TODO, BLOCKED, CHECKING }

@Composable
fun RowScope.Spacer8() { Spacer(Modifier.size(8.dp)) }

fun Modifier.minWidth(w: Dp): Modifier = widthIn(min = w)

@Composable
fun Dot(color: Color, size: Dp = 10.dp) {
    Box(Modifier.size(size).background(color, RoundedCornerShape(50)))
}
