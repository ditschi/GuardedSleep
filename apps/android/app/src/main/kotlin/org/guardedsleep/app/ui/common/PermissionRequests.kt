package org.guardedsleep.app.ui.common

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LifecycleResumeEffect
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.model.Banner
import org.guardedsleep.app.ui.model.BannerKind

/**
 * Requests one runtime permission. When the system shows no dialog any more (a repeated refusal:
 * the request returns denied and no rationale is offered), the app's system settings open instead
 * (`device-setup` "No later permission prompts"). [onResult] tells whether it ended up granted.
 */
@Composable
fun rememberPermissionRequest(
    permission: String,
    fallback: (Context) -> Intent = SystemSettings::appDetails,
    onResult: (Boolean) -> Unit = {},
): () -> Unit {
    val ctx = LocalContext.current
    val activity = ctx.findActivity()
    val launcher =
        rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
            if (
                !granted &&
                    activity != null &&
                    !activity.shouldShowRequestPermissionRationale(permission)
            ) {
                SystemSettings.launch(ctx, fallback(ctx))
            }
            onResult(granted)
        }
    return { launcher.launch(permission) }
}

/** True while the microphone is allowed; re-read whenever the screen comes back to the front. */
@Composable
fun rememberMicrophoneGranted(): Boolean {
    val ctx = LocalContext.current
    val tick = rememberResumeTick()
    return remember(tick) { micAllowed(ctx) }
}

/** Goes up every time the screen comes back to the front, to re-read system state. */
@Composable
private fun rememberResumeTick(): Int {
    var tick by remember { mutableIntStateOf(0) }
    LifecycleResumeEffect(Unit) {
        tick++
        onPauseOrDispose {}
    }
    return tick
}

/**
 * The baby unit's permission cards before it connects (`device-setup` "Baby unit permission gate
 * before connecting"): the microphone card blocks (the caller shows no code and starts nothing
 * while it is up, [blockingMic] tells it so), the background-operation and Do Not Disturb cards
 * only warn. Same look as the parent's readiness banners (safety colours); every card names its
 * reason and has a fix action. Shows nothing for a permission that is allowed.
 */
@Composable
fun BabyPermissionCards(modifier: Modifier = Modifier, tag: String = "baby_gate") {
    val ctx = LocalContext.current
    var refresh by remember { mutableIntStateOf(0) }
    LifecycleResumeEffect(Unit) {
        refresh++
        onPauseOrDispose {}
    }
    val micGranted = remember(refresh) { micAllowed(ctx) }
    val request = rememberPermissionRequest(Manifest.permission.RECORD_AUDIO) { refresh++ }
    val background = remember(refresh) { SystemSettings.batteryUnrestricted(ctx) }
    val dnd = remember(refresh) { SystemSettings.dndAccessGranted(ctx) }
    if (micGranted && background && dnd) return
    val fix = stringResource(R.string.native_ui_common_allow)
    val fixOther = stringResource(R.string.native_ui_banner_fix)
    Column(modifier.fillMaxWidth().testTag(tag), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        if (!micGranted)
            BannerCard(
                BannerSpec(
                    Banner(BannerKind.MIC_SILENT, "${tag}_mic"),
                    stringResource(R.string.native_ui_baby_mic_missing),
                    glyph = Glyph.MIC,
                    actionLabel = fix,
                    onAction = request,
                )
            )
        if (!background)
            BannerCard(
                BannerSpec(
                    Banner(BannerKind.READINESS, "${tag}_background"),
                    stringResource(R.string.native_ui_setup_device_check_battery_optimized),
                    glyph = Glyph.BATTERY,
                    actionLabel = fixOther,
                    onAction = {
                        SystemSettings.launch(ctx, SystemSettings.batteryOptimization(ctx))
                    },
                )
            )
        if (!dnd)
            BannerCard(
                BannerSpec(
                    Banner(BannerKind.READINESS, "${tag}_dnd"),
                    stringResource(R.string.native_ui_setup_device_check_dnd_access_missing),
                    glyph = Glyph.BELL,
                    actionLabel = fixOther,
                    onAction = { SystemSettings.launch(ctx, SystemSettings.dndAccess()) },
                )
            )
    }
}

private fun micAllowed(ctx: Context) =
    ContextCompat.checkSelfPermission(ctx, Manifest.permission.RECORD_AUDIO) ==
        PackageManager.PERMISSION_GRANTED
