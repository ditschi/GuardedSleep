package org.guardedsleep.app.ui.common

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.model.LinkUi
import org.guardedsleep.app.ui.theme.LocalSafetyColors

/**
 * The parent's status bar: one component for every parent screen while monitoring
 * (`connection-status` "Parent ambient bar" and "Ambient bar per level"). Connected is neutral with
 * a green dot, so a coloured bar always means a change. Colours of the other stages come from the
 * fixed safety colours, never from the scheme. Test tag `status_bar`.
 */
@Composable
fun StatusBar(
    link: LinkUi,
    name: String?,
    modifier: Modifier = Modifier,
    backgroundProtection: Boolean = true,
    tag: String = "status_bar",
    /**
     * The baby device's battery, shown on the right while it is known ("82%", bolt when charging).
     */
    babyBatteryPct: Int? = null,
    babyCharging: Boolean? = null,
) {
    val elapsed by rememberElapsedNow()
    val kind = if (backgroundProtection) statusKindOf(link) else null
    val style = kind?.style() ?: StatusStyle.ALERT
    val assertive = kind?.assertive() ?: true
    val headline =
        if (kind == null) stringResource(R.string.native_ui_fgs_missing_bar)
        else statusHeadlineText(kind, link, name)
    val sub = kind?.let { statusSubLine(it) }
    val glyph = kind?.glyph() ?: Glyph.BROKEN_LINK
    val loud = kind?.loud() == true
    val safety = LocalSafetyColors.current
    val scheme = MaterialTheme.colorScheme
    val (bg, fg) =
        when (style) {
            StatusStyle.CALM -> scheme.surfaceContainer to scheme.onSurface
            StatusStyle.INFO -> safety.infoContainer to safety.info
            StatusStyle.WARN -> safety.warnContainer to safety.warn
            StatusStyle.ALERT -> safety.alertContainer to safety.alert
            StatusStyle.GREY -> scheme.surfaceContainerHighest to scheme.onSurface
        }
    val shape = RoundedCornerShape(16.dp)
    val pulse = if (loud) pulseAlpha(0.8f, 1f, 2500) else 1f
    val start = link.outageStartElapsedMs()
    Row(
        modifier
            .padding(start = 12.dp, end = 12.dp, top = 4.dp)
            .fillMaxWidth()
            .alpha(pulse)
            .background(bg, shape)
            .then(
                if (style == StatusStyle.WARN) Modifier.border(BorderStroke(2.dp, fg), shape)
                else Modifier
            )
            .heightIn(min = 52.dp)
            .padding(horizontal = 14.dp, vertical = 8.dp)
            .testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(11.dp),
    ) {
        if (style == StatusStyle.CALM) Dot(safety.ok)
        GlyphIcon(glyph, if (style == StatusStyle.CALM) safety.ok else fg, size = 24.dp)
        if (loud || kind == StatusKind.NEEDS_USER || kind == null)
            GlyphIcon(Glyph.EXCLAIM, fg, size = 20.dp)
        // Only the stage words are a live region: the ticking duration beside them never is.
        Column(
            Modifier.weight(1f).semantics(mergeDescendants = true) {
                liveRegion = if (assertive) LiveRegionMode.Assertive else LiveRegionMode.Polite
            }
        ) {
            Text(
                headline,
                style = MaterialTheme.typography.titleMedium,
                color = fg,
                fontWeight = FontWeight.Bold,
            )
            if (sub != null) Text(sub, style = MaterialTheme.typography.bodyMedium, color = fg)
        }
        if (babyBatteryPct != null) {
            val a11y =
                stringResource(
                    if (babyCharging == true) R.string.native_ui_monitor_battery_charging_a11y
                    else R.string.native_ui_monitor_battery_a11y,
                    babyBatteryPct,
                )
            Row(
                Modifier.semantics(mergeDescendants = true) { contentDescription = a11y }
                    .testTag("status_bar_battery"),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                if (babyCharging == true) GlyphIcon(Glyph.BOLT, fg, size = 16.dp)
                Text(
                    stringResource(R.string.native_ui_monitor_battery_short, babyBatteryPct),
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.SemiBold,
                    color = fg,
                    maxLines = 1,
                )
            }
        }
        if (loud && start != null) {
            val down = (elapsed - start).coerceAtLeast(0L)
            val spoken = spokenDuration(down)
            Text(
                formatDurationWords(down),
                Modifier.testTag("status_bar_duration").semantics { contentDescription = spoken },
                style = MaterialTheme.typography.titleSmall,
                color = fg,
            )
        }
    }
}

/**
 * "State · Name" (`redesign-calm-ui` design section 8): the name is never spliced into a sentence.
 */
@Composable
fun statusHeadlineText(kind: StatusKind, link: LinkUi, name: String?): String {
    val state =
        when (kind) {
            StatusKind.CONNECTED -> stringResource(R.string.native_ui_status_connected)
            StatusKind.CONNECTING -> stringResource(R.string.native_ui_status_connecting)
            StatusKind.RECONNECTING -> stringResource(R.string.native_ui_status_reconnecting)
            StatusKind.NOT_CONNECTED_YET ->
                stringResource(R.string.native_ui_status_not_connected_yet)
            StatusKind.CANT_REACH -> stringResource(R.string.native_ui_status_cant_reach)
            StatusKind.COULDNT_CONNECT -> stringResource(R.string.native_ui_status_couldnt_connect)
            StatusKind.NO_CONNECTION -> stringResource(R.string.native_ui_status_no_connection)
            StatusKind.STILL_NO_CONNECTION ->
                stringResource(R.string.native_ui_status_still_no_connection)
            StatusKind.STOPPED -> stringResource(R.string.native_ui_status_stopped)
            StatusKind.NEEDS_USER ->
                return stringResource(
                    if (link.failureKey == "update_required") R.string.core_update_required
                    else R.string.core_pairing_key_mismatch
                )
            StatusKind.IDLE -> return stringResource(R.string.native_ui_link_idle)
        }
    return if (name == null) state
    else stringResource(R.string.native_ui_status_with_name, state, name)
}

@Composable
private fun statusSubLine(kind: StatusKind): String? =
    when (kind) {
        StatusKind.CONNECTING -> stringResource(R.string.native_ui_status_sub_connecting)
        StatusKind.RECONNECTING -> stringResource(R.string.native_ui_status_sub_reconnecting)
        StatusKind.NOT_CONNECTED_YET -> stringResource(R.string.native_ui_status_sub_not_answered)
        StatusKind.CANT_REACH -> stringResource(R.string.native_ui_status_sub_still_trying)
        StatusKind.STOPPED -> stringResource(R.string.native_ui_status_sub_stopped)
        else -> null
    }
