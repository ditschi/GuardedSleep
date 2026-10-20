package org.guardedsleep.app.ui.model

import kotlinx.coroutines.flow.StateFlow

/*
 * Contract between the Compose UI and the platform layer (ADR-018, alpha-ux-flows.md).
 * The UI renders only these plain models and calls only these actions; the platform layer maps the
 * Rust core's Link status/events (UniFFI) onto them. Keeping UniFFI types out of the UI lets
 * screenshot tests run on the JVM without the native core.
 *
 * Times: `*ElapsedMs` = SystemClock.elapsedRealtime() (durations, countdowns);
 * `*WallMs` = System.currentTimeMillis() (clock times shown to the user, e.g. "quiet since 02:14").
 */

enum class Role { BABY, PARENT }

/** Link state per ADR-017 §7; IDLE = monitoring not started. */
enum class LinkPhase { IDLE, CONNECTING, CONNECTED, RECONNECTING, PEER_STOPPED, FAILED_NEEDS_USER, STOPPED }

enum class CauseHint { SERVER_UNREACHABLE, PEER_ABSENT, MEDIA_BLOCKED }

data class LinkUi(
    val phase: LinkPhase = LinkPhase.IDLE,
    /** Escalation level 0..3 while not connected (ADR-017 §7); 0 when connected. */
    val level: Int = 0,
    /** Last contact with the peer; drives "not reachable for m:ss". */
    val lastAliveElapsedMs: Long? = null,
    /** "I'm checking" mute end (parent, level ≥ 2). */
    val acknowledgedUntilElapsedMs: Long? = null,
    val causeHint: CauseHint? = null,
    /** Message key of a terminal failure, e.g. `pairing_key_mismatch`, `update_required`. */
    val failureKey: String? = null,
    /** Weak connection shown only when POOR (link quality folded into the bar). */
    val quality: LinkQuality = LinkQuality.UNKNOWN,
    /** The baby unit has been absent for over 10 minutes: hint that a re-paired baby needs a new scan. */
    val stalePairingHint: Boolean = false,
)

enum class LinkQuality { GOOD, MEDIUM, POOR, UNKNOWN }

/** One value for the parent (ADR-018 §3.3). */
enum class SilenceStatus { SILENT, SCREEN_MAY_LIGHT, MAY_BUZZ, MAY_RING, UNKNOWN, NO_ACCESS }

data class PeerStatusUi(
    val batteryPct: Int? = null,
    val charging: Boolean? = null,
    val silence: SilenceStatus = SilenceStatus.UNKNOWN,
    val alarmsAllowed: Boolean? = null,
    val nextAlarmMinutes: Int? = null,
    val micSilent: Boolean? = null,
    /** Baby is a browser: silence can never be read or set. */
    val isWebClient: Boolean = false,
    /** Wire names of the baby unit's own warnings, e.g. "not_charging", "mic_permission_lost", "page_hidden". */
    val unitWarnings: List<String> = emptyList(),
)

enum class ReadinessIssue {
    NOTIFICATIONS_BLOCKED,
    ALARM_VOLUME_ZERO,
    FULL_SCREEN_BLOCKED,
    DND_BYPASS_MISSING,
    BATTERY_OPTIMIZED,
    PARENT_BATTERY_LOW,
    HEADPHONES_CONNECTED,
    NEVER_TESTED,
}

enum class ListenMode { OFF, HOLD, LOCKED }

enum class SilenceRequest { IDLE, PENDING, APPLIED, FAILED }

data class MissedEventsUi(
    val downMs: Long,
    val count: Int?,
    val complete: Boolean,
    /** UI-pass addition: end of the gap (wall clock); null = now. Start is `end - downMs`. */
    val endWallMs: Long? = null,
    /** UI-pass addition: loudest level 0..1 during the gap, when known. */
    val loudest: Float? = null,
)

/** UI-pass addition: the last finished noise event, for "Sound at {time} · lasted {duration}". */
data class SoundEventUi(val atWallMs: Long, val durationMs: Long)

data class ParentMonitorState(
    val link: LinkUi = LinkUi(),
    val peer: PeerStatusUi = PeerStatusUi(),
    /** Live level 0..1 of the received baby audio (meter). */
    val level: Float = 0f,
    /** Start of the current quiet period; null while disconnected or unknown (never "All quiet" then). */
    val quietSinceWallMs: Long? = null,
    val noiseAlertActive: Boolean = false,
    val listen: ListenMode = ListenMode.OFF,
    val snoozedUntilWallMs: Long? = null,
    val readiness: List<ReadinessIssue> = emptyList(),
    val silenceRequest: SilenceRequest = SilenceRequest.IDLE,
    val missed: MissedEventsUi? = null,
    /** Set for a few seconds after recovery: "Connected again after m:ss". */
    val restoredAfterMs: Long? = null,
    val securityWarning: Boolean = false,
    /** UI-pass additions (defaults keep the platform layer source compatible). */
    val noiseSinceWallMs: Long? = null,
    val noiseIsTest: Boolean = false,
    /** The baby unit's real noise threshold (from peer_status), so the meter can draw it; null until known. */
    val babyThreshold: Float? = null,
    val lastSound: SoundEventUi? = null,
    /** Parent phone's own battery percent, for "Your phone is at {percent}". */
    val ownBatteryPct: Int? = null,
)

