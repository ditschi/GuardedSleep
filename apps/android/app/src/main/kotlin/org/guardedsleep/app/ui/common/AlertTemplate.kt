package org.guardedsleep.app.ui.common

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.hideFromAccessibility
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import org.guardedsleep.app.ui.theme.AlertPalette

/** The big figure of an alert: shown as digits, spoken in words, never in a live region. */
data class AlertCounter(val text: String, val label: String, val spoken: String)

/** Decorative stage segments plus the words that say the same (`redesign-calm-ui` design 6). */
data class AlertProgress(val active: Int, val steps: Int, val words: String)

/**
 * The shared full-screen alert template (design section 6): a flat status strip hidden from
 * accessibility, a caps label, the headline, the counter with its word label, one sentence, the
 * decorative progress, a body slot (the cause card), a quiet text-button slot at the end of the
 * content and one filled action pinned below. The body scrolls; the action stays visible at 200 %
 * text. Colours come from a fixed [AlertPalette], never from the colour scheme.
 */
@Composable
fun AlertTemplate(
    tag: String,
    palette: AlertPalette,
    label: String,
    headline: String,
    headlineTag: String,
    modifier: Modifier = Modifier,
    hazard: Boolean = false,
    strip: (@Composable () -> Unit)? = null,
    counter: AlertCounter? = null,
    counterTag: String = "alert_counter",
    sentence: String? = null,
    sentenceTag: String = "alert_sentence",
    progress: AlertProgress? = null,
    body: (@Composable ColumnScope.() -> Unit)? = null,
    quiet: (@Composable () -> Unit)? = null,
    action: (@Composable () -> Unit)? = null,
) {
    val focus = remember { FocusRequester() }
    // Focus moves to the headline on entry; the strip and the caps label are not announced.
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    val bigText = LocalDensity.current.fontScale > 1.5f
    Column(modifier.fillMaxSize().background(palette.background).testTag(tag)) {
        if (hazard) HazardBand()
        if (strip != null) AlertStrip(palette, strip)
        Column(
            Modifier.weight(1f, fill = true)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp, vertical = 16.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Text(
                label.uppercase(),
                Modifier.semantics { hideFromAccessibility() }.testTag("alert_label"),
                style = MaterialTheme.typography.labelMedium,
                color = palette.accent,
            )
            Text(
                headline,
                Modifier.fillMaxWidth()
                    .focusRequester(focus)
                    .focusable()
                    .semantics {
                        heading()
                        // A stage change is announced once, assertively; the counter never is.
                        liveRegion = LiveRegionMode.Assertive
                    }
                    .testTag(headlineTag),
                style = MaterialTheme.typography.headlineMedium,
                color = palette.onBackground,
            )
            if (counter != null) {
                Column(
                    Modifier.semantics(mergeDescendants = true) {
                            contentDescription = counter.spoken
                        }
                        .testTag(counterTag)
                ) {
                    Text(
                        counter.text,
                        style =
                            if (bigText) MaterialTheme.typography.headlineMedium
                            else MaterialTheme.typography.displayMedium,
                        color = palette.onBackground,
                        fontWeight = FontWeight.SemiBold,
                    )
                    Text(
                        counter.label,
                        style = MaterialTheme.typography.bodyLarge,
                        color = palette.onVariant,
                    )
                }
            }
            if (sentence != null) {
                Text(
                    sentence,
                    Modifier.testTag(sentenceTag),
                    style = MaterialTheme.typography.bodyLarge,
                    color = palette.onVariant,
                )
            }
            if (progress != null) ProgressLine(progress, palette)
            if (body != null) body()
            if (quiet != null) Box(Modifier.fillMaxWidth()) { quiet() }
        }
        if (action != null) {
            Box(
                Modifier.fillMaxWidth()
                    .padding(horizontal = 20.dp)
                    .padding(top = 8.dp, bottom = 16.dp)
            ) {
                action()
            }
        }
    }
}

@Composable
private fun AlertStrip(palette: AlertPalette, content: @Composable () -> Unit) {
    Row(
        Modifier.fillMaxWidth()
            .heightIn(min = 48.dp)
            .padding(horizontal = 20.dp, vertical = 8.dp)
            .semantics { hideFromAccessibility() }
            .testTag("alert_strip"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        content()
    }
}

@Composable
private fun ProgressLine(p: AlertProgress, palette: AlertPalette) {
    Column(Modifier.testTag("alert_progress"), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(
            Modifier.fillMaxWidth().clearAndSetSemantics {},
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            repeat(p.steps) { i ->
                Box(
                    Modifier.weight(1f)
                        .height(6.dp)
                        .background(
                            if (i < p.active) palette.accent else palette.surface,
                            RoundedCornerShape(3.dp),
                        )
                )
            }
        }
        Text(p.words, style = MaterialTheme.typography.bodyMedium, color = palette.onVariant)
    }
}

/** Alternating diagonal-free hazard band along the top edge: the loudest stage. */
@Composable
private fun HazardBand() {
    Canvas(
        Modifier.fillMaxWidth()
            .height(10.dp)
            .semantics { hideFromAccessibility() }
            .testTag("alert_hazard")
    ) {
        val w = 28.dp.toPx()
        var x = 0f
        var on = true
        while (x < size.width) {
            drawRect(
                if (on) Color(0xFFFFB4AB) else Color(0xFF3A080B),
                Offset(x, 0f),
                Size(w.coerceAtMost(size.width - x), size.height),
            )
            x += w
            on = !on
        }
    }
}

/**
 * The one filled, pinned action of an alert: the palette's slab with its mandatory 2 px light
 * border (text at least 7:1).
 */
@Composable
fun AlertAction(
    text: String,
    onClick: () -> Unit,
    tag: String,
    palette: AlertPalette,
    modifier: Modifier = Modifier,
) {
    val shape = RoundedCornerShape(20.dp)
    Box(
        modifier
            .fillMaxWidth()
            .heightIn(min = 72.dp)
            .background(palette.slab, shape)
            .border(2.dp, palette.slabBorder, shape)
            .clickable(role = Role.Button, onClick = onClick)
            .padding(horizontal = 20.dp, vertical = 12.dp)
            .semantics { role = Role.Button }
            .testTag(tag),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text,
            style = MaterialTheme.typography.titleLarge,
            color = palette.onSlab,
            textAlign = TextAlign.Center,
        )
    }
}
