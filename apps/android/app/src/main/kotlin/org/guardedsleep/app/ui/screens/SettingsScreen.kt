package org.guardedsleep.app.ui.screens

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role as SemRole
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.common.AppCard
import org.guardedsleep.app.ui.common.Body
import org.guardedsleep.app.ui.common.ConfirmDialog
import org.guardedsleep.app.ui.common.Glyph
import org.guardedsleep.app.ui.common.GlyphIcon
import org.guardedsleep.app.ui.common.ScreenRoot
import org.guardedsleep.app.ui.common.ScrollColumn
import org.guardedsleep.app.ui.common.SecondaryButton
import org.guardedsleep.app.ui.common.formatTime
import org.guardedsleep.app.ui.common.rememberLanguageChoices
import org.guardedsleep.app.ui.model.AlertOutput
import org.guardedsleep.app.ui.model.AlertStyle
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.DiscreetEscalation
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.Theme
import org.guardedsleep.app.ui.model.TurnSaveResult
import org.guardedsleep.app.ui.theme.LocalAppColors

private val HoldOptions = listOf(3, 5, 10, 15)

/**
 * Build identity shown in Settings. Passed in from the wiring (Entry.kt) so screenshots and tests
 * use a fixed value instead of the commit of the build under test.
 */
data class BuildInfo(val version: String, val commit: String, val commitTime: String) {
    companion object {
        val Preview = BuildInfo("0.1.0-alpha.1", "0123abcd", "2026-01-01 12:00 UTC")
    }
}

