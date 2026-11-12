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

enum class Role {
    BABY,
    PARENT,
}

/** Link state per ADR-017 §7; IDLE = monitoring not started. */
enum class LinkPhase {
    IDLE,
    CONNECTING,
    CONNECTED,
    RECONNECTING,
    PEER_STOPPED,
    FAILED_NEEDS_USER,
    STOPPED,
}

enum class CauseHint {
    SERVER_UNREACHABLE,
    PEER_ABSENT,
    MEDIA_BLOCKED,
    /** This (baby) device holds no secret for the parent and its invite is over: pair again. */
    PAIR_AGAIN,
    /**
     * Parent: the baby device does not recognise this device. Only a new code on the baby device
     * recovers it.
     */
    BABY_UNRECOGNISED,
}

/**
 * A calm hint of the core (never an alarm): the clocks of the two devices differ by more than 10
 * minutes, or no mailbox was reachable at the last reachability check while the link is verified.
 */
enum class LinkHint {
    CLOCK_SKEW,
    RECOVERY_PATH_UNAVAILABLE,
}

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
    /**
     * The baby unit has been absent for over 10 minutes: hint that a re-paired baby needs a new
     * scan.
     */
    val stalePairingHint: Boolean = false,
    /** Calm hints currently raised by the core; the UI renders them later (tasks 5.3, 6.2). */
    val hints: Set<LinkHint> = emptySet(),
)

enum class LinkQuality {
    GOOD,
    MEDIUM,
    POOR,
    UNKNOWN,
}

/** One value for the parent (ADR-018 §3.3). */
enum class SilenceStatus {
    SILENT,
    SCREEN_MAY_LIGHT,
    MAY_BUZZ,
    MAY_RING,
    UNKNOWN,
    NO_ACCESS,
}

