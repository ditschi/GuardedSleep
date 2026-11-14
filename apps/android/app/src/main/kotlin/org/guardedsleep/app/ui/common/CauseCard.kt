package org.guardedsleep.app.ui.common

import androidx.annotation.StringRes
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.collapse
import androidx.compose.ui.semantics.expand
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.model.CauseHint
import org.guardedsleep.app.ui.theme.AlertPalette

/** One cause: its line, the first step (always visible) and the further steps (folded). */
private data class CauseCopy(
    @StringRes val line: Int?,
    @StringRes val step: Int,
    val more: List<Int>,
)

private fun causeCopyOf(hint: CauseHint?): CauseCopy =
    when (hint) {
        CauseHint.PEER_ABSENT ->
            CauseCopy(
                R.string.native_ui_conn_alert_cause_peer_absent_line,
                R.string.native_ui_conn_alert_cause_peer_absent_step,
                listOf(R.string.native_ui_conn_alert_cause_peer_absent_more),
            )
        CauseHint.SERVER_UNREACHABLE ->
            CauseCopy(
                R.string.native_ui_conn_alert_cause_server_unreachable_line,
                R.string.native_ui_conn_alert_cause_server_unreachable_step,
                listOf(R.string.native_ui_conn_alert_cause_server_unreachable_more),
            )
        CauseHint.MEDIA_BLOCKED ->
            CauseCopy(
                R.string.native_ui_conn_alert_cause_media_blocked_line,
                R.string.native_ui_conn_alert_cause_media_blocked_step,
                listOf(R.string.native_ui_conn_alert_cause_media_blocked_more),
            )
        // Pairing causes name the fix in the core's own message; there is nothing to fold.
        CauseHint.PAIR_AGAIN -> CauseCopy(null, R.string.core_pair_again, emptyList())
        CauseHint.BABY_UNRECOGNISED -> CauseCopy(null, R.string.core_baby_unrecognised, emptyList())
        null ->
            CauseCopy(
                null,
                R.string.native_ui_help_reconnect_step1,
                listOf(
                    R.string.native_ui_help_reconnect_step2,
                    R.string.native_ui_help_reconnect_step3,
                ),
            )
    }

/**
 * The troubleshooter card of a connection alert (`connection-status` "Troubleshooter
 * presentation"): one cause line, step 1 visible, "Try now" and further steps behind a disclosure
 * button that carries its expanded state. Focus moves to the first extra step when it opens.
 */
@Composable
fun CauseCard(
    hint: CauseHint?,
    palette: AlertPalette,
    onTryNow: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val copy = causeCopyOf(hint)
    var open by remember { mutableStateOf(false) }
    val firstMore = remember { FocusRequester() }
    LaunchedEffect(open) { if (open) runCatching { firstMore.requestFocus() } }
    Column(
        modifier
            .fillMaxWidth()
            .background(palette.surface, RoundedCornerShape(16.dp))
            .padding(16.dp)
            .testTag("link_alarm_cause_card"),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        if (copy.line != null) {
            Text(
                stringResource(R.string.native_ui_conn_alert_cause_label).uppercase(),
                style = MaterialTheme.typography.labelMedium,
                color = palette.accent,
            )
            Text(
                stringResource(copy.line),
                Modifier.testTag("link_alarm_cause"),
                style = MaterialTheme.typography.titleMedium,
                color = palette.onBackground,
            )
        }
        Text(
            stringResource(copy.step),
            Modifier.testTag("link_alarm_step"),
            style = MaterialTheme.typography.bodyLarge,
            color = palette.onVariant,
        )
        if (open) {
            copy.more.forEachIndexed { i, res ->
                Text(
                    stringResource(res),
                    (if (i == 0) Modifier.focusRequester(firstMore).focusable() else Modifier)
                        .testTag("link_alarm_more_$i"),
                    style = MaterialTheme.typography.bodyLarge,
                    color = palette.onVariant,
                )
            }
        }
        SecondaryButton(
            stringResource(R.string.native_ui_link_try_now),
            onTryNow,
            "link_alarm_try_now",
            Modifier.fillMaxWidth(),
        )
        if (copy.more.isNotEmpty()) {
            QuietButton(
                stringResource(
                    if (open) R.string.native_ui_conn_alert_fewer_steps
                    else R.string.native_ui_conn_alert_more_steps
                ),
                { open = !open },
                "link_alarm_more_steps",
                Modifier.semantics {
                    // A disclosure button: the screen reader hears whether it is open.
                    if (open) collapse { true } else expand { true }
                },
            )
        }
    }
}