/** S12: Basic settings per role; Advanced only behind a warning. */
@Composable
fun SettingsScreen(
    controller: AppController,
    onBack: () -> Unit,
    onOpenSetup: () -> Unit,
    onTestAlert: () -> Unit,
    buildInfo: BuildInfo = BuildInfo.Preview,
) {
    val s = controller.settings.collectAsState().value
    val app = controller.app.collectAsState().value
    val b = controller.baby.collectAsState().value
    val isBaby = app.role == Role.BABY
    var confirmAdvanced by remember { mutableStateOf(false) }
    var confirmForget by remember { mutableStateOf(false) }
    val peer = peerDisplayName(app.peerName, parentSide = !isBaby)
    BackHandler(onBack = onBack)
    ScreenRoot("settings") {
        ScrollColumn(verticalArrangement = Arrangement.spacedBy(14.dp)) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Row(
                    Modifier.size(48.dp)
                        .clickable(role = SemRole.Button, onClick = onBack)
                        .testTag("settings_back"),
                    horizontalArrangement = Arrangement.Center,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    GlyphIcon(Glyph.BACK, MaterialTheme.colorScheme.onBackground)
                }
                Text(
                    stringResource(R.string.native_ui_common_settings),
                    Modifier.testTag("settings_title").semantics { heading() },
                    style = MaterialTheme.typography.headlineMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onBackground,
                )
            }

            if (isBaby) {
                Section(
                    R.string.native_ui_settings_group_sound_detection,
                    "settings_group_detection",
                ) {
                    var th by remember(s.threshold) { mutableFloatStateOf(s.threshold) }
                    Body(
                        stringResource(R.string.native_ui_settings_sensitivity),
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                    MicLevelBar(b.micLevel, th, tag = "settings_mic_bar")
                    Slider(
                        value = th,
                        onValueChange = { th = it },
                        onValueChangeFinished = {
                            controller.updateSettings { st -> st.copy(threshold = th) }
                        },
                        valueRange = 0.03f..0.4f,
                        modifier = Modifier.testTag("settings_threshold"),
                    )
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                        Body(stringResource(R.string.native_ui_settings_more_sensitive))
                        Body(stringResource(R.string.native_ui_settings_less_sensitive))
                    }
                    Body(
                        stringResource(R.string.native_ui_settings_hold_label),
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        HoldOptions.forEach { sec ->
                            ChoiceChip(
                                stringResource(
                                    R.string.native_ui_common_seconds_short,
                                    sec.toString(),
                                ),
                                s.holdSeconds == sec,
                                "settings_hold_$sec",
                            ) {
                                controller.updateSettings { st -> st.copy(holdSeconds = sec) }
                            }
                        }
                    }
                }
                Section(R.string.native_ui_settings_group_silence, "settings_group_silence") {
                    SwitchRow(
                        stringResource(R.string.native_ui_setup_silence_title),
                        s.silenceWhileMonitoring,
                        "settings_silence",
                    ) { on ->
                        controller.updateSettings { st -> st.copy(silenceWhileMonitoring = on) }
                    }
                    Body(
                        stringResource(
                            if (b.dndAccess) R.string.native_ui_settings_silence_access_ok
                            else R.string.native_ui_baby_silence_no_access
                        ),
                        tag = "settings_silence_status",
                    )
                }
            } else {
                Section(R.string.native_ui_settings_group_alerts, "settings_group_alerts") {
                    AlertStyleChoice(s.alertStyle) { style ->
                        controller.updateSettings { st -> st.copy(alertStyle = style) }
                    }
                    if (s.alertStyle == AlertStyle.DISCREET) {
                        DiscreetEscalationChoice(s.discreetEscalation) { choice ->
                            controller.updateSettings { st -> st.copy(discreetEscalation = choice) }
                        }
                    }
                    Body(
                        stringResource(R.string.native_ui_settings_output_label),
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                    ChoiceRow(
                        stringResource(R.string.native_ui_settings_output_default),
                        s.alertOutput == AlertOutput.DEFAULT,
                        "settings_output_default",
                    ) {
                        controller.updateSettings { st ->
                            st.copy(alertOutput = AlertOutput.DEFAULT)
                        }
                    }
                    ChoiceRow(
                        stringResource(R.string.native_ui_settings_output_speaker),
                        s.alertOutput == AlertOutput.SPEAKER_ONLY,
                        "settings_output_speaker",
                    ) {
                        controller.updateSettings { st ->
                            st.copy(alertOutput = AlertOutput.SPEAKER_ONLY)
                        }
                    }
                    var gain by
                        remember(s.alertGainPct) { mutableFloatStateOf(s.alertGainPct.toFloat()) }
                    Body(
                        stringResource(R.string.native_ui_settings_gain, gain.toInt().toString()),
                        tag = "settings_gain_label",
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                    Slider(
                        value = gain,
                        onValueChange = { gain = it },
                        onValueChangeFinished = {
                            controller.updateSettings { st -> st.copy(alertGainPct = gain.toInt()) }
                        },
                        valueRange = 25f..150f,
                        modifier = Modifier.testTag("settings_gain"),
                    )
                    SecondaryButton(
                        stringResource(R.string.native_ui_probe_button),
                        onTestAlert,
                        "settings_test_alert",
                    )
                    Body(
                        s.lastTestWallMs?.let {
                            stringResource(R.string.native_ui_settings_last_test, formatTime(it))
                        } ?: stringResource(R.string.native_ui_settings_never_tested),
                        tag = "settings_last_test",
                    )
                }
                Section(R.string.native_ui_settings_group_screen, "settings_group_screen") {
                    ChoiceRow(
                        stringResource(R.string.native_ui_settings_theme_dark),
                        s.theme == Theme.DARK,
                        "settings_theme_dark",
                    ) {
                        controller.updateSettings { st -> st.copy(theme = Theme.DARK) }
                    }
                    ChoiceRow(
                        stringResource(R.string.native_ui_settings_theme_light),
                        s.theme == Theme.LIGHT,
                        "settings_theme_light",
                    ) {
                        controller.updateSettings { st -> st.copy(theme = Theme.LIGHT) }
                    }
                    ChoiceRow(
                        stringResource(R.string.native_ui_settings_theme_system),
                        s.theme == Theme.SYSTEM,
                        "settings_theme_system",
                    ) {
                        controller.updateSettings { st -> st.copy(theme = Theme.SYSTEM) }
                    }
                }
            }

            Section(R.string.native_ui_settings_group_paired, "settings_group_paired") {
                if (app.paired) {
                    Body(
                        peer,
                        tag = "settings_peer_name",
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                    SecondaryButton(
                        stringResource(
                            if (isBaby) R.string.native_ui_settings_forget_parent
                            else R.string.native_ui_settings_forget_baby
                        ),
                        { confirmForget = true },
                        "settings_forget",
                        Modifier.fillMaxWidth(),
                        destructive = true,
                    )
                } else {
                    Body(stringResource(R.string.native_ui_home_empty), tag = "settings_not_paired")
                }
            }
            Section(R.string.native_ui_settings_group_this_phone, "settings_group_phone") {
                SecondaryButton(
                    stringResource(R.string.native_ui_settings_check_phone),
                    onOpenSetup,
                    "settings_open_setup",
                    Modifier.fillMaxWidth(),
                )
            }
            Section(R.string.native_ui_settings_group_start_mode, "settings_group_start_mode") {
                // Which role a start without running monitoring opens in; null asks every time.
                ChoiceRow(
                    stringResource(R.string.native_ui_settings_start_mode_ask),
                    app.defaultRole == null,
                    "start_mode_ask",
                ) {
                    controller.setDefaultRole(null)
                }
                ChoiceRow(
                    stringResource(R.string.native_ui_role_baby_title),
                    app.defaultRole == Role.BABY,
                    "start_mode_baby",
                ) {
                    controller.setDefaultRole(Role.BABY)
                }
                ChoiceRow(
                    stringResource(R.string.native_ui_role_parent_title),
                    app.defaultRole == Role.PARENT,
                    "start_mode_parent",
                ) {
                    controller.setDefaultRole(Role.PARENT)
                }
            }
            Section(R.string.native_ui_settings_group_language, "settings_group_language") {
                // "Match system" is translated; the languages are shown by endonym (generated).
                val langs = rememberLanguageChoices().map { it.tag to it.endonym }
                ChoiceRow(
                    stringResource(R.string.native_ui_settings_lang_system),
                    s.language == null,
                    "settings_lang_system",
                ) {
                    controller.updateSettings { st -> st.copy(language = null) }
                }
                langs.forEach { (code, name) ->
                    ChoiceRow(name, s.language == code, "settings_lang_$code") {
                        controller.updateSettings { st -> st.copy(language = code) }
                    }
                }
            }
            Section(R.string.native_ui_settings_group_about, "settings_group_about") {
                Body(stringResource(R.string.native_ui_setup_limits_body), tag = "settings_limits")
                Body(
                    stringResource(
                        R.string.native_ui_settings_version,
                        buildInfo.version,
                        buildInfo.commit,
                        buildInfo.commitTime,
                    ),
                    tag = "settings_version",
                )
            }

            Section(R.string.native_ui_settings_group_advanced, "settings_group_advanced") {
                SwitchRow(
                    stringResource(R.string.native_ui_settings_advanced_toggle),
                    s.advancedEnabled,
                    "settings_advanced_toggle",
                ) { on ->
                    if (on) confirmAdvanced = true
                    else controller.updateSettings { st -> st.copy(advancedEnabled = false) }
                }
                if (s.advancedEnabled)
                    AdvancedFields(
                        controller,
                        s.signalingUrls,
                        s.turnUrl,
                        s.turnUsername,
                        s.turnPasswordSaved,
                        s.heartbeatSeconds,
                    )
            }
        }
        if (confirmAdvanced) {
            ConfirmDialog(
                tag = "dialog_advanced",
                title = stringResource(R.string.native_ui_settings_advanced_warning_title),
                body = stringResource(R.string.native_ui_settings_advanced_warning_body),
                confirmText = stringResource(R.string.native_ui_settings_advanced_confirm),
                cancelText = stringResource(R.string.native_ui_settings_advanced_cancel),
                confirmTag = "dialog_advanced_confirm",
                cancelTag = "dialog_advanced_cancel",
                onConfirm = {
                    confirmAdvanced = false
                    controller.updateSettings { st -> st.copy(advancedEnabled = true) }
                },
                onDismiss = { confirmAdvanced = false },
                destructive = false,
            )
        }
        if (confirmForget) {
            ConfirmDialog(
                tag = "dialog_forget",
                title =
                    customPeerName(app.peerName)?.let {
                        stringResource(R.string.native_ui_forget_title, it)
                    }
                        ?: stringResource(
                            if (isBaby) R.string.native_ui_forget_title_parent_default
                            else R.string.native_ui_forget_title_baby_default
                        ),
                body = stringResource(R.string.native_ui_forget_body),
                confirmText = stringResource(R.string.native_ui_forget_confirm),
                cancelText = stringResource(R.string.native_ui_common_cancel),
                confirmTag = "dialog_forget_confirm",
                cancelTag = "dialog_forget_cancel",
                onConfirm = {
                    confirmForget = false
                    controller.forgetPairing()
                },
                onDismiss = { confirmForget = false },
            )
        }
    }
}

@Composable
private fun AdvancedFields(
    controller: AppController,
    urls: List<String>,
    turn: String?,
    turnUsername: String?,
    turnPasswordSaved: Boolean,
    heartbeat: Int,
) {
    var newUrl by remember { mutableStateOf("") }
    var urlError by remember { mutableStateOf(false) }
    var turnText by remember(turn) { mutableStateOf(turn.orEmpty()) }
    var turnUser by remember(turnUsername) { mutableStateOf(turnUsername.orEmpty()) }
    // Plain remember, not rememberSaveable: the password must not reach saved UI state.
    var turnPass by remember { mutableStateOf("") }
    var turnResult by remember { mutableStateOf<TurnSaveResult?>(null) }
    var hb by remember(heartbeat) { mutableFloatStateOf(heartbeat.toFloat()) }
    Body(
        stringResource(R.string.native_ui_settings_urls_label),
        color = MaterialTheme.colorScheme.onSurface,
    )
    urls.forEachIndexed { i, url ->
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                url,
                Modifier.weight(1f).testTag("settings_url_$i"),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurface,
            )
            SecondaryButton(
                stringResource(R.string.native_ui_common_remove),
                {
                    controller.updateSettings { st ->
                        st.copy(signalingUrls = st.signalingUrls.filterIndexed { j, _ -> j != i })
                    }
                },
                "settings_url_remove_$i",
            )
        }
    }
    OutlinedTextField(
        value = newUrl,
        onValueChange = {
            newUrl = it
            urlError = false
        },
        label = { Text(stringResource(R.string.native_ui_settings_url_new)) },
        isError = urlError,
        supportingText =
            if (urlError) {
                { Text(stringResource(R.string.native_ui_settings_url_invalid)) }
            } else null,
        singleLine = true,
        modifier = Modifier.fillMaxWidth().testTag("settings_url_new"),
    )
    SecondaryButton(
        stringResource(R.string.native_ui_common_add),
        {
            val v = newUrl.trim()
            if (Regex("^wss?://\\S+$").matches(v)) {
                controller.updateSettings { st -> st.copy(signalingUrls = st.signalingUrls + v) }
                newUrl = ""
            } else urlError = true
        },
        "settings_url_add",
    )
    OutlinedTextField(
        value = turnText,
        onValueChange = {
            turnText = it
            turnResult = null
        },
        label = { Text(stringResource(R.string.native_ui_settings_turn_label)) },
        isError = turnResult == TurnSaveResult.INVALID_ADDRESS,
        supportingText =
            if (turnResult == TurnSaveResult.INVALID_ADDRESS) {
                { Text(stringResource(R.string.native_ui_settings_turn_invalid)) }
            } else null,
        singleLine = true,
        modifier = Modifier.fillMaxWidth().testTag("settings_turn"),
    )
    OutlinedTextField(
        value = turnUser,
        onValueChange = {
            turnUser = it
            turnResult = null
        },
        label = { Text(stringResource(R.string.native_ui_settings_turn_username)) },
        singleLine = true,
        modifier = Modifier.fillMaxWidth().testTag("settings_turn_user"),
    )
    val credentialsError = turnResult == TurnSaveResult.INVALID_CREDENTIALS
    OutlinedTextField(
        value = turnPass,
        onValueChange = {
            turnPass = it
            turnResult = null
        },
        label = { Text(stringResource(R.string.native_ui_settings_turn_password)) },
        isError = credentialsError,
        supportingText =
            when {
                credentialsError -> {
                    { Text(stringResource(R.string.native_ui_settings_turn_credentials_invalid)) }
                }
                turnPasswordSaved -> {
                    { Text(stringResource(R.string.native_ui_settings_turn_password_saved)) }
                }
                else -> null
            },
        visualTransformation = PasswordVisualTransformation(),
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
        singleLine = true,
        modifier = Modifier.fillMaxWidth().testTag("settings_turn_pass"),
    )
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        SecondaryButton(
            stringResource(R.string.native_ui_common_save),
            {
                val r = controller.setTurnServer(turnText, turnUser, turnPass)
                turnResult = r.takeIf { it != TurnSaveResult.SAVED && it != TurnSaveResult.CLEARED }
                if (turnResult == null) turnPass = ""
            },
            "settings_turn_save",
        )
        if (turn != null) {
            SecondaryButton(
                stringResource(R.string.native_ui_common_remove),
                {
                    controller.clearTurnServer()
                    turnPass = ""
                    turnResult = null
                },
                "settings_turn_remove",
            )
        }
    }
    Body(
        stringResource(R.string.native_ui_settings_heartbeat, hb.toInt().toString()),
        tag = "settings_heartbeat_label",
        color = MaterialTheme.colorScheme.onSurface,
    )
    Slider(
        value = hb,
        onValueChange = { hb = it },
        onValueChangeFinished = {
            controller.updateSettings { st -> st.copy(heartbeatSeconds = hb.toInt()) }
        },
        valueRange = 5f..15f,
        steps = 9,
        modifier = Modifier.testTag("settings_heartbeat"),
    )
}

