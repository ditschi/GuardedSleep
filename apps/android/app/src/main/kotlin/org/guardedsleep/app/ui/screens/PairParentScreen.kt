package org.guardedsleep.app.ui.screens

import android.Manifest
import android.content.pm.PackageManager
import android.os.Handler
import android.os.Looper
import android.util.Size
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.core.resolutionselector.ResolutionSelector
import androidx.camera.core.resolutionselector.ResolutionStrategy
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.LocalLifecycleOwner
import java.util.concurrent.Executors
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.common.Body
import org.guardedsleep.app.ui.common.Glyph
import org.guardedsleep.app.ui.common.GlyphIcon
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.common.PrimaryButton
import org.guardedsleep.app.ui.common.QuietButton
import org.guardedsleep.app.ui.common.ScreenRoot
import org.guardedsleep.app.ui.common.ScrollColumn
import org.guardedsleep.app.ui.common.SecondaryButton
import org.guardedsleep.app.ui.common.SystemSettings
import org.guardedsleep.app.ui.common.Title
import org.guardedsleep.app.ui.common.findActivity
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.PairingError
import org.guardedsleep.app.ui.model.PairingPhase
import org.guardedsleep.app.ui.theme.LocalAppColors
import org.guardedsleep.app.ui.theme.StateColors
import zxingcpp.BarcodeReader

/** Camera preview + QR analyzer; replaced by a placeholder in screenshot tests. */
val LocalScannerView =
    staticCompositionLocalOf<@Composable (onCode: (String) -> Unit, modifier: Modifier) -> Unit> {
        { onCode, modifier -> CameraScanner(onCode, modifier) }
    }

/** S05: parent phone scans the code (or pastes it). */
@Composable
fun PairParentScreen(controller: AppController) {
    val ctx = LocalContext.current
    val env = LocalUiEnv.current
    val state = controller.pairing.collectAsState().value
    var refresh by remember { mutableIntStateOf(0) }
    LifecycleResumeEffect(Unit) {
        refresh++
        onPauseOrDispose {}
    }
    val granted =
        env.cameraGranted
            ?: remember(refresh) {
                ContextCompat.checkSelfPermission(ctx, Manifest.permission.CAMERA) ==
                    PackageManager.PERMISSION_GRANTED
            }
    var denials by remember { mutableIntStateOf(0) }
    val launcher =
        rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { ok ->
            refresh++
            if (!ok) denials++
        }
    val activity = ctx.findActivity()
    val focus = remember { FocusRequester() }
    // The pasted code is a secret: plain remember, never saved instance state.
    var pasted by remember { mutableStateOf("") }
    var askedOnce by remember { mutableStateOf(false) }
    if (!granted && !askedOnce && env.cameraGranted == null) {
        askedOnce = true
        DisposableEffect(Unit) {
            launcher.launch(Manifest.permission.CAMERA)
            onDispose {}
        }
    }
    BackHandler { controller.cancelPairing() }

    ParentPairContent(
        phase = state?.phase ?: PairingPhase.SCANNING,
        error = state?.error,
        cameraGranted = granted,
        pasted = pasted,
        onPastedChange = { pasted = it },
        focus = focus,
        onCode = { controller.submitPairingCode(it) },
        onSubmitPaste = { controller.submitPairingCode(pasted.trim()) },
        onAllowCamera = {
            if (
                denials > 0 &&
                    activity != null &&
                    !activity.shouldShowRequestPermissionRationale(Manifest.permission.CAMERA)
            ) {
                SystemSettings.launch(ctx, SystemSettings.appDetails(ctx))
            } else {
                launcher.launch(Manifest.permission.CAMERA)
            }
        },
        onRetry = { controller.startPairing() },
        onCancel = { controller.cancelPairing() },
    )
}

