package org.guardedsleep.app.ui.screens

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
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
import org.guardedsleep.app.ui.common.Dot
import org.guardedsleep.app.ui.common.Glyph
import org.guardedsleep.app.ui.common.GlyphButton
import org.guardedsleep.app.ui.common.GlyphIcon
import org.guardedsleep.app.ui.common.RenameConnectionDialog
import org.guardedsleep.app.ui.common.ScreenRoot
import org.guardedsleep.app.ui.common.ScrollColumn
import org.guardedsleep.app.ui.common.SecondaryButton
import org.guardedsleep.app.ui.common.connectionLabel
import org.guardedsleep.app.ui.common.formatTime
import org.guardedsleep.app.ui.common.rememberLanguageChoices
import org.guardedsleep.app.ui.model.AlertOutput
import org.guardedsleep.app.ui.model.AlertStyle
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.BabyMonitorState
import org.guardedsleep.app.ui.model.DiscreetEscalation
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.SettingsState
import org.guardedsleep.app.ui.model.TalkVolume
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

/**
 * The pages of Settings: an overview and one page per group row (`device-setup` "Settings
 * structure").
 */
enum class SettingsPage {
    ROOT,
    CONNECTIONS,
    ALERTS,
    DETECTION,
    SILENCE,
    TALK,
    DISPLAY,
    OPEN_AT_START,
    LANGUAGE,
    ABOUT,
    ADVANCED,
}

/**
 * S12: the Settings overview with two groups, "Connections" and "This device", and one page behind
 * each row. Advanced stays behind a warning. [initialPage] and [onPageChange] let the caller keep
 * the page across the alarm tone picker and the set-up check; back on a page returns to the
 * overview, back on the overview calls [onBack].
 */
@Composable
fun SettingsScreen(
    controller: AppController,
    onBack: () -> Unit,
    onOpenSetup: () -> Unit,
    onTestAlert: () -> Unit,
    buildInfo: BuildInfo = BuildInfo.Preview,
    onOpenAlarmTone: () -> Unit = {},
    initialPage: SettingsPage = SettingsPage.ROOT,
    onPageChange: (SettingsPage) -> Unit = {},
) {
    val s = controller.settings.collectAsState().value
    val app = controller.app.collectAsState().value
    val b = controller.baby.collectAsState().value
    val isBaby = app.viewRole == Role.BABY
    var page by rememberSaveable { mutableStateOf(initialPage) }
    fun goTo(p: SettingsPage) {
        page = p
        onPageChange(p)
    }
    var confirmAdvanced by remember { mutableStateOf(false) }
    // The connection (of its own role) a rename or forget dialog is about.
    var renaming by remember { mutableStateOf<ConnectionRef?>(null) }
    var forgetting by remember { mutableStateOf<ConnectionRef?>(null) }
    // A page goes back to the overview, the overview back to where Settings was opened from.
    val goBack = { if (page == SettingsPage.ROOT) onBack() else goTo(SettingsPage.ROOT) }
    BackHandler(onBack = goBack)
    ScreenRoot("settings") {
        ScrollColumn(verticalArrangement = Arrangement.spacedBy(14.dp)) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                GlyphButton(
                    Glyph.BACK,
                    stringResource(R.string.native_ui_common_back),
                    goBack,
                    "settings_back",
                )
                Text(
                    stringResource(pageTitle(page)),
                    Modifier.testTag("settings_title").semantics { heading() },
                    style = MaterialTheme.typography.headlineMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onBackground,
                )
            }

            when (page) {
                SettingsPage.ROOT ->
                    SettingsOverview(
                        controller,
                        app,
                        s,
                        isBaby,
                        onOpenSetup = onOpenSetup,
                        onPage = ::goTo,
                    )
                SettingsPage.CONNECTIONS ->
                    ConnectionsPage(
                        app,
                        onRename = { renaming = it },
                        onForget = { forgetting = it },
                    )
                SettingsPage.DETECTION -> DetectionPage(controller, s, b)
                SettingsPage.SILENCE -> SilencePage(controller, s, b)
                SettingsPage.TALK -> TalkPage(controller, s)
                SettingsPage.ALERTS -> AlertsPage(controller, s, onOpenAlarmTone, onTestAlert)
                SettingsPage.DISPLAY -> DisplayPage(controller, s)
                SettingsPage.OPEN_AT_START -> OpenAtStartPage(controller, app)
                SettingsPage.LANGUAGE -> LanguagePage(controller, s)
                SettingsPage.ABOUT -> AboutPage(buildInfo)
                SettingsPage.ADVANCED -> AdvancedPage(controller, s) { confirmAdvanced = true }
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
        renaming?.let { ref ->
            val c = app.connectionsOf(ref.role).firstOrNull { it.id == ref.id }
            RenameConnectionDialog(
                initial = c?.name.orEmpty(),
                onSave = { controller.renameConnection(ref.role, ref.id, it) },
                onDismiss = { renaming = null },
            )
        }
        forgetting?.let { ref ->
            val list = app.connectionsOf(ref.role)
            val c = list.firstOrNull { it.id == ref.id }
            ConfirmDialog(
                tag = "dialog_forget",
                title =
                    stringResource(
                        R.string.native_ui_forget_title,
                        c?.let { connectionLabel(it, list, ref.role) }.orEmpty(),
                    ),
                body = stringResource(R.string.native_ui_forget_body),
                confirmText = stringResource(R.string.native_ui_forget_confirm),
                cancelText = stringResource(R.string.native_ui_common_cancel),
                confirmTag = "dialog_forget_confirm",
                cancelTag = "dialog_forget_cancel",
                onConfirm = {
                    forgetting = null
                    controller.forgetConnection(ref.role, ref.id)
                },
                onDismiss = { forgetting = null },
            )
        }
    }
}