@Composable
private fun Section(title: Int, tag: String, content: @Composable () -> Unit) {
    AppCard(Modifier.testTag(tag)) {
        Text(
            stringResource(title),
            Modifier.semantics { heading() },
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.SemiBold,
            color = LocalAppColors.current.secondaryText,
        )
        content()
    }
}

@Composable
internal fun ChoiceRow(label: String, selected: Boolean, tag: String, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth()
            .heightIn(min = 48.dp)
            .selectable(selected, role = SemRole.RadioButton, onClick = onClick)
            .testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        RadioButton(selected, onClick = null)
        Text(
            label,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurface,
        )
    }
}

@Composable
private fun ChoiceChip(label: String, selected: Boolean, tag: String, onClick: () -> Unit) {
    val bg = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surface
    val fg =
        if (selected) MaterialTheme.colorScheme.onPrimary else MaterialTheme.colorScheme.onSurface
    Row(
        Modifier.heightIn(min = 48.dp)
            .androidxBg(bg)
            .selectable(selected, role = SemRole.RadioButton, onClick = onClick)
            .padding(horizontal = 16.dp)
            .testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(label, color = fg, style = MaterialTheme.typography.titleMedium)
    }
}

private fun Modifier.androidxBg(c: androidx.compose.ui.graphics.Color): Modifier =
    this.then(Modifier.background(c, androidx.compose.foundation.shape.RoundedCornerShape(24.dp)))

