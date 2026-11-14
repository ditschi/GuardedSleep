package org.guardedsleep.app.ui.screens

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LifecycleResumeEffect
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.common.AppCard
import org.guardedsleep.app.ui.common.Body
import org.guardedsleep.app.ui.common.Glyph
import org.guardedsleep.app.ui.common.GlyphIcon
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.common.PrimaryButton
import org.guardedsleep.app.ui.common.QuietButton
import org.guardedsleep.app.ui.common.RowState
import org.guardedsleep.app.ui.common.ScreenRoot
import org.guardedsleep.app.ui.common.ScrollColumn
import org.guardedsleep.app.ui.common.SecondaryButton
import org.guardedsleep.app.ui.common.SystemSettings
import org.guardedsleep.app.ui.common.Title
import org.guardedsleep.app.ui.common.findActivity
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.DeviceCheck
import org.guardedsleep.app.ui.model.DeviceCheckReason
import org.guardedsleep.app.ui.model.MailboxReach
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.theme.LocalAppColors
import org.guardedsleep.app.ui.theme.StateColors

enum class SetupRow {
    MIC,
    BACKGROUND,
    SILENCE,
    NOTIFICATIONS,
    LOCK_SCREEN,
    DND_BYPASS,
}

data class SetupChecks(val states: Map<SetupRow, RowState> = emptyMap()) {
    operator fun get(r: SetupRow): RowState = states[r] ?: RowState.TODO
}

/** S02 (baby) / S03 (parent): one checklist screen, every row = status + button into the system. */
@Composable
fun SetupScreen(controller: AppController, role: Role, onboarding: Boolean, onBack: () -> Unit) {
    val ctx = LocalContext.current
    val app = controller.app.collectAsState().value
    var refresh by remember { mutableIntStateOf(0) }
    LifecycleResumeEffect(Unit) {
        refresh++
        // The baby device check runs in the background and never blocks the checklist.
        if (role == Role.BABY) controller.runDeviceCheck()
        onPauseOrDispose {}
    }
    val checks = remember(refresh) { readSetupChecks(ctx, role) }
    var asked by rememberSaveable { mutableStateOf(setOf<String>()) }
    val activity = ctx.findActivity()

    val micLauncher =
        rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
            refresh++
            if (
                !granted &&
                    activity != null &&
                    !activity.shouldShowRequestPermissionRationale(Manifest.permission.RECORD_AUDIO)
            ) {
                SystemSettings.launch(ctx, SystemSettings.appDetails(ctx))
            }
        }
    val notifLauncher =
        rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
            refresh++
            if (!granted) SystemSettings.launch(ctx, SystemSettings.notificationSettings(ctx))
        }

    val onAction: (SetupRow) -> Unit = { row ->
        asked = asked + row.name
        when (row) {
            SetupRow.MIC ->
                if (asked.contains(row.name) && checks[row] == RowState.BLOCKED) {
                    SystemSettings.launch(ctx, SystemSettings.appDetails(ctx))
                } else {
                    micLauncher.launch(Manifest.permission.RECORD_AUDIO)
                }
            SetupRow.NOTIFICATIONS ->
                if (Build.VERSION.SDK_INT >= 33 && checks[row] != RowState.BLOCKED) {
                    notifLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
                } else {
                    SystemSettings.launch(ctx, SystemSettings.notificationSettings(ctx))
                }
            SetupRow.LOCK_SCREEN ->
                SystemSettings.launch(ctx, SystemSettings.fullScreenIntentSettings(ctx))
            SetupRow.SILENCE,
            SetupRow.DND_BYPASS -> SystemSettings.launch(ctx, SystemSettings.dndAccess())
            SetupRow.BACKGROUND ->
                SystemSettings.launch(ctx, SystemSettings.batteryOptimization(ctx))
        }
    }
    BackHandler(enabled = !onboarding, onBack = onBack)
    SetupContent(
        role = role,
        checks = checks,
        deviceCheck = app.deviceCheck,
        deviceCheckReasons = app.deviceCheckReasons,
        deviceCheckMailboxes = app.deviceCheckMailboxes,
        onDeviceCheckAgain = { controller.runDeviceCheck(force = true) },
        talkBack = controller.settings.collectAsState().value.talkBackEnabled,
        onTalkBack = { on -> controller.updateSettings { it.copy(talkBackEnabled = on) } },
        onAction = onAction,
        onContinue = {
            if (onboarding) {
                controller.completeOnboarding()
                controller.startPairing()
            } else {
                onBack()
            }
        },
    )
}

