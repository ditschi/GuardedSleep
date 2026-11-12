/**
 * Platform timing constants. None of them is a safety decision: those live in the Rust core
 * (escalation levels, heartbeats, reconnect back-off). These only set how often the platform
 * feeds the core (tick, audio level), how long an announced stop is flushed, and UI-side
 * windows that mirror the Android platform layer (`AlertPolicy`, `AppDefaults`).
 */

/** The core's clock input; the core schedules everything else against it. */
export const TICK_MS = 250;
export const AUDIO_LEVEL_MS = 100;
/** Baby: how often the device status is read; it is sent on change or after the resend time. */
export const DEVICE_STATUS_CHECK_MS = 2_000;
export const DEVICE_STATUS_RESEND_MS = 10_000;
/** Parent: inbound audio counters for the link-quality hint. */
export const STATS_MS = 5_000;
/** A stop notice written to the DataChannel needs a moment to leave before the close. */
export const FLUSH_MS = 300;
/** The remote DTLS fingerprint appears in `getStats()` shortly after `connected`. */
export const FINGERPRINT_TRIES = 10;
export const FINGERPRINT_RETRY_MS = 300;

export const PSK_BYTES = 32;
/** QR / invite validity, single use (ADR-018 section 2). */
export const INVITE_VALIDITY_MS = 5 * 60_000;
/** Parent pairing shows "server not reachable" after this long without a verified link. */
export const PAIRING_UNREACHABLE_AFTER_MS = 20_000;

export const ALARM_AFTER_MS = 60_000;
export const MIN_HEARTBEAT_MS = 5_000;
export const MAX_HEARTBEAT_MS = 15_000;

/** A new noise within this time after an event ended continues it (alpha-ux-flows 3.7). */
export const NOISE_EVENT_JOIN_MS = 120_000;
/** The meter must stay below the threshold this long before the event counts as quiet. */
export const NOISE_QUIET_AFTER_MS = 120_000;
/** Locked listening stops by itself (alpha-ux-flows 3.8). */
export const LISTEN_MAX_MS = 60 * 60_000;
/** "Connected again after m:ss" stays visible this long. */
export const RESTORED_NOTICE_MS = 10_000;
/** Without a `missed_events` summary the parent restarts "quiet since" at the reconnect. */
export const MISSED_EVENTS_GRACE_MS = 5_000;

export const LEVEL2_TONE_INTERVAL_MS = 10_000;
export const NOISE_TONE_INTERVAL_MS = 4_000;

/** A stay in the background shorter than this is not reported on return. */
export const AWAY_REPORT_MIN_MS = 15_000;
/** The baby device keeps reporting "woke from sleep" this long after it did. */
export const RESUMED_WARNING_MS = 10 * 60_000;
/** How often the page watch looks at its own clock to notice a sleep (no link decision). */
export const GAP_CHECK_MS = 1_000;
/** Silence this long on a visible page means the device slept or was suspended. */
export const SLEEP_GAP_MS = 10_000;
/** A hidden page's timers run at most once a minute; only a clearly longer silence counts. */
export const BACKGROUND_GAP_MS = 150_000;

// ---- talk-back (add-talk-back design sections 7 and 8) ----------------------------------------
// Platform-side only: when a talk may play, how long and how often is decided in the Rust core.

/** Baby: how often the talk receiver's packet counter is compared with the lease. */
export const TALK_RTP_POLL_MS = 1_000;
/** Packets this soon after a lease ended are the parent's last frames in flight, not a breach. */
export const TALK_RTP_GRACE_MS = 2_000;
/**
 * Baby output gains per talk level, as linear factors of the computer's own volume (which a
 * browser cannot see). High is lower than the Android share on purpose (design section 8).
 */
export const TALK_LEVEL_GAIN = { low: 0.2, medium: 0.4, high: 0.6 } as const;
/** The limiter's hard clip, as a linear amplitude in front of the system volume. */
export const TALK_LIMITER_CEILING = 0.5;
/** The compressor of the limiter: threshold in dB, ratio, attack and release in seconds. */
export const TALK_LIMITER_THRESHOLD_DB = -18;
export const TALK_LIMITER_RATIO = 20;
export const TALK_LIMITER_ATTACK_S = 0.001;
export const TALK_LIMITER_RELEASE_S = 0.1;
/** The chime sits this far below the voice level (about 12 dB) but never below the floor. */
export const TALK_CHIME_RATIO = 0.25;
export const TALK_CHIME_FLOOR = 0.05;
/** Two notes of the chime and how long each rings (seconds). */
export const TALK_CHIME_NOTES_HZ = [660, 880] as const;
export const TALK_CHIME_NOTE_S = 0.3;
/** The core's listen after a talk; the screen counts it with the core's own `listen_changed`. */
export const TALK_AFTER_LISTEN_MS = 10_000;
/** An end reason or denial stays on the parent's screen this long. */
/** How often the parent's own microphone level is read while a talk runs (stats, no sink). */
export const TALK_MIC_POLL_MS = 500;
/** The own-microphone hint appears after this long without sound while talking. */
export const TALK_MIC_SILENT_MS = 2_000;
/** Below this level (0..1) the parent's own microphone counts as silent. */
export const TALK_MIC_SILENT_BELOW = 0.002;
export const TALK_RESULT_VISIBLE_MS = 6_000;
/** "Your voice may not be audible" stays this long. */
export const TALK_UNCONFIRMED_VISIBLE_MS = 15_000;