@Composable
internal fun SwitchRow(label: String, checked: Boolean, tag: String, onChange: (Boolean) -> Unit) {
    Row(
        Modifier.fillMaxWidth()
            .heightIn(min = 48.dp)
            .toggleable(checked, role = SemRole.Switch, onValueChange = onChange)
            .testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(
            label,
            Modifier.weight(1f),
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurface,
        )
        Switch(checked, onCheckedChange = null)
    }
}

/** The two alert styles with the ids shared with the web client; used in Settings and More. */
@Composable
internal fun AlertStyleChoice(style: AlertStyle, onChoose: (AlertStyle) -> Unit) {
    Body(
        stringResource(R.string.native_ui_settings_alert_style_label),
        color = MaterialTheme.colorScheme.onSurface,
    )
    val options =
        listOf(
            Triple(
                AlertStyle.SOUND,
                R.string.native_ui_settings_alert_style_sound,
                "alert_style_sound",
            ),
            Triple(
                AlertStyle.DISCREET,
                R.string.native_ui_settings_alert_style_discreet,
                "alert_style_discreet",
            ),
        )
    for ((value, label, tag) in options) {
        ChoiceRow(stringResource(label), style == value, tag) { onChoose(value) }
    }
}

/** After how long an unacknowledged discreet noise event switches to sound; Off warns. */
@Composable
private fun DiscreetEscalationChoice(
    choice: DiscreetEscalation,
    onChoose: (DiscreetEscalation) -> Unit,
) {
    Body(
        stringResource(R.string.native_ui_settings_discreet_escalation_label),
        color = MaterialTheme.colorScheme.onSurface,
    )
    val options =
        listOf(
            Triple(
                DiscreetEscalation.ONE_MINUTE,
                R.string.native_ui_settings_discreet_escalation_m1,
                "discreet_escalation_1m",
            ),
            Triple(
                DiscreetEscalation.TWO_MINUTES,
                R.string.native_ui_settings_discreet_escalation_m2,
                "discreet_escalation_2m",
            ),
            Triple(
                DiscreetEscalation.FIVE_MINUTES,
                R.string.native_ui_settings_discreet_escalation_m5,
                "discreet_escalation_5m",
            ),
            Triple(
                DiscreetEscalation.OFF,
                R.string.native_ui_settings_discreet_escalation_off,
                "discreet_escalation_off",
            ),
        )
    for ((value, label, tag) in options) {
        ChoiceRow(stringResource(label), choice == value, tag) { onChoose(value) }
    }
    if (choice == DiscreetEscalation.OFF) {
        Body(
            stringResource(R.string.native_ui_parent_discreet_off_warning),
            tag = "discreet_escalation_off_warning",
            color = LocalAppColors.current.warn,
        )
    }
}
