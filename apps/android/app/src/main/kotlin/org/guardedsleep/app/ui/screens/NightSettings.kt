package org.guardedsleep.app.ui.screens

import android.text.format.DateFormat
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TimePicker
import androidx.compose.material3.rememberTimePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import java.util.Calendar
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.common.AppCard
import org.guardedsleep.app.ui.common.Body
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.common.formatTime
import org.guardedsleep.app.ui.common.rememberWallNow
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.NightMode
import org.guardedsleep.app.ui.model.NightSchedule
import org.guardedsleep.app.ui.model.SettingsState
import org.guardedsleep.app.ui.theme.LocalAppColors

/**
 * Settings › Display › Screen at night (`ui-appearance` "Night mode", "Automatic night mode by
 * time"): Off, On or Automatic, and the daily window under Automatic. The choice is stored on the
 * device and never touches the core; alerts and connection problems keep their colours.
 */
@Composable
internal fun NightSection(controller: AppController, s: SettingsState) {
    val env = LocalUiEnv.current
    val wall by rememberWallNow()
    AppCard(Modifier.testTag("settings_group_night")) {
        Text(
            stringResource(R.string.native_ui_settings_night_group),
            Modifier.semantics { heading() },
            style = MaterialTheme.typography.titleMedium,
            color = LocalAppColors.current.secondaryText,
        )
        Text(
            stringResource(R.string.native_ui_settings_night_title),
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurface,
        )
        Body(stringResource(R.string.native_ui_settings_night_hint))
        NightMode.entries.forEach { mode ->
            ChoiceRow(
                nightModeLabel(mode),
                s.nightMode == mode,
                "settings_night_" + mode.name.lowercase(),
            ) {
                controller.updateSettings { st -> NightSchedule.withMode(st, mode) }
            }
        }
        if (s.nightMode == NightMode.AUTO) {
            Row(
                Modifier.fillMaxWidth().testTag("settings_night_window"),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Text(
                    stringResource(R.string.native_ui_settings_night_between),
                    Modifier.weight(1f),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                TimeChip(
                    s.nightFromMinute,
                    R.string.native_ui_settings_night_from_a11y,
                    "settings_night_from",
                ) { m ->
                    controller.updateSettings { st ->
                        NightSchedule.withWindow(st, m, st.nightToMinute)
                    }
                }
                // A drawn dash: a literal "–" in UI code would bypass the catalogue.
                Box(
                    Modifier.size(width = 8.dp, height = 1.dp)
                        .background(MaterialTheme.colorScheme.onSurface)
                )
                TimeChip(
                    s.nightToMinute,
                    R.string.native_ui_settings_night_until_a11y,
                    "settings_night_to",
                ) { m ->
                    controller.updateSettings { st ->
                        NightSchedule.withWindow(st, st.nightFromMinute, m)
                    }
                }
            }
        }
        Body(nightStatus(s, wall, env.zone.getOffset(wall).toLong()), tag = "settings_night_status")
        Body(stringResource(R.string.native_ui_settings_night_note), tag = "settings_night_note")
    }
}

@Composable
internal fun nightModeLabel(mode: NightMode): String =
    stringResource(
        when (mode) {
            NightMode.OFF -> R.string.native_ui_settings_night_off
            NightMode.ON -> R.string.native_ui_settings_night_on
            NightMode.AUTO -> R.string.native_ui_settings_night_auto
        }
    )

/** "Now off · turns on at 21:00": the state and, with Automatic, the next change. */
@Composable
private fun nightStatus(s: SettingsState, wall: Long, offsetMs: Long): String {
    val active = NightSchedule.isActive(s, wall, offsetMs)
    val next =
        if (s.nightMode == NightMode.AUTO)
            NightSchedule.nextBoundaryWallMs(s.nightFromMinute, s.nightToMinute, wall, offsetMs)
        else null
    return when {
        next == null ->
            stringResource(
                if (active) R.string.native_ui_settings_night_status_on
                else R.string.native_ui_settings_night_status_off
            )
        active ->
            stringResource(R.string.native_ui_settings_night_status_on_until, formatTime(next))
        else -> stringResource(R.string.native_ui_settings_night_status_off_until, formatTime(next))
    }
}

/** A minute of the day as a local clock time in the user's locale (today's date, no zone maths). */
@Composable
private fun formatMinuteOfDay(minute: Int): String {
    val zone = LocalUiEnv.current.zone
    val at =
        Calendar.getInstance(zone)
            .apply {
                set(Calendar.HOUR_OF_DAY, minute / 60)
                set(Calendar.MINUTE, minute % 60)
                set(Calendar.SECOND, 0)
                set(Calendar.MILLISECOND, 0)
            }
            .timeInMillis
    return formatTime(at)
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun TimeChip(minute: Int, a11y: Int, tag: String, onPick: (Int) -> Unit) {
    var open by remember { mutableStateOf(false) }
    val shown = formatMinuteOfDay(minute)
    val label = stringResource(a11y, shown)
    TextButton(
        onClick = { open = true },
        Modifier.testTag(tag).semantics { contentDescription = label },
    ) {
        Text(shown, style = MaterialTheme.typography.titleMedium)
    }
    if (open) {
        val state =
            rememberTimePickerState(
                initialHour = minute / 60,
                initialMinute = minute % 60,
                is24Hour = DateFormat.is24HourFormat(LocalContext.current),
            )
        AlertDialog(
            onDismissRequest = { open = false },
            text = { TimePicker(state, Modifier.testTag("${tag}_picker")) },
            confirmButton = {
                TextButton(
                    onClick = {
                        onPick(state.hour * 60 + state.minute)
                        open = false
                    },
                    Modifier.testTag("${tag}_ok"),
                ) {
                    Text(stringResource(R.string.native_ui_common_ok))
                }
            },
            dismissButton = {
                TextButton(onClick = { open = false }, Modifier.testTag("${tag}_cancel")) {
                    Text(stringResource(R.string.native_ui_common_cancel))
                }
            },
        )
    }
}
