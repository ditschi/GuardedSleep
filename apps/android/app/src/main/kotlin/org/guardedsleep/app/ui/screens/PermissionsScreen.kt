package org.guardedsleep.app.ui.screens

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
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
import org.guardedsleep.app.ui.common.PrimaryButton
import org.guardedsleep.app.ui.common.QuietButton
import org.guardedsleep.app.ui.common.RowState
import org.guardedsleep.app.ui.common.ScreenRoot
import org.guardedsleep.app.ui.common.ScrollColumn
import org.guardedsleep.app.ui.common.SecondaryButton
import org.guardedsleep.app.ui.common.SystemSettings
import org.guardedsleep.app.ui.common.Title
import org.guardedsleep.app.ui.common.findActivity
import org.guardedsleep.app.ui.common.rememberPermissionRequest
import org.guardedsleep.app.ui.model.AppController

/**
 * What either role can need, in the order the screen lists it. Each row has its own system request:
 * the runtime ones (microphone, camera, notifications) show the system dialog, the special ones
 * open their system setting.
 */
enum class PermissionRow(val tag: String) {
    MICROPHONE("microphone"),
    CAMERA("camera"),
    NOTIFICATIONS("notifications"),
    FULL_SCREEN("full_screen"),
    DND("dnd"),
    BACKGROUND("background"),
}

/** The rows that exist on this Android version (notifications 33+, full-screen alerts 34+). */
internal fun permissionRows(sdk: Int = Build.VERSION.SDK_INT): List<PermissionRow> =
    PermissionRow.entries.filter {
        when (it) {
            PermissionRow.NOTIFICATIONS -> sdk >= 33
            PermissionRow.FULL_SCREEN -> sdk >= 34
            else -> true
        }
    }

private val RuntimePermissions =
    mapOf(
        PermissionRow.MICROPHONE to Manifest.permission.RECORD_AUDIO,
        PermissionRow.CAMERA to Manifest.permission.CAMERA,
        PermissionRow.NOTIFICATIONS to "android.permission.POST_NOTIFICATIONS",
    )

private fun granted(ctx: Context, row: PermissionRow): Boolean =
    when (row) {
        PermissionRow.MICROPHONE,
        PermissionRow.CAMERA ->
            ContextCompat.checkSelfPermission(ctx, RuntimePermissions.getValue(row)) ==
                PackageManager.PERMISSION_GRANTED
        PermissionRow.NOTIFICATIONS -> SystemSettings.notificationsEnabled(ctx)
        PermissionRow.FULL_SCREEN -> SystemSettings.fullScreenAllowed(ctx)
        PermissionRow.DND -> SystemSettings.dndAccessGranted(ctx)
        PermissionRow.BACKGROUND -> SystemSettings.batteryUnrestricted(ctx)
    }

private fun settingsIntent(ctx: Context, row: PermissionRow): Intent =
    when (row) {
        PermissionRow.MICROPHONE,
        PermissionRow.CAMERA -> SystemSettings.appDetails(ctx)
        PermissionRow.NOTIFICATIONS -> SystemSettings.notificationSettings(ctx)
        PermissionRow.FULL_SCREEN -> SystemSettings.fullScreenIntentSettings(ctx)
        PermissionRow.DND -> SystemSettings.dndAccess()
        PermissionRow.BACKGROUND -> SystemSettings.batteryOptimization(ctx)
    }

/** The special settings "Allow all" opens in turn (the runtime ones come first, in one dialog). */
private val SpecialRows =
    listOf(PermissionRow.FULL_SCREEN, PermissionRow.DND, PermissionRow.BACKGROUND)

private const val APP_SETTINGS = "APP_SETTINGS"

/**
 * S01b: the first-start permission screen, shown once before the role choice (`device-setup`
 * "Permissions on first start"). "Allow all" asks for the runtime permissions in one dialog
 * sequence, then opens the remaining system settings one by one as the user returns; every row can
 * also be allowed alone or put off. Nothing blocks: Continue is always possible.
 */