@Composable
fun ParentPairContent(
    phase: PairingPhase,
    error: PairingError?,
    cameraGranted: Boolean,
    pasted: String,
    onPastedChange: (String) -> Unit,
    focus: FocusRequester,
    onCode: (String) -> Unit,
    onSubmitPaste: () -> Unit,
    onAllowCamera: () -> Unit,
    onRetry: () -> Unit,
    onCancel: () -> Unit,
) {
    val connecting = phase == PairingPhase.CONNECTING
    val cameraDenied = !cameraGranted || error == PairingError.CAMERA_DENIED
    ScreenRoot("parent_pair") {
        ScrollColumn(verticalArrangement = Arrangement.spacedBy(14.dp)) {
            Title(stringResource(R.string.native_ui_pair_parent_title), "parent_pair_title")
            Body(
                stringResource(R.string.native_ui_pair_parent_camera_why),
                tag = "parent_pair_camera_why",
            )
            if (cameraDenied) {
                Column(
                    Modifier.fillMaxWidth()
                        .background(
                            MaterialTheme.colorScheme.surfaceVariant,
                            RoundedCornerShape(20.dp),
                        )
                        .padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        GlyphIcon(Glyph.CROSS, LocalAppColors.current.bad)
                        Text(
                            stringResource(R.string.native_ui_pair_error_camera_denied),
                            Modifier.testTag("parent_pair_camera_denied"),
                            style = MaterialTheme.typography.titleMedium,
                            color = MaterialTheme.colorScheme.onSurface,
                        )
                    }
                    SecondaryButton(
                        stringResource(R.string.native_ui_pair_error_allow_camera),
                        onAllowCamera,
                        "parent_pair_allow_camera",
                        Modifier.fillMaxWidth(),
                    )
                    SecondaryButton(
                        stringResource(R.string.native_ui_pair_error_paste_instead),
                        { runCatching { focus.requestFocus() } },
                        "parent_pair_paste_instead",
                        Modifier.fillMaxWidth(),
                    )
                }
            } else {
                Box(
                    Modifier.fillMaxWidth()
                        .widthIn(max = 420.dp)
                        .aspectRatio(1f)
                        .clip(RoundedCornerShape(20.dp))
                        .background(Color.Black)
                        .testTag("parent_pair_scanner")
                ) {
                    LocalScannerView.current(onCode, Modifier.matchParentSize())
                }
            }
            if (connecting) {
                Row(
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    if (LocalUiEnv.current.reduceMotion)
                        GlyphIcon(Glyph.REFRESH, MaterialTheme.colorScheme.primary)
                    else
                        CircularProgressIndicator(
                            Modifier.padding(2.dp).heightIn(max = 24.dp),
                            strokeWidth = 2.dp,
                        )
                    Text(
                        stringResource(R.string.native_ui_pair_parent_connecting),
                        Modifier.testTag("parent_pair_connecting"),
                        style = MaterialTheme.typography.titleMedium,
                        color = MaterialTheme.colorScheme.onBackground,
                    )
                }
            }
            if (error != null && error != PairingError.CAMERA_DENIED) {
                ErrorBlock(error, onRetry)
            }
            if (!connecting) {
                OutlinedTextField(
                    value = pasted,
                    onValueChange = onPastedChange,
                    label = { Text(stringResource(R.string.native_ui_pair_parent_paste_label)) },
                    modifier =
                        Modifier.fillMaxWidth()
                            .focusRequester(focus)
                            .testTag("parent_pair_paste_field"),
                    maxLines = 4,
                    keyboardOptions =
                        KeyboardOptions(
                            capitalization = KeyboardCapitalization.None,
                            autoCorrectEnabled = false,
                        ),
                )
                PrimaryButton(
                    stringResource(R.string.native_ui_pair_parent_paste_submit),
                    onSubmitPaste,
                    "parent_pair_paste_submit",
                    enabled = pasted.isNotBlank(),
                )
            }
            QuietButton(
                stringResource(R.string.native_ui_common_cancel),
                onCancel,
                "parent_pair_cancel",
            )
        }
    }
}