private fun readSetupChecks(ctx: android.content.Context, role: Role): SetupChecks {
    fun granted(p: String) =
        ContextCompat.checkSelfPermission(ctx, p) == PackageManager.PERMISSION_GRANTED
    fun st(ok: Boolean, blockedWhenDenied: Boolean = false) =
        if (ok) RowState.DONE else if (blockedWhenDenied) RowState.BLOCKED else RowState.TODO
    return SetupChecks(
        mapOf(
            SetupRow.MIC to st(granted(Manifest.permission.RECORD_AUDIO)),
            SetupRow.BACKGROUND to st(SystemSettings.batteryUnrestricted(ctx)),
            SetupRow.SILENCE to st(SystemSettings.dndAccessGranted(ctx)),
            SetupRow.NOTIFICATIONS to st(SystemSettings.notificationsEnabled(ctx)),
            SetupRow.LOCK_SCREEN to st(SystemSettings.fullScreenAllowed(ctx)),
            SetupRow.DND_BYPASS to st(SystemSettings.dndAccessGranted(ctx)),
        )
    )
}

@Composable
fun SetupContent(
    role: Role,
    checks: SetupChecks,
    deviceCheck: DeviceCheck,
    onAction: (SetupRow) -> Unit,
    onContinue: () -> Unit,
    /** Baby: the talk-back switch of the checklist row; null hides the row (previews). */
    talkBack: Boolean? = null,
    onTalkBack: (Boolean) -> Unit = {},
    /** Baby: why the device check is not OK, worst first. */
    deviceCheckReasons: List<DeviceCheckReason> = emptyList(),
    deviceCheckMailboxes: List<MailboxReach> = emptyList(),
    /** Shows "Check again" next to a finished device check; null hides it (previews). */
    onDeviceCheckAgain: (() -> Unit)? = null,
) {
    val required = if (role == Role.BABY) SetupRow.MIC else SetupRow.NOTIFICATIONS
    var skipped by rememberSaveable { mutableStateOf(setOf<String>()) }
    val canContinue = checks[required] == RowState.DONE
    ScreenRoot(if (role == Role.BABY) "baby_setup" else "parent_setup") {
        ScrollColumn {
            Title(stringResource(R.string.native_ui_setup_title), "setup_title")
            if (role == Role.BABY) {
                CheckRow(
                    "setup_row_mic",
                    SetupRow.MIC,
                    checks,
                    R.string.native_ui_setup_mic_title,
                    R.string.native_ui_setup_mic_why,
                    skipped,
                    true,
                    onAction,
                ) {}
                CheckRow(
                    "setup_row_background",
                    SetupRow.BACKGROUND,
                    checks,
                    R.string.native_ui_setup_background_title,
                    R.string.native_ui_setup_background_why,
                    skipped,
                    false,
                    onAction,
                ) {
                    skipped = skipped + it.name
                }
                CheckRow(
                    "setup_row_silence",
                    SetupRow.SILENCE,
                    checks,
                    R.string.native_ui_setup_silence_title,
                    R.string.native_ui_setup_silence_why,
                    skipped,
                    false,
                    onAction,
                ) {
                    skipped = skipped + it.name
                }
                if (talkBack != null) {
                    AppCard(Modifier.testTag("setup_row_talk")) {
                        Body(stringResource(R.string.native_ui_talk_baby_checklist))
                        SwitchRow(
                            stringResource(R.string.native_ui_talk_baby_switch),
                            talkBack,
                            "setup_talk_toggle",
                            onTalkBack,
                        )
                    }
                }
                AppCard(Modifier.testTag("setup_row_safety")) {
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                        verticalAlignment = Alignment.Top,
                    ) {
                        GlyphIcon(Glyph.BATTERY, LocalAppColors.current.warn)
                        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                            Body(
                                stringResource(R.string.native_ui_setup_safety_body),
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                            Body(stringResource(R.string.native_ui_setup_charging_tip))
                        }
                    }
                }
                DeviceCheckRow(
                    deviceCheck,
                    deviceCheckReasons,
                    deviceCheckMailboxes,
                    onDeviceCheckAgain,
                )
            } else {
                CheckRow(
                    "setup_row_notifications",
                    SetupRow.NOTIFICATIONS,
                    checks,
                    R.string.native_ui_setup_notifications_title,
                    R.string.native_ui_setup_notifications_why,
                    skipped,
                    true,
                    onAction,
                ) {}
                CheckRow(
                    "setup_row_lockscreen",
                    SetupRow.LOCK_SCREEN,
                    checks,
                    R.string.native_ui_setup_lock_screen_title,
                    R.string.native_ui_setup_lock_screen_why,
                    skipped,
                    false,
                    onAction,
                ) {
                    skipped = skipped + it.name
                }
                CheckRow(
                    "setup_row_dnd_bypass",
                    SetupRow.DND_BYPASS,
                    checks,
                    R.string.native_ui_setup_dnd_bypass_title,
                    R.string.native_ui_setup_dnd_bypass_why,
                    skipped,
                    false,
                    onAction,
                ) {
                    skipped = skipped + it.name
                }
                CheckRow(
                    "setup_row_background",
                    SetupRow.BACKGROUND,
                    checks,
                    R.string.native_ui_setup_background_title,
                    R.string.native_ui_setup_background_why,
                    skipped,
                    false,
                    onAction,
                ) {
                    skipped = skipped + it.name
                }
                AppCard(Modifier.testTag("setup_limits_card")) {
                    Body(
                        stringResource(R.string.native_ui_setup_limits_body),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            if (!canContinue) {
                Body(
                    stringResource(
                        if (role == Role.BABY) R.string.native_ui_setup_mic_required
                        else R.string.native_ui_setup_notifications_required
                    ),
                    tag = "setup_required_note",
                    color = LocalAppColors.current.bad,
                )
            }
            PrimaryButton(
                stringResource(R.string.native_ui_common_continue),
                onContinue,
                "setup_continue",
                enabled = canContinue,
            )
        }
    }
}

@Composable
private fun CheckRow(
    tag: String,
    row: SetupRow,
    checks: SetupChecks,
    title: Int,
    why: Int,
    skipped: Set<String>,
    required: Boolean,
    onAction: (SetupRow) -> Unit,
    onSkip: (SetupRow) -> Unit,
) {
    val state = checks[row]
    val isSkipped = skipped.contains(row.name) && state != RowState.DONE
    val stateText = stateText(state)
    AppCard(
        Modifier.testTag(tag).semantics(mergeDescendants = true) { stateDescription = stateText }
    ) {
        Row(
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalAlignment = Alignment.Top,
        ) {
            StateBadge(state)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(
                    stringResource(title),
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Body(stringResource(why), color = MaterialTheme.colorScheme.onSurfaceVariant)
                if (isSkipped)
                    Body(stringResource(R.string.native_ui_setup_skipped), tag = tag + "_skipped")
            }
        }
        if (state != RowState.DONE && state != RowState.CHECKING && !isSkipped) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                SecondaryButton(
                    stringResource(R.string.native_ui_common_allow),
                    { onAction(row) },
                    tag + "_allow",
                )
                if (!required)
                    QuietButton(
                        stringResource(R.string.native_ui_common_later),
                        { onSkip(row) },
                        "setup_skip_row_" + row.name.lowercase(),
                    )
            }
        }
    }
}