@Composable
fun PermissionsScreen(controller: AppController) {
    val ctx = LocalContext.current
    val activity = ctx.findActivity()
    val rows = remember { permissionRows() }
    var refresh by remember { mutableIntStateOf(0) }
    var later by rememberSaveable { mutableStateOf(setOf<String>()) }
    // The special settings still to open after "Allow all", and whether one is open right now.
    var queue by remember { mutableStateOf(listOf<String>()) }
    var waiting by remember { mutableStateOf(false) }

    fun advance() {
        while (queue.isNotEmpty()) {
            val next = queue.first()
            queue = queue.drop(1)
            val intent =
                if (next == APP_SETTINGS) SystemSettings.appDetails(ctx)
                else {
                    val row = PermissionRow.valueOf(next)
                    if (granted(ctx, row)) continue else settingsIntent(ctx, row)
                }
            waiting = true
            SystemSettings.launch(ctx, intent)
            return
        }
    }

    LifecycleResumeEffect(Unit) {
        refresh++
        if (waiting) {
            waiting = false
            advance()
        }
        onPauseOrDispose {}
    }
    val states =
        remember(refresh) {
            rows.associateWith { if (granted(ctx, it)) RowState.DONE else RowState.TODO }
        }

    val multi =
        rememberLauncherForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) {
            results ->
            refresh++
            // A refused runtime permission with no dialog left needs the app's settings.
            val stuck = results.any { (p, ok) ->
                !ok && activity != null && !activity.shouldShowRequestPermissionRationale(p)
            }
            queue =
                (if (stuck) listOf(APP_SETTINGS) else emptyList()) +
                    SpecialRows.filter { it in rows }.map { it.name }
            advance()
        }
    val mic = rememberPermissionRequest(Manifest.permission.RECORD_AUDIO) { refresh++ }
    val camera = rememberPermissionRequest(Manifest.permission.CAMERA) { refresh++ }
    val notifications =
        rememberPermissionRequest(
            "android.permission.POST_NOTIFICATIONS",
            fallback = SystemSettings::notificationSettings,
        ) {
            refresh++
        }

    PermissionsContent(
        rows = rows,
        states = states,
        later = later,
        onAllowAll = {
            val missing =
                rows
                    .filter { it in RuntimePermissions && states[it] != RowState.DONE }
                    .map { RuntimePermissions.getValue(it) }
            if (missing.isEmpty()) {
                queue = SpecialRows.filter { it in rows }.map { it.name }
                advance()
            } else {
                multi.launch(missing.toTypedArray())
            }
        },
        onAllow = { row ->
            when (row) {
                PermissionRow.MICROPHONE -> mic()
                PermissionRow.CAMERA -> camera()
                PermissionRow.NOTIFICATIONS -> notifications()
                else -> SystemSettings.launch(ctx, settingsIntent(ctx, row))
            }
        },
        onLater = { later = later + it.tag },
        onContinue = { controller.completePermissionsIntro() },
    )
}

/** The stateless screen: rows with state, reason and buttons; used by the tests and previews. */
@Composable
fun PermissionsContent(
    rows: List<PermissionRow>,
    states: Map<PermissionRow, RowState>,
    later: Set<String>,
    onAllowAll: () -> Unit,
    onAllow: (PermissionRow) -> Unit,
    onLater: (PermissionRow) -> Unit,
    onContinue: () -> Unit,
) {
    val allDone = rows.all { states[it] == RowState.DONE }
    ScreenRoot("permissions_screen") {
        ScrollColumn {
            Title(stringResource(R.string.native_ui_permissions_title), "permissions_title")
            Body(
                stringResource(R.string.native_ui_permissions_intro),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            for (row in rows) {
                PermissionRowCard(
                    row,
                    states[row] ?: RowState.TODO,
                    row.tag in later,
                    onAllow,
                    onLater,
                )
            }
            if (!allDone)
                PrimaryButton(
                    stringResource(R.string.native_ui_permissions_allow_all),
                    onAllowAll,
                    "permissions_allow_all",
                )
            if (allDone)
                PrimaryButton(
                    stringResource(R.string.native_ui_common_continue),
                    onContinue,
                    "permissions_continue",
                )
            else
                SecondaryButton(
                    stringResource(R.string.native_ui_common_continue),
                    onContinue,
                    "permissions_continue",
                )
        }
    }
}

@Composable
private fun PermissionRowCard(
    row: PermissionRow,
    state: RowState,
    isLater: Boolean,
    onAllow: (PermissionRow) -> Unit,
    onLater: (PermissionRow) -> Unit,
) {
    val tag = "permission_row_${row.tag}"
    val stateText =
        stringResource(
            if (state == RowState.DONE) R.string.native_ui_common_state_done
            else R.string.native_ui_common_state_todo
        )
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
                    stringResource(titleOf(row)),
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Body(
                    stringResource(whyOf(row)),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                if (isLater && state != RowState.DONE)
                    Body(stringResource(R.string.native_ui_setup_skipped), tag = tag + "_skipped")
            }
        }
        if (state != RowState.DONE) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                SecondaryButton(
                    stringResource(R.string.native_ui_common_allow),
                    { onAllow(row) },
                    tag + "_allow",
                )
                if (!isLater)
                    QuietButton(
                        stringResource(R.string.native_ui_common_later),
                        { onLater(row) },
                        tag + "_later",
                    )
            }
        }
    }
}

private fun titleOf(row: PermissionRow): Int =
    when (row) {
        PermissionRow.MICROPHONE -> R.string.native_ui_permissions_mic_title
        PermissionRow.CAMERA -> R.string.native_ui_permissions_camera_title
        PermissionRow.NOTIFICATIONS -> R.string.native_ui_setup_notifications_title
        PermissionRow.FULL_SCREEN -> R.string.native_ui_setup_lock_screen_title
        PermissionRow.DND -> R.string.native_ui_permissions_dnd_title
        PermissionRow.BACKGROUND -> R.string.native_ui_setup_background_title
    }

private fun whyOf(row: PermissionRow): Int =
    when (row) {
        PermissionRow.MICROPHONE -> R.string.native_ui_permissions_mic_why
        PermissionRow.CAMERA -> R.string.native_ui_permissions_camera_why
        PermissionRow.NOTIFICATIONS -> R.string.native_ui_setup_notifications_why
        PermissionRow.FULL_SCREEN -> R.string.native_ui_setup_lock_screen_why
        PermissionRow.DND -> R.string.native_ui_permissions_dnd_why
        PermissionRow.BACKGROUND -> R.string.native_ui_setup_background_why
    }