@Composable
private fun ErrorBlock(error: PairingError, onRetry: () -> Unit) {
    val text =
        when (error) {
            PairingError.NOT_OUR_CODE -> R.string.native_ui_pair_error_not_ours
            PairingError.EXPIRED -> R.string.native_ui_pair_error_expired
            PairingError.KEY_MISMATCH -> R.string.native_ui_pair_error_mismatch
            PairingError.ALREADY_PAIRED -> R.string.native_ui_pair_error_already_paired
            PairingError.UNREACHABLE -> R.string.native_ui_pair_error_unreachable
            PairingError.INCOMPLETE_PASTE -> R.string.native_ui_pair_error_incomplete
            PairingError.CAMERA_DENIED -> R.string.native_ui_pair_error_camera_denied
        }
    Column(
        Modifier.fillMaxWidth()
            .background(StateColors.Red, RoundedCornerShape(16.dp))
            .padding(14.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Row(
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalAlignment = Alignment.Top,
        ) {
            GlyphIcon(Glyph.EXCLAIM, StateColors.OnState)
            Text(
                stringResource(text),
                Modifier.testTag("parent_pair_error").weight(1f),
                style = MaterialTheme.typography.titleMedium,
                color = StateColors.OnState,
            )
        }
        if (error == PairingError.UNREACHABLE) {
            SecondaryButton(
                stringResource(R.string.native_ui_common_try_again),
                onRetry,
                "parent_pair_retry",
                Modifier.fillMaxWidth(),
            )
        }
    }
}

@Composable
private fun CameraScanner(onCode: (String) -> Unit, modifier: Modifier) {
    val ctx = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    val onCodeState = rememberUpdatedState(onCode)
    val previewView = remember {
        PreviewView(ctx).apply { scaleType = PreviewView.ScaleType.FILL_CENTER }
    }
    DisposableEffect(lifecycleOwner) {
        val analysisExecutor = Executors.newSingleThreadExecutor()
        val future = ProcessCameraProvider.getInstance(ctx)
        val main = ContextCompat.getMainExecutor(ctx)
        val reader =
            BarcodeReader(
                BarcodeReader.Options(
                    formats = setOf(BarcodeReader.Format.QR_CODE),
                    tryInvert = true,
                    tryHarder = true,
                )
            )
        var lastText: String? = null
        var lastAt = 0L
        val handler = Handler(Looper.getMainLooper())
        val analysis =
            ImageAnalysis.Builder()
                .setResolutionSelector(
                    ResolutionSelector.Builder()
                        .setResolutionStrategy(
                            ResolutionStrategy(
                                Size(1280, 720),
                                ResolutionStrategy.FALLBACK_RULE_CLOSEST_HIGHER_THEN_LOWER,
                            )
                        )
                        .build()
                )
                .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                .build()
        analysis.setAnalyzer(analysisExecutor) { image ->
            try {
                val text = reader.read(image).firstOrNull()?.text
                val now = System.currentTimeMillis()
                if (text != null && (text != lastText || now - lastAt > 3000)) {
                    lastText = text
                    lastAt = now
                    handler.post { onCodeState.value(text) }
                }
            } finally {
                image.close()
            }
        }
        var provider: ProcessCameraProvider? = null
        future.addListener(
            {
                provider = future.get()
                val preview =
                    Preview.Builder().build().also {
                        it.setSurfaceProvider(previewView.surfaceProvider)
                    }
                runCatching {
                    provider?.unbindAll()
                    provider?.bindToLifecycle(
                        lifecycleOwner,
                        CameraSelector.DEFAULT_BACK_CAMERA,
                        preview,
                        analysis,
                    )
                }
            },
            main,
        )
        onDispose {
            analysis.clearAnalyzer()
            provider?.unbindAll()
            analysisExecutor.shutdown()
        }
    }
    AndroidView(factory = { previewView }, modifier = modifier)
}
