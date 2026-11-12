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

    /** The listen the core starts after a talk (`TALK_AFTER_LISTEN_MS`), for the countdown line. */
    const val TALK_AFTER_LISTEN_MS = 10_000L

    /** Without a `missed_events` summary the parent restarts "quiet since" at the reconnect. */
    const val MISSED_EVENTS_GRACE_MS = 5_000L
}

object PairingPolicy {
    /** QR code validity; single use, fresh PSK per code (ADR-018 §2). */
    const val QR_VALIDITY_MS = 5L * 60_000L
    const val PSK_BYTES = 32
}

object AppDefaults {
    /**
     * Debug builds point at the project server so a CI debug APK pairs two real phones without
     * setup; the emulator gate passes its own URL as `signal_urls`, and a local emulator setup
     * enters `ws://10.0.2.2:8787` in Advanced. Release builds stay empty until the disclosure and
     * off switch exist (alpha-connection-model task 5.3).
     */
    val signalingUrls: List<String> =
        if (BuildConfig.DEBUG) listOf("wss://gs.mydata.rocks/signaling") else emptyList()

    const val ALARM_AFTER_MS = 60_000L
    const val TICK_MS = 250L
    const val AUDIO_LEVEL_MS = 100L
    const val DEVICE_STATUS_CHECK_MS = 2_000L
    const val DEVICE_STATUS_RESEND_MS = 10_000L
}

/**
 * Talk-back playout on the baby unit (`add-talk-back` design §7). The numbers marked provisional
 * are tuned in the hardware round (task 2.1 (f)); the emulator cannot measure a loudspeaker.
 */
object TalkPolicy {
    /** Fraction of the alarm stream's maximum per level; high stays at or below about 70 %. */
    const val LOW_FRACTION = 0.25
    const val MEDIUM_FRACTION = 0.45
    const val HIGH_FRACTION = 0.70

    /** Ceiling of the received voice's RMS after the gain (0..1 of full scale), provisional. */
    const val CEILING_RMS = 0.30

    /** The level control gives the gain back by this much per level sample (about 10 Hz). */
    const val LIMITER_RELEASE_PER_TICK = 0.02

    /** Chime peak amplitude relative to full scale: about 12 dB under a loud voice. */
    const val CHIME_PEAK = 0.35

    /** The chime is never quieter than this (audible floor), provisional. */
    const val CHIME_FLOOR = 0.08
    const val CHIME_MS = 600

    /** Talk RTP inside this long after a lease ended is the sender's tail, not a violation. */
    const val RTP_GRACE_MS = 1_500L

    /** More packets than this outside a lease and its grace count as audio without a grant. */
    const val RTP_OUTSIDE_PACKETS = 5L

    /** Level samples of the talk receiver and the outside-grant check use this period. */
    const val TALK_POLL_MS = 100L
    const val OUTSIDE_POLL_MS = 1_000L
}
