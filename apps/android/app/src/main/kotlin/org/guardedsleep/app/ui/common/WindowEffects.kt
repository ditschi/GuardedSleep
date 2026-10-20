package org.guardedsleep.app.ui.common

import android.view.WindowManager
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.hapticfeedback.HapticFeedback
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.LocalView

/** Overrides the window brightness (0..1) while composed; null leaves the system value alone. */
@Composable
fun WindowBrightness(level: Float?) {
    val ctx = LocalContext.current
    DisposableEffect(level) {
        val window = ctx.findActivity()?.window
        val previous = window?.attributes?.screenBrightness
        if (window != null && level != null) {
            window.attributes = window.attributes.also { it.screenBrightness = level }
        }
        onDispose {
            if (window != null && level != null) {
                window.attributes = window.attributes.also {
                    it.screenBrightness = previous ?: WindowManager.LayoutParams.BRIGHTNESS_OVERRIDE_NONE
                }
            }
        }
    }
}

private object NoHaptics : HapticFeedback {
    override fun performHapticFeedback(hapticFeedbackType: HapticFeedbackType) = Unit
}

/** Baby phone rule (ADR-018 §3.1, §6.2): no haptics and no click sounds from any UI, ever. */
@Composable
fun BabyQuiet(content: @Composable () -> Unit) {
    val view = LocalView.current
    DisposableEffect(view) {
        val hapticBefore = view.isHapticFeedbackEnabled
        val soundBefore = view.isSoundEffectsEnabled
        view.isHapticFeedbackEnabled = false
        view.isSoundEffectsEnabled = false
        onDispose {
            view.isHapticFeedbackEnabled = hapticBefore
            view.isSoundEffectsEnabled = soundBefore
        }
    }
    CompositionLocalProvider(LocalHapticFeedback provides NoHaptics, content = content)
}
