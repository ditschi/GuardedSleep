package org.guardedsleep.app.ui.common

import android.content.Context
import android.os.SystemClock
import android.provider.Settings
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import java.util.TimeZone
import kotlinx.coroutines.delay

data class BatteryReading(val pct: Int, val charging: Boolean)

/**
 * Everything the UI reads from the environment, as one injectable value so screenshot tests are
 * deterministic (fixed clocks, no ticker, no looping animations, no real camera/battery).
 */
data class UiEnv(
    val elapsedNow: () -> Long = { SystemClock.elapsedRealtime() },
    val wallNow: () -> Long = { System.currentTimeMillis() },
    /** Live countdowns re-read the clocks once per second. */
    val ticking: Boolean = true,
    /** Pulses become static colour + icon (accessibility, §6.1). */
    val reduceMotion: Boolean = false,
    val zone: TimeZone = TimeZone.getDefault(),
    /** null = ask the system. */
    val cameraGranted: Boolean? = null,
    /** null = read the sticky battery broadcast. */
    val battery: BatteryReading? = null,
)

val LocalUiEnv = compositionLocalOf { UiEnv() }

@Composable
fun systemReduceMotion(): Boolean {
    val ctx = LocalContext.current
    return remember(ctx) {
        runCatching {
                Settings.Global.getFloat(
                    ctx.contentResolver,
                    Settings.Global.ANIMATOR_DURATION_SCALE,
                    1f,
                ) == 0f
            }
            .getOrDefault(false)
    }
}

/** Elapsed-realtime clock that re-reads every second while [UiEnv.ticking]. */
@Composable
fun rememberElapsedNow(): State<Long> {
    val env = LocalUiEnv.current
    val state = remember { mutableLongStateOf(env.elapsedNow()) }
    LaunchedEffect(env.ticking, env.elapsedNow) {
        state.longValue = env.elapsedNow()
        while (env.ticking) {
            delay(1000)
            state.longValue = env.elapsedNow()
        }
    }
    return remember {
        object : State<Long> {
            override val value: Long
                get() = state.longValue
        }
    }
}

@Composable
fun rememberWallNow(): State<Long> {
    val env = LocalUiEnv.current
    val state = remember { mutableLongStateOf(env.wallNow()) }
    LaunchedEffect(env.ticking, env.wallNow) {
        state.longValue = env.wallNow()
        while (env.ticking) {
            delay(1000)
            state.longValue = env.wallNow()
        }
    }
    return remember {
        object : State<Long> {
            override val value: Long
                get() = state.longValue
        }
    }
}

fun Context.findActivity(): android.app.Activity? {
    var c: Context? = this
    while (c is android.content.ContextWrapper) {
        if (c is android.app.Activity) return c
        c = c.baseContext
    }
    return null
}
