package org.guardedsleep.app.ui.common

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
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
import org.guardedsleep.app.ui.theme.LocalAppColors

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
    var refresh by remember { mutableIntStateOf(0) }
    LifecycleResumeEffect(Unit) {
        refresh++
        onPauseOrDispose {}
    }
    return remember(refresh) {
        ContextCompat.checkSelfPermission(ctx, Manifest.permission.RECORD_AUDIO) ==
            PackageManager.PERMISSION_GRANTED
    }
}

/**
 * Why this device cannot start listening and a way to allow the microphone (the system dialog, or
 * the app's settings after a repeated refusal). Shows nothing while the microphone is allowed.
 */
@Composable
fun MicNeededNote(tag: String, modifier: Modifier = Modifier) {
    val granted = rememberMicrophoneGranted()
    var tick by remember { mutableIntStateOf(0) }
    val request = rememberPermissionRequest(Manifest.permission.RECORD_AUDIO) { tick++ }
    if (granted) return
    Column(modifier.testTag(tag), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Body(
            stringResource(R.string.native_ui_permissions_mic_needed),
            tag = tag + "_text",
            color = LocalAppColors.current.warn,
        )
        SecondaryButton(
            stringResource(R.string.native_ui_common_allow),
            request,
            tag + "_allow",
        )
    }
}
