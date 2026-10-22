package org.guardedsleep.app.ui.screens

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.PersistableBundle
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.qrcode.QRCodeWriter
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel
import org.guardedsleep.app.R
import org.guardedsleep.app.ui.common.AdaptiveButtonRow
import org.guardedsleep.app.ui.common.AmbientBar
import org.guardedsleep.app.ui.common.BabyQuiet
import org.guardedsleep.app.ui.common.Body
import org.guardedsleep.app.ui.common.Glyph
import org.guardedsleep.app.ui.common.GlyphIcon
import org.guardedsleep.app.ui.common.PrimaryButton
import org.guardedsleep.app.ui.common.QuietButton
import org.guardedsleep.app.ui.common.ScreenRoot
import org.guardedsleep.app.ui.common.ScrollColumn
import org.guardedsleep.app.ui.common.SecondaryButton
import org.guardedsleep.app.ui.common.Title
import org.guardedsleep.app.ui.common.Tone
import org.guardedsleep.app.ui.common.WindowBrightness
import org.guardedsleep.app.ui.common.formatClock
import org.guardedsleep.app.ui.common.rememberElapsedNow
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.PairingPhase

/**
 * Renders the pairing payload as a QR bitmap. Inverted by default (light modules on near-black, our
 * scanner handles inversion, ADR-018) and deliberately not full white (<= 50 % brightness look).
 */
fun renderQr(payload: String, bright: Boolean): ImageBitmap? = runCatching {
    val hints =
        mapOf(
            EncodeHintType.MARGIN to 2,
            EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M,
            EncodeHintType.CHARACTER_SET to "UTF-8",
        )
    val matrix = QRCodeWriter().encode(payload, BarcodeFormat.QR_CODE, 0, 0, hints)
    val scale = (560 / matrix.width).coerceAtLeast(4)
    val fg = if (bright) 0xFFFFFFFF.toInt() else 0xFFB8B8B8.toInt()
    val bg = 0xFF080808.toInt()
    val w = matrix.width * scale
    val h = matrix.height * scale
    val px = IntArray(w * h)
    for (y in 0 until h) for (x in 0 until w) {
        px[y * w + x] = if (matrix.get(x / scale, y / scale)) fg else bg
    }
    val bmp =
        android.graphics.Bitmap.createBitmap(px, w, h, android.graphics.Bitmap.Config.ARGB_8888)
    bmp.asImageBitmap()
}
    .getOrNull()