data class BabyMonitorState(
    val link: LinkUi = LinkUi(),
    val micLevel: Float = 0f,
    val threshold: Float = 0.12f,
    val holdSeconds: Int = 5,
    val silenceApplied: Boolean? = null,
    val dndAccess: Boolean = false,
    val kioskLocked: Boolean = false,
    /** A previous DND rule was restored after a crash; show the notice once. */
    val silenceRestoredNotice: Boolean = false,
    val micSilent: Boolean = false,
    /** UI-pass addition: the parent is listening (shown on touch-wake only). */
    val parentListening: Boolean = false,
)

enum class PairingPhase { SHOWING_CODE, EXPIRED, SCANNING, CONNECTING, PAIRED, FAILED }

enum class PairingError { CAMERA_DENIED, NOT_OUR_CODE, EXPIRED, KEY_MISMATCH, ALREADY_PAIRED, UNREACHABLE, INCOMPLETE_PASTE }

data class PairingState(
    val phase: PairingPhase,
    /** Baby: QR / copyable payload. Never logged. */
    val codePayload: String? = null,
    val expiresAtElapsedMs: Long? = null,
    val error: PairingError? = null,
) {
    // The payload contains the PSK: keep it out of logs and crash reports.
    override fun toString(): String = "PairingState(phase=$phase, codePayload=${if (codePayload == null) "null" else "<redacted>"}, error=$error)"
}

enum class Theme { DARK, LIGHT, SYSTEM }

enum class AlertOutput { DEFAULT, SPEAKER_ONLY }

data class SettingsState(
    val theme: Theme = Theme.DARK,
    /** null = follow system; else "en", "de", "fr", "es". */
    val language: String? = null,
    val threshold: Float = 0.12f,
    val holdSeconds: Int = 5,
    val alertOutput: AlertOutput = AlertOutput.DEFAULT,
    val alertGainPct: Int = 100,
    val silenceWhileMonitoring: Boolean = true,
    val advancedEnabled: Boolean = false,
    val signalingUrls: List<String> = emptyList(),
    val turnUrl: String? = null,
    val heartbeatSeconds: Int = 10,
    /** UI-pass addition: last successful test alert (settings line "Last test"). */
    val lastTestWallMs: Long? = null,
)

data class AppState(
    val role: Role? = null,
    val onboardingDone: Boolean = false,
    /** A saved pairing exists (home screen shows one-tap start). */
    val paired: Boolean = false,
    val peerName: String? = null,
    val monitoring: Boolean = false,
    /** False while monitoring is on but Android is not running the foreground service: show red, never green. */
    val backgroundProtection: Boolean = true,
    /** UI-pass additions. */
    val lastUsedWallMs: Long? = null,
    /** Show the first-run coach marks on the parent monitoring screen once. */
    val coachPending: Boolean = false,
    val deviceCheck: DeviceCheck = DeviceCheck.UNKNOWN,
)

/** Baby self-test result (ADR-010) shown on the setup checklist. */
enum class DeviceCheck { UNKNOWN, OK, LIMITED, NOT_RECOMMENDED }

interface AppController {
    val app: StateFlow<AppState>
    val settings: StateFlow<SettingsState>
    val parent: StateFlow<ParentMonitorState>
    val baby: StateFlow<BabyMonitorState>
    val pairing: StateFlow<PairingState?>

    fun chooseRole(role: Role)
    fun completeOnboarding()
    fun startPairing()
    fun cancelPairing()
    /** Parent: decoded QR text or pasted code. */
    fun submitPairingCode(payload: String)
    fun refreshPairingCode()
    fun startMonitoring()
    fun stopMonitoring()
    fun forgetPairing()
    fun updateSettings(transform: (SettingsState) -> SettingsState)

    // Parent
    fun setListen(mode: ListenMode)
    fun requestTestAlert()
    fun snooze(minutes: Int)
    fun cancelSnooze()
    fun acknowledgeAlarm()
    fun stopAlertSound()
    fun requestSilenceBaby()
    fun tryReconnectNow()
    fun dismissMissedEvents()

    // Baby
    fun setKioskLocked(locked: Boolean)
    fun dismissSilenceRestoredNotice()

    // UI-pass additions: default no-ops so existing implementations keep compiling.
    /** Parent answered "Did you hear and feel it?" after a test alert. */
    fun reportTestResult(heardAndFelt: Boolean) {}
    fun dismissCoach() {}
}
