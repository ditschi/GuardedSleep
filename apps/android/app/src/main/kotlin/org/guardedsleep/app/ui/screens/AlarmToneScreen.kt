package org.guardedsleep.app.ui.screens

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.selection.selectable
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role as SemRole
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.common.AppCard
import org.guardedsleep.app.ui.common.Body
import org.guardedsleep.app.ui.common.Glyph
import org.guardedsleep.app.ui.common.GlyphButton
import org.guardedsleep.app.ui.common.ScreenRoot
import org.guardedsleep.app.ui.common.ScrollColumn
import org.guardedsleep.app.ui.common.SecondaryButton
import org.guardedsleep.app.ui.model.AlarmTonePlayback
import org.guardedsleep.app.ui.model.AlarmTones
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.theme.LocalAppColors

private class ToneCopy(val name: Int, val desc: Int)

/** Name and description strings per tone id, keyed by the contract's [AlarmTones.IDS]. */
private val ToneStrings: Map<String, ToneCopy> =
    mapOf(
        "classic" to
            ToneCopy(
                R.string.native_ui_settings_alarm_tone_name_classic,
                R.string.native_ui_settings_alarm_tone_desc_classic,
            ),
        "low" to
            ToneCopy(
                R.string.native_ui_settings_alarm_tone_name_low,
                R.string.native_ui_settings_alarm_tone_desc_low,
            ),
        "beacon" to
            ToneCopy(
                R.string.native_ui_settings_alarm_tone_name_beacon,
                R.string.native_ui_settings_alarm_tone_desc_beacon,
            ),
        "rising" to
            ToneCopy(
                R.string.native_ui_settings_alarm_tone_name_rising,
                R.string.native_ui_settings_alarm_tone_desc_rising,
            ),
        "chime" to
            ToneCopy(
                R.string.native_ui_settings_alarm_tone_name_chime,
                R.string.native_ui_settings_alarm_tone_desc_chime,
            ),
    )

private fun toneCopy(id: String): ToneCopy =
    ToneStrings[id] ?: checkNotNull(ToneStrings[AlarmTones.DEFAULT])

/** The localized name of a connection alarm tone id (an unknown id shows the default's name). */
@Composable internal fun alarmToneName(id: String): String = stringResource(toneCopy(id).name)

@Composable private fun alarmToneDescription(id: String): String = stringResource(toneCopy(id).desc)

/**
 * Settings > Alerts > Connection alarm tone (parent only): one radio row per built-in tone with a
 * preview button, the explanation that noise alerts keep the child's sound, and the full-volume
 * test. Leaving the screen stops any preview or test.
 */
@Composable
fun AlarmTonePickerScreen(controller: AppController, onBack: () -> Unit) {
    val settings by controller.settings.collectAsState()
    val playback by controller.alarmTonePlayback.collectAsState()
    DisposableEffect(Unit) { onDispose { controller.stopAlarmTone() } }
    BackHandler(onBack = onBack)
    ScreenRoot("alarm_tone_picker") {
        ScrollColumn(verticalArrangement = Arrangement.spacedBy(14.dp)) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                GlyphButton(
                    Glyph.BACK,
                    stringResource(R.string.native_ui_common_back),
                    onBack,
                    "alarm_tone_back",
                )
                Text(
                    stringResource(R.string.native_ui_settings_alarm_tone_label),
                    Modifier.testTag("alarm_tone_title").semantics { heading() },
                    style = MaterialTheme.typography.headlineMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onBackground,
                )
            }
            Body(
                stringResource(R.string.native_ui_settings_alarm_tone_explanation),
                tag = "alarm_tone_explanation",
            )
            AlarmToneRows(
                selected = settings.connectionAlarmTone,
                playback = playback,
                onSelect = { id ->
                    controller.updateSettings { st -> st.copy(connectionAlarmTone = id) }
                },
                onPlay = controller::previewAlarmTone,
                onStop = controller::stopAlarmTone,
            )
            val testing = playback is AlarmTonePlayback.Test
            AppCard {
                Body(
                    stringResource(R.string.native_ui_settings_alarm_tone_test_warning),
                    tag = "alarm_tone_test_warning",
                    color = MaterialTheme.colorScheme.onSurface,
                )
                SecondaryButton(
                    stringResource(
                        if (testing) R.string.native_ui_settings_alarm_tone_stop
                        else R.string.native_ui_settings_alarm_tone_test
                    ),
                    {
                        if (testing) controller.stopAlarmTone()
                        else controller.testAlarmTone(settings.connectionAlarmTone)
                    },
                    if (testing) "alarm_tone_test_stop" else "alarm_tone_test",
                )
            }
        }
    }
}

@Composable
private fun AlarmToneRows(
    selected: String,
    playback: AlarmTonePlayback,
    onSelect: (String) -> Unit,
    onPlay: (String) -> Unit,
    onStop: () -> Unit,
) {
    AppCard {
        AlarmTones.IDS.forEach { id ->
            val name = alarmToneName(id)
            val playing = playback is AlarmTonePlayback.Preview && playback.id == id
            Row(
                Modifier.fillMaxWidth().heightIn(min = 56.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Row(
                    Modifier.weight(1f)
                        .heightIn(min = 56.dp)
                        .selectable(selected == id, role = SemRole.RadioButton) { onSelect(id) }
                        .testTag("alarm_tone_$id"),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    RadioButton(selected == id, onClick = null)
                    Column {
                        Text(
                            name,
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.onSurface,
                        )
                        if (id == AlarmTones.HARD_OF_HEARING) {
                            Text(
                                stringResource(
                                    R.string.native_ui_settings_alarm_tone_tag_hard_of_hearing
                                ),
                                Modifier.testTag("alarm_tone_tag_$id"),
                                style = MaterialTheme.typography.labelLarge,
                                color = MaterialTheme.colorScheme.primary,
                            )
                        }
                        Text(
                            alarmToneDescription(id),
                            style = MaterialTheme.typography.bodyMedium,
                            color = LocalAppColors.current.secondaryText,
                        )
                    }
                }
                val play = stringResource(R.string.native_ui_settings_alarm_tone_play_named, name)
                SecondaryButton(
                    stringResource(
                        if (playing) R.string.native_ui_settings_alarm_tone_stop
                        else R.string.native_ui_settings_alarm_tone_play
                    ),
                    { if (playing) onStop() else onPlay(id) },
                    "alarm_tone_preview_$id",
                    Modifier.semantics { if (!playing) contentDescription = play },
                )
            }
            if (id == AlarmTones.HARD_OF_HEARING) {
                Body(
                    stringResource(R.string.native_ui_settings_alarm_tone_hint_hard_of_hearing),
                    tag = "alarm_tone_hint",
                )
            }
        }
    }
}
