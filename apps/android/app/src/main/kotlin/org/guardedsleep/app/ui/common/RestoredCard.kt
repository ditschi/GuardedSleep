package org.guardedsleep.app.ui.common

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.model.MissedEventsUi
import org.guardedsleep.app.ui.theme.LocalSafetyColors

/** The three results of the missed-sound report (`connection-status` "Restored notice"). */
enum class MissedResult {
    NONE,
    SOME,
    UNKNOWN,
}

/** An incomplete or absent report never shows a count. */
fun missedResultOf(m: MissedEventsUi?): MissedResult =
    when {
        m == null || !m.complete || m.count == null -> MissedResult.UNKNOWN
        m.count == 0 -> MissedResult.NONE
        else -> MissedResult.SOME
    }

/**
 * "Connected again" (frame 14): the duration in words and the missed-sound result, until the parent
 * taps OK. It never dismisses itself; a new outage replaces it (the platform clears the state).
 */
@Composable
fun RestoredCard(
    restoredAfterMs: Long,
    missed: MissedEventsUi?,
    onOk: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val safety = LocalSafetyColors.current
    val result = missedResultOf(missed)
    Column(
        modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceContainer, RoundedCornerShape(20.dp))
            .padding(16.dp)
            .testTag("restored_card"),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            GlyphIcon(Glyph.CHECK, safety.ok, size = 28.dp)
            Text(
                stringResource(R.string.native_ui_link_alarm_restored_title),
                Modifier.semantics { heading() }.testTag("restored_title"),
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.onSurface,
                fontWeight = FontWeight.Bold,
            )
        }
        Text(
            stringResource(
                R.string.native_ui_conn_alert_restored_body,
                formatDurationWords(restoredAfterMs),
            ),
            Modifier.testTag("restored_body"),
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurface,
        )
        Text(
            when (result) {
                MissedResult.NONE -> stringResource(R.string.native_ui_conn_alert_restored_none)
                MissedResult.SOME ->
                    pluralStringResource(
                        R.plurals.native_ui_conn_alert_restored_some,
                        missed?.count ?: 0,
                        missed?.count ?: 0,
                    )
                MissedResult.UNKNOWN ->
                    stringResource(R.string.native_ui_conn_alert_restored_unknown)
            },
            Modifier.testTag("restored_result"),
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        PrimaryButton(stringResource(R.string.native_ui_common_ok), onOk, "restored_ok")
    }
}