data class PeerStatusUi(
    val batteryPct: Int? = null,
    val charging: Boolean? = null,
    val silence: SilenceStatus = SilenceStatus.UNKNOWN,
    val alarmsAllowed: Boolean? = null,
    val nextAlarmMinutes: Int? = null,
    val micSilent: Boolean? = null,
    /** Baby is a browser: silence can never be read or set. */
    val isWebClient: Boolean = false,
    /**
     * Wire names of the baby unit's own warnings, e.g. "not_charging", "mic_permission_lost",
     * "page_hidden".
     */
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

enum class ListenMode {
    OFF,
    HOLD,
    LOCKED,
}

enum class SilenceRequest {
    IDLE,
    PENDING,
    APPLIED,
    FAILED,
}

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

/** Talk-back volume of the baby device (a baby setting; `add-talk-back` design §7). */
enum class TalkVolume {
    LOW,
    MEDIUM,
    HIGH,
}

/** Why a talk ended on the parent device (core `TalkEndReason`). */
enum class TalkEnd {
    RELEASED,
    LIMIT,
    NO_ANSWER,
    LINK_LOST,
    MIC_FAILED,
    ALERT,
    SWITCHED_OFF,
    STOPPED,
}

/** Why the baby device refused a talk (core `TalkDenyReason`; UNKNOWN for a newer reason). */
enum class TalkDeny {
    OFF,
    UNSUPPORTED,
    MUTED,
    BUSY,
    COOLDOWN,
    BUDGET,
    RATE,
    UNKNOWN,
}

/** Why the talk button is disabled before a press. */
enum class TalkUnavailable {
    OFF,
    OTHER_TALKING,
    UNSUPPORTED,
    MUTED,
    NOT_CONNECTED,
    COOLDOWN,
}

/** The talk button's state; times are on the elapsed clock. */
sealed interface TalkUi {
    data object Idle : TalkUi

    /** "One moment…": the request is on its way. */
    data object Requesting : TalkUi

    /**
     * "Speak now": the microphone is attached; the baby device ends the talk at [untilElapsedMs].
     */
    data class Talking(val untilElapsedMs: Long) : TalkUi

    data class Denied(val reason: TalkDeny, val retryAtElapsedMs: Long? = null) : TalkUi

    data class Unavailable(val reason: TalkUnavailable, val retryAtElapsedMs: Long? = null) : TalkUi
}

/** Microphone permission step of the talk button (a press without permission sends nothing). */
enum class TalkPermission {
    NONE,
    NEEDED,
    JUST_GRANTED,
    REFUSED,
}

/** Who pauses noise detection: this device's own talk, or the other parent device's. */
enum class NoisePause {
    YOU,
    OTHER,
}

/**
 * Baby: the last talk, local only ("Last talk: {name}, {time}, {duration}"). [atWallMs] is the wall
 * time at which the talk started (its end minus [durationMs]), the same on every client.
 */
data class LastTalkUi(val name: String, val atWallMs: Long, val durationMs: Long)

data class ParentMonitorState(
    val link: LinkUi = LinkUi(),
    val peer: PeerStatusUi = PeerStatusUi(),
    /** Live level 0..1 of the received baby audio (meter). */
    val level: Float = 0f,
    /**
     * Start of the current quiet period; null while disconnected or unknown (never "All quiet"
     * then).
     */
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
    /**
     * The baby unit's real noise threshold (from peer_status), so the meter can draw it; null until
     * known.
     */
    val babyThreshold: Float? = null,
    val lastSound: SoundEventUi? = null,
    /** Parent phone's own battery percent, for "Your phone is at {percent}". */
    val ownBatteryPct: Int? = null,
    /**
     * The running noise event switched from discreet to sound after this many minutes without a
     * reaction (the core's `noise_escalated`); null otherwise.
     */
    val noiseEscalatedAfterMin: Int? = null,
    /** Talk-back (`add-talk-back`). */
    val talk: TalkUi = TalkUi.Idle,
    /** How the last talk ended; null before the first and after the next press. */
    val talkEnd: TalkEnd? = null,
    val talkPermission: TalkPermission = TalkPermission.NONE,
    val noisePause: NoisePause? = null,
    /** The baby device's playback check could not hear the last voice ("may not be audible"). */
    val talkUnconfirmed: Boolean = false,
    /** Three unconfirmed talks in a row: shown before the next press, never a denial. */
    val talkAudibleUncertain: Boolean = false,
    /**
     * Your own microphone has picked up (almost) no sound for a while during the talk: shown as a
     * line, never spoken while the microphone is open.
     */
    val talkMicSilent: Boolean = false,
    /** The 10 s listen after a talk runs until then ("tap to keep listening"). */
    val afterTalkListenUntilElapsedMs: Long? = null,
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
    /**
     * The name the pinned parent device sent over the verified DataChannel ("Connected to {name}");
     * null unless connected, empty when it sent none.
     */
    val parentName: String? = null,
    /** A device that is not the paired parent tried to connect (the core refused it). */
    val securityWarning: Boolean = false,
    /** A parent's voice plays now (static indicator under the edge). */
    val parentTalking: Boolean = false,
    /** The last finished talk of this run, local only. */
    val lastTalk: LastTalkUi? = null,
)

enum class PairingPhase {
    SHOWING_CODE,
    EXPIRED,
    SCANNING,
    CONNECTING,
    PAIRED,
    FAILED,
}

enum class PairingError {
    CAMERA_DENIED,
    NOT_OUR_CODE,
    EXPIRED,
    KEY_MISMATCH,
    ALREADY_PAIRED,
    UNREACHABLE,
    INCOMPLETE_PASTE,
}

data class PairingState(
    val phase: PairingPhase,
    /** Baby: QR / copyable payload. Never logged. */
    val codePayload: String? = null,
    val expiresAtElapsedMs: Long? = null,
    val error: PairingError? = null,
) {
    // The payload contains the PSK: keep it out of logs and crash reports.
    override fun toString(): String =
        "PairingState(phase=$phase, codePayload=${if (codePayload == null) "null" else "<redacted>"}, error=$error)"
}

enum class Theme {
    DARK,
    LIGHT,
    SYSTEM,
}

enum class AlertOutput {
    DEFAULT,
    SPEAKER_ONLY,
}

/**
 * How a noise alert reaches the parent (alert style setting): "Sound" plays the child's audio with
 * every alert, "Discreet" vibrates and shows the alert but plays no audio until the parent listens.
 */
enum class AlertStyle {
    SOUND,
    DISCREET,
}

/**
 * After how many minutes an unacknowledged discreet noise event switches to sound; [OFF] never does
 * (the settings warn that a forgotten discreet mode can then stay silent all night).
 */
enum class DiscreetEscalation(val minutes: Int?) {
    ONE_MINUTE(1),
    TWO_MINUTES(2),
    FIVE_MINUTES(5),
    OFF(null),
}

data class SettingsState(
    val theme: Theme = Theme.DARK,
    /** null = follow system; else "en", "de", "fr", "es". */
    val language: String? = null,
    val threshold: Float = 0.12f,
    val holdSeconds: Int = 5,
    val alertOutput: AlertOutput = AlertOutput.DEFAULT,
    val alertStyle: AlertStyle = AlertStyle.SOUND,
    val discreetEscalation: DiscreetEscalation = DiscreetEscalation.TWO_MINUTES,
    val alertGainPct: Int = 100,
    val silenceWhileMonitoring: Boolean = true,
    val advancedEnabled: Boolean = false,
    val signalingUrls: List<String> = emptyList(),
    /**
     * "Only same Wi-Fi or my own server": this device contacts no project server (no mailbox, STUN
     * or TURN) and keeps it out of the invites it builds. Task 5.3 adds the control.
     */
    val onlyLocal: Boolean = false,
    /** Mirrors the sealed TURN entry; never persisted by `updateSettings`. */
    val turnUrl: String? = null,
    val turnUsername: String? = null,
    /** The password itself never leaves the platform layer. */
    val turnPasswordSaved: Boolean = false,
    val heartbeatSeconds: Int = 10,
    /** UI-pass addition: last successful test alert (settings line "Last test"). */
    val lastTestWallMs: Long? = null,
    /** Baby: parents may talk through this device (switch of the single pairing; default on). */
    val talkBackEnabled: Boolean = true,
    val talkVolume: TalkVolume = TalkVolume.LOW,
    /** Baby: a soft chime announces a talk. */
    val talkChime: Boolean = true,
    /** Parent: the one-time "hold to talk" coach mark was shown. */
    val talkCoachSeen: Boolean = false,
)

/** Outcome of [AppController.setTurnServer]. */
enum class TurnSaveResult {
    SAVED,
    CLEARED,
    INVALID_ADDRESS,
    INVALID_CREDENTIALS,
}

/**
 * One saved connection (pairing) of the session role. The name is local and private: it is not part
 * of [toString], so it never reaches a log.
 */
data class ConnectionUi(
    /** The stable local id of the pairing. */
    val id: String,
    /** The connection name; null while there is none (the UI shows a numbered default). */
    val name: String?,
    val isDefault: Boolean,
    /** This is the connection monitoring uses (or will use) in this session. */
    val selected: Boolean,
) {
    override fun toString(): String =
        "ConnectionUi(id=$id, isDefault=$isDefault, selected=$selected)"

    companion object {
        /** The longest connection name, counted in code points (`pairing` spec: 1 to 30). */
        const val MAX_NAME_LENGTH = 30
    }
}

data class AppState(
    /** The session role; null shows the role screen. */
    val role: Role? = null,
    /** "Start mode": the role a start without monitoring opens in; null asks every time. */
    val defaultRole: Role? = null,
    val onboardingDone: Boolean = false,
    /** A saved pairing exists (home screen shows one-tap start). */
    val paired: Boolean = false,
    val peerName: String? = null,
    /**
     * The saved pairings of the session role, in slot order. The list is shown only with more than
     * one entry.
     */
    val connections: List<ConnectionUi> = emptyList(),
    /** Fewer than the cap of pairings are saved: a further one can be added. */
    val canAddPairing: Boolean = true,
    val monitoring: Boolean = false,
    /**
     * False while monitoring is on but Android is not running the foreground service: show red,
     * never green.
     */
    val backgroundProtection: Boolean = true,
    /** UI-pass additions. */
    val lastUsedWallMs: Long? = null,
    /** Show the first-run coach marks on the parent monitoring screen once. */
    val coachPending: Boolean = false,
    val deviceCheck: DeviceCheck = DeviceCheck.UNKNOWN,
)

/** Baby self-test result (ADR-010) shown on the setup checklist. */
enum class DeviceCheck {
    UNKNOWN,
    OK,
    LIMITED,
    NOT_RECOMMENDED,
}

interface AppController {
    val app: StateFlow<AppState>
    val settings: StateFlow<SettingsState>
    val parent: StateFlow<ParentMonitorState>
    val baby: StateFlow<BabyMonitorState>
    val pairing: StateFlow<PairingState?>

    /**
     * Sets the session role (the pairing of each role is kept). [makeDefault] also stores it as the
     * start mode.
     */
    fun chooseRole(role: Role, makeDefault: Boolean = false)

    /**
     * Back to the role screen without changing the start mode or any pairing; cancels an open
     * invite. Does nothing while monitoring is active.
     */
    fun switchMode()

    /** Sets the start mode: a role, or null for "Ask every time". */
    fun setDefaultRole(role: Role?)

    fun completeOnboarding()

    fun startPairing()

    fun cancelPairing()

    /** Parent: decoded QR text or pasted code. */
    fun submitPairingCode(payload: String)

    fun refreshPairingCode()

    fun startMonitoring()

    fun stopMonitoring()

    /** Forgets the pairing this session uses (the selected connection). */
    fun forgetPairing()

    // Named connections (`multi-parent-pairings`).
    /** Picks the connection of this session (not while monitoring); the default is not changed. */
    fun selectConnection(id: String)

    /** Renames a connection; false when the name is not 1 to 30 characters. */
    fun renameConnection(id: String, name: String): Boolean

    fun setDefaultConnection(id: String)

    /** Forgets exactly one connection; monitoring stops first when it is the one in use. */
    fun forgetConnection(id: String)

    fun updateSettings(transform: (SettingsState) -> SettingsState)

    /**
     * Saves the one TURN server of this device. An empty [url] removes it. An empty [password]
     * keeps the saved one while [url] and [username] are unchanged.
     */
    fun setTurnServer(url: String, username: String, password: String): TurnSaveResult

    fun clearTurnServer()

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

    // Talk-back (`add-talk-back`); defaults keep other implementations compiling.
    /** Parent: the talk button went down or up (a press without permission sends nothing). */
    fun setTalk(pressed: Boolean) {}

    /** Parent: the app left the foreground, the screen turned off or a call started. */
    fun interruptTalk() {}

    /** Parent: the answer of the system microphone permission dialog. */
    fun talkPermissionResult(granted: Boolean) {}

    fun dismissTalkPermission() {}

    fun dismissTalkCoach() {}

    /** Baby: "Play chime" in the settings, at the chosen level. */
    fun playTalkChimeSample() {}

    /** Baby: an adult opened the talk-back settings (ends the "may not be audible" warning). */
    fun talkSettingsOpened() {}
}