/** One saved connection together with the role it belongs to (its rows act on that role). */
private data class ConnectionRef(val role: Role, val id: String)

private val ROLES_IN_ORDER = listOf(Role.PARENT, Role.BABY)

private fun Role.tagKey(): String = name.lowercase()

private fun roleTitleRes(role: Role): Int =
    if (role == Role.PARENT) R.string.native_ui_role_parent_title
    else R.string.native_ui_role_baby_title

@Composable
private fun RoleLabel(role: Role) {
    Text(
        stringResource(roleTitleRes(role)),
        Modifier.testTag("settings_connections_role_${role.tagKey()}").semantics { heading() },
        style = MaterialTheme.typography.titleSmall,
        fontWeight = FontWeight.SemiBold,
        color = LocalAppColors.current.secondaryText,
    )
}

@Composable
private fun DetectionPage(controller: AppController, s: SettingsState, b: BabyMonitorState) {
    Section(
        null,
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
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
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
}

@Composable
private fun SilencePage(controller: AppController, s: SettingsState, b: BabyMonitorState) {
    Section(null, "settings_group_silence") {
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
}

@Composable
private fun TalkPage(controller: AppController, s: SettingsState) {
    // An adult opened the talk-back settings: the "may not be audible" warning ends.
    LaunchedEffect(Unit) { controller.talkSettingsOpened() }
    Section(null, "settings_group_talk") {
        SwitchRow(
            stringResource(R.string.native_ui_talk_baby_switch),
            s.talkBackEnabled,
            "baby_talk_toggle",
        ) { on ->
            controller.updateSettings { st -> st.copy(talkBackEnabled = on) }
        }
        Body(stringResource(R.string.native_ui_talk_baby_switch_hint))
        if (s.talkBackEnabled) {
            Body(
                stringResource(R.string.native_ui_talk_baby_volume),
                color = MaterialTheme.colorScheme.onSurface,
            )
            Row(
                Modifier.testTag("baby_talk_volume"),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                TalkVolume.entries.forEach { v ->
                    ChoiceChip(
                        stringResource(
                            when (v) {
                                TalkVolume.LOW -> R.string.native_ui_talk_baby_low
                                TalkVolume.MEDIUM -> R.string.native_ui_talk_baby_medium
                                TalkVolume.HIGH -> R.string.native_ui_talk_baby_high
                            }
                        ),
                        s.talkVolume == v,
                        "baby_talk_volume_" + v.name.lowercase(),
                    ) {
                        controller.updateSettings { st -> st.copy(talkVolume = v) }
                    }
                }
            }
            SecondaryButton(
                stringResource(R.string.native_ui_talk_baby_play_chime),
                { controller.playTalkChimeSample() },
                "baby_talk_play_chime",
            )
            SwitchRow(
                stringResource(R.string.native_ui_talk_baby_chime),
                s.talkChime,
                "baby_talk_chime_toggle",
            ) { on ->
                controller.updateSettings { st -> st.copy(talkChime = on) }
            }
        }
    }
}

@Composable
private fun AlertsPage(
    controller: AppController,
    s: SettingsState,
    onOpenAlarmTone: () -> Unit,
    onTestAlert: () -> Unit,
) {
    Section(null, "settings_group_alerts") {
        AlertStyleChoice(s.alertStyle) { style ->
            controller.updateSettings { st -> st.copy(alertStyle = style) }
        }
        if (s.alertStyle == AlertStyle.DISCREET) {
            DiscreetEscalationChoice(s.discreetEscalation) { choice ->
                controller.updateSettings { st ->
                    st.copy(discreetEscalation = choice)
                }
            }
        }
        // The tone of the connection alarm (parent only); the picker is its own
        // screen.
        Row(
            Modifier.fillMaxWidth()
                .heightIn(min = 56.dp)
                .clickable(role = SemRole.Button, onClick = onOpenAlarmTone)
                .testTag("settings_alarm_tone"),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Column(Modifier.weight(1f)) {
                Text(
                    stringResource(R.string.native_ui_settings_alarm_tone_label),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Body(
                    alarmToneName(s.connectionAlarmTone),
                    tag = "settings_alarm_tone_name",
                )
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
            remember(s.alertGainPct) {
                mutableFloatStateOf(s.alertGainPct.toFloat())
            }
        Body(
            stringResource(
                R.string.native_ui_settings_gain,
                gain.toInt().toString(),
            ),
            tag = "settings_gain_label",
            color = MaterialTheme.colorScheme.onSurface,
        )
        Slider(
            value = gain,
            onValueChange = { gain = it },
            onValueChangeFinished = {
                controller.updateSettings { st ->
                    st.copy(alertGainPct = gain.toInt())
                }
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
                stringResource(
                    R.string.native_ui_settings_last_test,
                    formatTime(it),
                )
            } ?: stringResource(R.string.native_ui_settings_never_tested),
            tag = "settings_last_test",
        )
    }
}

@Composable
private fun DisplayPage(controller: AppController, s: SettingsState) {
    Section(null, "settings_group_display") {
        Theme.entries.forEach { theme ->
            ChoiceRow(
                themeLabel(theme),
                s.theme == theme,
                "settings_theme_" + theme.name.lowercase(),
            ) {
                controller.updateSettings { st -> st.copy(theme = theme) }
            }
        }
    }
    NightSection(controller, s)
}

@Composable
private fun LanguagePage(controller: AppController, s: SettingsState) {
    Section(null, "settings_group_language") {
        // "Match system" is translated; the languages are shown by endonym
        // (generated).
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
}

@Composable
private fun AboutPage(buildInfo: BuildInfo) {
    Section(null, "settings_group_about") {
        Body(
            stringResource(R.string.native_ui_setup_limits_body),
            tag = "settings_limits",
        )
        Body(
            stringResource(
                R.string.native_ui_settings_version,
                buildInfo.version,
                buildInfo.commit,
                buildInfo.commitTime,
            ),
            tag = "settings_version",
        )
        Body(
            stringResource(R.string.native_ui_settings_font_credit),
            tag = "settings_font_credit",
        )
    }
}

@Composable
private fun AdvancedPage(
    controller: AppController,
    s: SettingsState,
    onConfirmAdvanced: () -> Unit,
) {
    Section(null, "settings_group_advanced") {
        SwitchRow(
            stringResource(R.string.native_ui_settings_advanced_toggle),
            s.advancedEnabled,
            "settings_advanced_toggle",
        ) { on ->
            if (on) onConfirmAdvanced()
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

@Composable
private fun themeLabel(theme: Theme): String =
    stringResource(
        when (theme) {
            Theme.DARK -> R.string.native_ui_settings_theme_dark
            Theme.LIGHT -> R.string.native_ui_settings_theme_light
            Theme.SYSTEM -> R.string.native_ui_settings_theme_system
        }
    )

private fun pageTitle(page: SettingsPage): Int =
    when (page) {
        SettingsPage.ROOT -> R.string.native_ui_common_settings
        SettingsPage.CONNECTIONS -> R.string.native_ui_connections_title
        SettingsPage.ALERTS -> R.string.native_ui_settings_group_alerts
        SettingsPage.DETECTION -> R.string.native_ui_settings_group_sound_detection
        SettingsPage.SILENCE -> R.string.native_ui_settings_group_silence
        SettingsPage.TALK -> R.string.native_ui_talk_baby_title
        SettingsPage.DISPLAY -> R.string.native_ui_settings_group_display
        SettingsPage.OPEN_AT_START -> R.string.native_ui_settings_open_at_start_title
        SettingsPage.LANGUAGE -> R.string.native_ui_settings_group_language
        SettingsPage.ABOUT -> R.string.native_ui_settings_group_about
        SettingsPage.ADVANCED -> R.string.native_ui_settings_group_advanced
    }

/** A group heading of the overview ("Connections", "This device"). */
@Composable
private fun GroupHeading(text: String, tag: String) {
    Text(
        text,
        Modifier.testTag(tag).semantics { heading() },
        style = MaterialTheme.typography.titleMedium,
        fontWeight = FontWeight.SemiBold,
        color = LocalAppColors.current.secondaryText,
    )
}

/** One row of the overview: title, optional summary, chevron; the whole row is the target. */
@Composable
private fun NavRow(title: String, summary: String?, tag: String, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth()
            .heightIn(min = 56.dp)
            .clickable(role = SemRole.Button, onClick = onClick)
            .testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                title,
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurface,
            )
            if (summary != null) Body(summary)
        }
        GlyphIcon(Glyph.CHEVRON, LocalAppColors.current.secondaryText)
    }
}

/**
 * The overview: the saved connections (read-only while monitoring), then the "This device" rows. A
 * baby device has no alert or display rows: it is always dark and only listens.
 */
@Composable
private fun SettingsOverview(
    controller: AppController,
    app: AppState,
    s: SettingsState,
    isBaby: Boolean,
    onOpenSetup: () -> Unit,
    onPage: (SettingsPage) -> Unit,
) {
    // The only way to change a connection is to stop first (`device-setup` "Connections read-only
    // while monitoring"): the rows then show no chevron and cannot be opened.
    val readOnly = app.monitoring
    GroupHeading(stringResource(R.string.native_ui_connections_title), "settings_group_connections")
    val anySaved = ROLES_IN_ORDER.any { app.connectionsOf(it).isNotEmpty() }
    AppCard(
        Modifier.testTag("settings_connections"),
        onClick =
            if (!readOnly && anySaved) {
                { onPage(SettingsPage.CONNECTIONS) }
            } else null,
    ) {
        if (!anySaved) {
            Body(stringResource(R.string.native_ui_home_empty), tag = "settings_not_paired")
        } else {
            ROLES_IN_ORDER.forEach { role ->
                val list = app.connectionsOf(role)
                if (list.isNotEmpty()) {
                    RoleLabel(role)
                    list.forEach { c ->
                        ConnectionLine(
                            connectionLabel(c, list, role),
                            "${role.tagKey()}_${c.id}",
                            inUse = readOnly && app.role == role && c.selected,
                        )
                    }
                }
            }
        }
        if (readOnly) {
            ConnectionsReadOnlyNote()
        } else if (anySaved) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                GlyphIcon(Glyph.CHEVRON, LocalAppColors.current.secondaryText)
            }
        }
    }

    GroupHeading(
        stringResource(R.string.native_ui_settings_group_this_phone),
        "settings_group_phone",
    )
    AppCard(Modifier.testTag("settings_this_device")) {
        if (isBaby) {
            NavRow(
                stringResource(R.string.native_ui_settings_group_sound_detection),
                null,
                "settings_nav_detection",
            ) {
                onPage(SettingsPage.DETECTION)
            }
            NavRow(
                stringResource(R.string.native_ui_settings_group_silence),
                null,
                "settings_nav_silence",
            ) {
                onPage(SettingsPage.SILENCE)
            }
            NavRow(
                stringResource(R.string.native_ui_talk_baby_title),
                null,
                "settings_nav_talk",
            ) {
                onPage(SettingsPage.TALK)
            }
        } else {
            NavRow(
                stringResource(R.string.native_ui_settings_group_alerts),
                alarmToneName(s.connectionAlarmTone),
                "settings_nav_alerts",
            ) {
                onPage(SettingsPage.ALERTS)
            }
            NavRow(
                stringResource(R.string.native_ui_settings_group_display),
                themeLabel(s.theme),
                "settings_nav_display",
            ) {
                onPage(SettingsPage.DISPLAY)
            }
        }
        NavRow(
            stringResource(R.string.native_ui_settings_open_at_start_title),
            startModeLabel(app.defaultRole),
            "settings_nav_open_at_start",
        ) {
            onPage(SettingsPage.OPEN_AT_START)
        }
        NavRow(
            stringResource(R.string.native_ui_settings_check_phone),
            null,
            "settings_open_setup",
            onOpenSetup,
        )
        val langs = rememberLanguageChoices()
        NavRow(
            stringResource(R.string.native_ui_settings_group_language),
            langs.firstOrNull { it.tag == s.language }?.endonym
                ?: stringResource(R.string.native_ui_settings_lang_system),
            "settings_nav_language",
        ) {
            onPage(SettingsPage.LANGUAGE)
        }
        NavRow(
            stringResource(R.string.native_ui_settings_group_about),
            null,
            "settings_nav_about",
        ) {
            onPage(SettingsPage.ABOUT)
        }
        NavRow(
            stringResource(R.string.native_ui_settings_group_advanced),
            null,
            "settings_nav_advanced",
        ) {
            onPage(SettingsPage.ADVANCED)
        }
    }
}

/** Why the connections cannot be changed: monitoring is running. */
@Composable
private fun ConnectionsReadOnlyNote() {
    Body(
        stringResource(R.string.native_ui_connections_read_only_note),
        tag = "settings_connections_read_only",
    )
}

/** A saved connection on the overview: its name and "In use". */
@Composable
private fun ConnectionLine(label: String, key: String, inUse: Boolean) {
    Column(Modifier.fillMaxWidth().testTag("settings_connection_row_$key")) {
        Text(
            label,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurface,
        )
        if (inUse) {
            // Dot and words: the state never rests on colour alone.
            Row(verticalAlignment = Alignment.CenterVertically) {
                Dot(LocalAppColors.current.ok)
                Spacer(Modifier.width(8.dp))
                Body(
                    stringResource(R.string.native_ui_connections_in_use),
                    tag = "settings_connection_in_use",
                    color = MaterialTheme.colorScheme.onSurface,
                )
            }
        }
    }
}

/**
 * Settings › Connections: the saved connections of both roles, grouped under "Parent device" and
 * "Baby device", with rename and forget acting on the row's own role. While monitoring the page is
 * read-only (it is also not reachable from the overview then; this guards a session that starts
 * meanwhile).
 */
@Composable
private fun ConnectionsPage(
    app: AppState,
    onRename: (ConnectionRef) -> Unit,
    onForget: (ConnectionRef) -> Unit,
) {
    val readOnly = app.monitoring
    val anySaved = ROLES_IN_ORDER.any { app.connectionsOf(it).isNotEmpty() }
    ROLES_IN_ORDER.forEach { role ->
        val list = app.connectionsOf(role)
        if (list.isNotEmpty()) {
            val key = role.tagKey()
            Section(roleTitleRes(role), "settings_group_paired_$key") {
                list.forEach { c ->
                    Column(
                        Modifier.testTag("settings_connection_${key}_${c.id}"),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Body(
                            connectionLabel(c, list, role),
                            tag = "settings_connection_name_${key}_${c.id}",
                            color = MaterialTheme.colorScheme.onSurface,
                        )
                        if (!readOnly) {
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                SecondaryButton(
                                    stringResource(R.string.native_ui_connections_rename),
                                    { onRename(ConnectionRef(role, c.id)) },
                                    "settings_connection_rename_${key}_${c.id}",
                                    Modifier.weight(1f),
                                )
                                SecondaryButton(
                                    stringResource(R.string.native_ui_forget_confirm),
                                    { onForget(ConnectionRef(role, c.id)) },
                                    "settings_connection_forget_${key}_${c.id}",
                                    Modifier.weight(1f),
                                    destructive = true,
                                )
                            }
                        }
                    }
                }
            }
        }
    }
    if (!anySaved) {
        Body(stringResource(R.string.native_ui_home_empty), tag = "settings_not_paired")
    }
    if (readOnly) {
        ConnectionsReadOnlyNote()
    }
}

/**
 * Settings › Open at start: the built start mode (which role a start opens in), the parent
 * connection that starts monitoring once per app start, and the baby unit's default connection. All
 * behave as built; only the place moved (`parent-pairing-entry`, `alternate-roles-and-apk-size`).
 */
@Composable
private fun OpenAtStartPage(controller: AppController, app: AppState) {
    Section(R.string.native_ui_settings_group_start_mode, "settings_group_start_mode") {
        Body(stringResource(R.string.native_ui_settings_open_at_start_mode_hint))
        // Which role a start without running monitoring opens in; null asks every time.
        listOf(null, Role.BABY, Role.PARENT).forEach { mode ->
            ChoiceRow(
                startModeLabel(mode),
                app.defaultRole == mode,
                "start_mode_" + (mode?.name?.lowercase() ?: "ask"),
            ) {
                controller.setDefaultRole(mode)
            }
        }
    }
    if (app.parentConnections.isNotEmpty()) {
        Section(
            R.string.native_ui_settings_open_at_start_auto_title,
            "settings_group_auto_start",
        ) {
            Body(stringResource(R.string.native_ui_settings_open_at_start_auto_hint))
            // The marked connection starts monitoring at app start; none is allowed.
            ChoiceRow(
                stringResource(R.string.native_ui_settings_open_at_start_auto_none),
                app.parentConnections.none { it.isDefault },
                "settings_auto_none",
            ) {
                controller.clearDefaultConnection(Role.PARENT)
            }
            app.parentConnections.forEach { c ->
                ChoiceRow(
                    connectionLabel(c, app.parentConnections, Role.PARENT),
                    c.isDefault,
                    "settings_connection_default_${c.id}",
                ) {
                    controller.setDefaultConnection(Role.PARENT, c.id)
                }
            }
        }
    }
    if (app.babyConnections.isNotEmpty()) {
        Section(
            R.string.native_ui_settings_open_at_start_default_title,
            "settings_group_baby_default",
        ) {
            app.babyConnections.forEach { c ->
                ChoiceRow(
                    connectionLabel(c, app.babyConnections, Role.BABY),
                    c.isDefault,
                    "settings_baby_default_${c.id}",
                ) {
                    controller.setDefaultConnection(Role.BABY, c.id)
                }
            }
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
private fun Section(title: Int?, tag: String, content: @Composable () -> Unit) {
    AppCard(Modifier.testTag(tag)) {
        // No heading where the page title already says it; only Open at start has two sections.
        if (title != null) {
            Text(
                stringResource(title),
                Modifier.semantics { heading() },
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
                color = LocalAppColors.current.secondaryText,
            )
        }
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
