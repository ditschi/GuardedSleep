package org.guardedsleep.app.config

import org.guardedsleep.app.platform.BuildConfig

/**
 * Owner defaults that are still pending confirmation (ADR-018 §6). Everything that may change after
 * the owner decides lives here, so a decision is a one-line edit.
 */
object AlertPolicy {
    /** Level 1 (20 s without contact): visual only, no vibration (alarm fatigue on flaky Wi-Fi). */
    const val LEVEL1_VIBRATES = false

    /** Level 2 (60 s): repeating connection haptic. */
    const val LEVEL2_HAPTIC_INTERVAL_MS = 10_000L

    /**
     * Level 2: short soft tone on the alarm stream, every 10 s like a classic hardware monitor
     * (owner, final).
     */
    const val LEVEL2_TONE_INTERVAL_MS = 10_000L

    /** Level 3 (180 s): continuous alarm until acknowledged or restored. */
    const val LEVEL3_CONTINUOUS = true

    /** Noise alert: repeating vibration while the event is not stopped. */
    const val NOISE_HAPTIC_INTERVAL_MS = 4_000L

    /** A new noise within this time after an event ended continues it (alpha-ux-flows §3.7). */
    const val NOISE_EVENT_JOIN_MS = 120_000L

    /** The meter must stay below the threshold this long before the event counts as quiet. */
    const val NOISE_QUIET_AFTER_MS = 120_000L

    /** Locked listening stops by itself (alpha-ux-flows §3.8). */
    const val LISTEN_MAX_MS = 60L * 60_000L

    /** "Connected again after m:ss" stays visible this long. */
    const val RESTORED_NOTICE_MS = 10_000L

    /** Without a `missed_events` summary the parent restarts "quiet since" at the reconnect. */
    const val MISSED_EVENTS_GRACE_MS = 5_000L
}

object PairingPolicy {
    /** QR code validity; single use, fresh PSK per code (ADR-018 §2). */
    const val QR_VALIDITY_MS = 5L * 60_000L
    const val PSK_BYTES = 32
}

object AppDefaults {
    /** The alpha has no public server; builds without one start with an empty list (Advanced). */
    val signalingUrls: List<String> =
        if (BuildConfig.DEBUG) listOf("ws://10.0.2.2:8787") else emptyList()

    const val ALARM_AFTER_MS = 60_000L
    const val TICK_MS = 250L
    const val AUDIO_LEVEL_MS = 100L
    const val DEVICE_STATUS_CHECK_MS = 2_000L
    const val DEVICE_STATUS_RESEND_MS = 10_000L
}