/** S04: baby phone shows the pairing code. */
@Composable
fun PairBabyScreen(controller: AppController) {
    val state = controller.pairing.collectAsState().value
    val ctx = LocalContext.current
    val now by rememberElapsedNow()
    var bright by remember { mutableStateOf(false) }
    var copied by remember { mutableStateOf(false) }
    var showText by remember { mutableStateOf(false) }
    BackHandler { controller.cancelPairing() }
    val expired =
        state?.phase == PairingPhase.EXPIRED ||
            (state?.expiresAtElapsedMs?.let { it <= now } ?: false)
    val payload = state?.codePayload
    WindowBrightness(if (bright) 0.9f else 0.4f)
    BabyQuiet {
        ScreenRoot("baby_pair", background = Color(0xFF050505)) {
            ScrollColumn(verticalArrangement = Arrangement.spacedBy(14.dp)) {
                Title(stringResource(R.string.native_ui_pair_baby_title), "baby_pair_title")
                when (state?.phase) {
                    PairingPhase.PAIRED ->
                        AmbientBar(
                            Glyph.LINK,
                            stringResource(R.string.native_ui_baby_status_connected),
                            Tone.OK,
                            "baby_pair_connected",
                        )
                    PairingPhase.CONNECTING ->
                        AmbientBar(
                            Glyph.REFRESH,
                            stringResource(R.string.native_ui_pair_baby_parent_found),
                            Tone.INFO,
                            "baby_pair_connecting",
                        )
                    PairingPhase.FAILED ->
                        AmbientBar(
                            Glyph.EXCLAIM,
                            stringResource(R.string.native_ui_pair_baby_failed),
                            Tone.BAD,
                            "baby_pair_failed",
                        )
                    else -> Unit
                }
                if (payload != null && state.phase != PairingPhase.PAIRED) {
                    val qr = remember(payload, bright) { renderQr(payload, bright) }
                    val dimmed = expired || state.phase == PairingPhase.FAILED
                    Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                        if (qr != null) {
                            Image(
                                qr,
                                contentDescription =
                                    stringResource(R.string.native_ui_pair_baby_qr_alt),
                                filterQuality = FilterQuality.None,
                                modifier =
                                    Modifier.size(280.dp)
                                        .alpha(if (dimmed) 0.2f else 1f)
                                        .testTag("baby_pair_qr"),
                            )
                        }
                    }
                    if (dimmed) {
                        Body(
                            stringResource(R.string.native_ui_pair_baby_expired),
                            tag = "baby_pair_expired",
                            color = Color(0xFFFFB4A9),
                        )
                        PrimaryButton(
                            stringResource(R.string.native_ui_pair_baby_new_code),
                            {
                                copied = false
                                controller.refreshPairingCode()
                            },
                            "baby_pair_new_code",
                        )
                    } else {
                        Body(
                            stringResource(R.string.native_ui_pair_baby_instruction),
                            tag = "baby_pair_instruction",
                            color = Color(0xFFD0D0D0),
                        )
                        val remaining = (state.expiresAtElapsedMs ?: now) - now
                        Text(
                            stringResource(
                                R.string.native_ui_pair_baby_valid_for,
                                formatClock(remaining),
                            ),
                            Modifier.testTag("baby_pair_countdown"),
                            style = MaterialTheme.typography.titleLarge,
                            color = Color(0xFFE0E0E0),
                        )
                        if (state.phase == PairingPhase.SHOWING_CODE) {
                            Row(
                                horizontalArrangement = Arrangement.spacedBy(8.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                GlyphIcon(Glyph.REFRESH, Color(0xFFB0B0B0), size = 20.dp)
                                Body(
                                    stringResource(R.string.native_ui_pair_baby_waiting),
                                    tag = "baby_pair_waiting",
                                    color = Color(0xFFB0B0B0),
                                )
                            }
                        }
                    }
                    if (!dimmed) {
                        // The text form contains the secret: only on request, never saved or
                        // logged.
                        if (showText) {
                            Text(
                                payload,
                                Modifier.fillMaxWidth()
                                    .background(Color(0xFF151515), RoundedCornerShape(12.dp))
                                    .padding(12.dp)
                                    .testTag("baby_pair_code_text"),
                                style = MaterialTheme.typography.bodySmall,
                                fontFamily = FontFamily.Monospace,
                                color = Color(0xFFB0B0B0),
                                maxLines = 3,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                        AdaptiveButtonRow {
                            SecondaryButton(
                                stringResource(
                                    if (copied) R.string.native_ui_pair_baby_copied
                                    else R.string.native_ui_pair_baby_copy
                                ),
                                {
                                    copyCode(ctx, payload)
                                    copied = true
                                },
                                "baby_pair_copy",
                            )
                            SecondaryButton(
                                stringResource(
                                    if (showText) R.string.native_ui_pair_baby_hide_text
                                    else R.string.native_ui_pair_baby_show_text
                                ),
                                { showText = !showText },
                                "baby_pair_show_text",
                            )
                            SecondaryButton(
                                stringResource(R.string.native_ui_pair_baby_brighter),
                                { bright = !bright },
                                "baby_pair_brighter",
                            )
                        }
                    }
                    Body(
                        stringResource(R.string.native_ui_pair_baby_privacy),
                        tag = "baby_pair_privacy",
                        color = Color(0xFFD0D0D0),
                    )
                } else if (state?.phase != PairingPhase.PAIRED) {
                    Body(
                        stringResource(R.string.native_ui_pair_baby_waiting),
                        tag = "baby_pair_waiting",
                        color = Color(0xFFB0B0B0),
                    )
                }
                QuietButton(
                    stringResource(R.string.native_ui_common_cancel),
                    { controller.cancelPairing() },
                    "baby_pair_cancel",
                    Modifier.heightIn(min = 48.dp),
                )
            }
        }
    }
}

/** The code is a secret: mark the clip sensitive so Android hides it in the clipboard preview. */
private fun copyCode(ctx: Context, payload: String) {
    val cm = ctx.getSystemService(ClipboardManager::class.java) ?: return
    val clip = ClipData.newPlainText("", payload)
    clip.description.extras =
        PersistableBundle().apply { putBoolean("android.content.extra.IS_SENSITIVE", true) }
    cm.setPrimaryClip(clip)
}