@Composable
private fun DeviceCheckRow(
    check: DeviceCheck,
    reasons: List<DeviceCheckReason>,
    mailboxes: List<MailboxReach>,
    onAgain: (() -> Unit)?,
) {
    val (state, text) =
        when (check) {
            DeviceCheck.UNKNOWN ->
                RowState.CHECKING to R.string.native_ui_setup_device_check_checking
            DeviceCheck.OK -> RowState.DONE to R.string.native_ui_setup_device_check_ok
            DeviceCheck.LIMITED -> RowState.TODO to R.string.native_ui_setup_device_check_limited
            DeviceCheck.NOT_RECOMMENDED ->
                RowState.TODO to R.string.native_ui_setup_device_check_not_recommended
        }
    AppCard(Modifier.testTag("setup_row_device_check")) {
        Row(
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalAlignment = Alignment.Top,
        ) {
            StateBadge(state)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Body(
                    stringResource(text),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                for (reason in reasons) {
                    Body(
                        stringResource(reasonText(reason)),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier =
                            Modifier.testTag(
                                "setup_device_check_reason_" + reason.name.lowercase()
                            ),
                    )
                }
                for (m in mailboxes) {
                    Body(
                        stringResource(
                            if (m.reachable) R.string.native_ui_setup_device_check_mailbox_reachable
                            else R.string.native_ui_setup_device_check_mailbox_unreachable,
                            m.host,
                        ),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                if (onAgain != null && check != DeviceCheck.UNKNOWN)
                    QuietButton(
                        stringResource(R.string.native_ui_setup_device_check_again),
                        onAgain,
                        "setup_device_check_again",
                    )
            }
        }
    }
}

private fun reasonText(reason: DeviceCheckReason): Int =
    when (reason) {
        DeviceCheckReason.MIC_PERMISSION_MISSING ->
            R.string.native_ui_setup_device_check_mic_permission_missing
        DeviceCheckReason.MIC_SILENT -> R.string.native_ui_setup_device_check_mic_silent
        DeviceCheckReason.BATTERY_UNHEALTHY ->
            R.string.native_ui_setup_device_check_battery_unhealthy
        DeviceCheckReason.BATTERY_LOW_NOT_CHARGING ->
            R.string.native_ui_setup_device_check_battery_low_not_charging
        DeviceCheckReason.NO_MAILBOX_REACHABLE ->
            R.string.native_ui_setup_device_check_no_mailbox_reachable
        DeviceCheckReason.NOT_CHARGING -> R.string.native_ui_setup_device_check_not_charging
        DeviceCheckReason.BATTERY_OPTIMIZED ->
            R.string.native_ui_setup_device_check_battery_optimized
        DeviceCheckReason.DND_ACCESS_MISSING ->
            R.string.native_ui_setup_device_check_dnd_access_missing
        DeviceCheckReason.SOME_MAILBOXES_UNREACHABLE ->
            R.string.native_ui_setup_device_check_some_mailboxes_unreachable
    }

@Composable
private fun stateText(state: RowState) =
    stringResource(
        when (state) {
            RowState.DONE -> R.string.native_ui_common_state_done
            RowState.TODO -> R.string.native_ui_common_state_todo
            RowState.BLOCKED -> R.string.native_ui_common_state_blocked
            RowState.CHECKING -> R.string.native_ui_common_state_checking
        }
    )

/**
 * done = green check, todo = amber !, blocked = red cross, checking = spinner (static when motion
 * is reduced).
 */
@Composable
fun StateBadge(state: RowState) {
    val colors = LocalAppColors.current
    val (bg, glyph) =
        when (state) {
            RowState.DONE -> StateColors.Green to Glyph.CHECK
            RowState.TODO -> StateColors.Amber to Glyph.EXCLAIM
            RowState.BLOCKED -> StateColors.Red to Glyph.CROSS
            RowState.CHECKING -> StateColors.Grey to Glyph.DOTS
        }
    Box(Modifier.size(36.dp).background(bg, CircleShape), contentAlignment = Alignment.Center) {
        if (state == RowState.CHECKING && !LocalUiEnv.current.reduceMotion) {
            CircularProgressIndicator(Modifier.size(22.dp), color = Color.White, strokeWidth = 2.dp)
        } else {
            GlyphIcon(glyph, StateColors.OnState, size = 20.dp)
        }
    }
    @Suppress("UNUSED_EXPRESSION") colors
}
