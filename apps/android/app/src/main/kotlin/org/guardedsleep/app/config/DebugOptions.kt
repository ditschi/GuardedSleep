package org.guardedsleep.app.config

import org.guardedsleep.app.BuildConfig

/**
 * Test knobs of the emulator gate. Only the debug-only `DebugLaunchActivity` writes them; every
 * reader is additionally guarded by [BuildConfig.DEBUG], so a release build ignores them.
 */
object DebugOptions {
    @Volatile private var relayOnlyValue = false
    @Volatile private var autoListenValue = false
    @Volatile private var heartbeatMsValue: Long? = null
    @Volatile private var alarmAfterMsValue: Long? = null
    @Volatile private var noiseHoldMsValue: Long? = null

    val relayOnly: Boolean
        get() = BuildConfig.DEBUG && relayOnlyValue

    val autoListen: Boolean
        get() = BuildConfig.DEBUG && autoListenValue

    val heartbeatMs: Long?
        get() = if (BuildConfig.DEBUG) heartbeatMsValue else null

    val alarmAfterMs: Long?
        get() = if (BuildConfig.DEBUG) alarmAfterMsValue else null

    val noiseHoldMs: Long?
        get() = if (BuildConfig.DEBUG) noiseHoldMsValue else null

    fun set(
        relayOnly: Boolean,
        autoListen: Boolean,
        heartbeatMs: Long?,
        alarmAfterMs: Long?,
        noiseHoldMs: Long? = null,
    ) {
        noiseHoldMsValue = noiseHoldMs
        relayOnlyValue = relayOnly
        autoListenValue = autoListen
        heartbeatMsValue = heartbeatMs
        alarmAfterMsValue = alarmAfterMs
    }
}
