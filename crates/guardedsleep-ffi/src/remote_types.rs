//! UniFFI declarations for the core types that cross the Kotlin/Swift boundary. The core carries no
//! binding attributes (ADR-020), so each type is declared again here with `#[uniffi::remote]`. The
//! field and variant lists must match the core definitions exactly; a mismatch fails to compile.
//! Docs: <https://mozilla.github.io/uniffi-rs/latest/proc_macro/index.html#the-uniffiremote-attribute>

use guardedsleep_core::link::{
    LinkAction, LinkConfig, LinkEvent, LinkInput, LinkState, LinkStatus,
};
use guardedsleep_core::protocol::{DeviceStatus, Dnd, IceCandidate, Ringer, Role, UnitWarning};
use guardedsleep_core::session::{Action, Input, SessionConfig, SessionEvent, SessionState};

#[uniffi::remote(Enum)]
pub enum Role {
    Baby,
    Parent,
}

#[uniffi::remote(Record)]
pub struct IceCandidate {
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_m_line_index: Option<u32>,
}

#[uniffi::remote(Enum)]
pub enum Ringer {
    Normal,
    Vibrate,
    Silent,
    Unknown,
}

#[uniffi::remote(Enum)]
pub enum Dnd {
    Off,
    AlarmsOnly,
    Priority,
    Total,
    Unknown,
}

/// Why a baby unit may stop monitoring reliably (mainly a web/laptop baby). Variant order is the
/// stable wire order.
#[uniffi::remote(Enum)]
pub enum UnitWarning {
    PageHidden,
    WakeLockOff,
    ResumedFromSleep,
    NotCharging,
    MicPermissionLost,
}

/// Coarse baby-phone state shown to the parent (ADR-018). Every field may be unknown (`None`).
#[uniffi::remote(Record)]
pub struct DeviceStatus {
    /// Rounded to 5 by [`DeviceStatus::normalized`].
    pub battery_pct: Option<u8>,
    pub charging: Option<bool>,
    pub ringer: Option<Ringer>,
    pub dnd: Option<Dnd>,
    pub alarms_allowed: Option<bool>,
    /// The app may control Do Not Disturb.
    pub dnd_access: Option<bool>,
    /// Our own silence rule is active.
    pub quiet_active: Option<bool>,
    /// Minutes until the next alarm clock, rounded to multiples of 15 and capped at
    /// [`MAX_NEXT_ALARM_MIN`].
    pub next_alarm_min: Option<u32>,
    /// The microphone delivered only digital silence for 30 s (set by the core, not the platform).
    pub mic_silent: Option<bool>,
    /// The baby's live noise-gate threshold (0.01..=1.0), set by the core.
    pub noise_threshold: Option<f64>,
    /// Reliability warnings reported by the baby platform; sorted and deduplicated by
    /// [`DeviceStatus::normalized`] and the parser, omitted on the wire when empty.
    pub unit_warnings: Vec<UnitWarning>,
}

#[uniffi::remote(Record)]
pub struct LinkConfig {
    pub role: Role,
    /// Pairing PSK (base64url). The room id is derived from it.
    pub psk_b64url: String,
    /// The PSK was verified successfully in an earlier run, so a mismatch is never terminal.
    pub paired_before: bool,
    /// 5000..=15000.
    pub heartbeat_interval_ms: u64,
    /// Escalation level 2 starts this long after the last sign of life.
    pub alarm_after_ms: u64,
    pub noise_threshold: f64,
    pub noise_hold_ms: u64,
    /// A link only becomes verified once the platform also reported the remote DTLS certificate
    /// fingerprint (`attempt_dtls_fingerprint`) and it matches the remote SDP. Defaults to `true`;
    /// switch off only for a platform that cannot read WebRTC certificate stats.
    pub require_dtls_fingerprint: bool,
}

#[uniffi::remote(Enum)]
pub enum LinkState {
    /// Started, never verified in this run.
    Connecting,
    Connected,
    Reconnecting,
    /// The peer announced a deliberate stop (ADR-017 §7); no escalation runs.
    PeerStopped,
    FailedNeedsUser,
    Stopped,
}

#[uniffi::remote(Record)]
pub struct LinkStatus {
    pub state: LinkState,
    /// Escalation level 0-3 while `connecting` / `reconnecting`, otherwise 0.
    pub level: u8,
    /// `failed_needs_user` key (see [`message_keys`]).
    pub key: Option<String>,
    /// Troubleshooter hint from level 2 (see [`message_keys`]).
    pub cause: Option<String>,
    /// Parent: sound and vibration of levels 2-3 are muted until this time.
    pub acknowledged_until: Option<u64>,
    /// Monotonic time (same clock as the inputs) of the last verified contact with the peer, so a
    /// UI can show "last contact N min ago". Set while `connecting` (after a first contact),
    /// `reconnecting` and `peer_stopped`; `None` while `connected` (the contact is "now", and a
    /// moving value would re-emit the status on every message) and before any contact.
    pub last_alive_ms: Option<u64>,
}

#[uniffi::remote(Enum)]
pub enum LinkInput {
    Start {
        entropy: u64,
    },
    Stop,
    /// Parent: "I'm checking" - mute levels 2-3 for 5 minutes.
    AcknowledgeAlarm,
    /// The platform's network changed or came back: retry immediately, reset the backoff.
    NetworkAvailable,
    SignalingOpened,
    SignalingMessage {
        text: String,
    },
    SignalingClosed,
    AttemptLocalDescription {
        attempt: u32,
        sdp_type: String,
        sdp: String,
    },
    AttemptLocalIce {
        attempt: u32,
        candidate: IceCandidate,
    },
    AttemptDataOpen {
        attempt: u32,
    },
    AttemptDataMessage {
        attempt: u32,
        text: String,
    },
    AttemptDataClosed {
        attempt: u32,
    },
    /// The SHA-256 fingerprint of the remote DTLS certificate, read from the WebRTC stats
    /// (`RTCCertificateStats` of the remote certificate) once DTLS connected. Colon-separated hex
    /// (`AB:CD:...`) or plain hex. Required for verification unless
    /// `LinkConfig::require_dtls_fingerprint` is off.
    AttemptDtlsFingerprint {
        attempt: u32,
        fingerprint: String,
    },
    /// `RTCPeerConnectionState` as lowercase string.
    AttemptPcState {
        attempt: u32,
        state: String,
    },
    /// Baby: microphone RMS 0..1, sampled ~10 Hz.
    AudioLevel {
        rms: f64,
    },
    /// Baby: current phone state (the core adds `mic_silent` itself).
    DeviceStatus {
        status: DeviceStatus,
    },
    /// Baby: change the noise gate while running. Out-of-range values (threshold 0.01..=1.0,
    /// `hold_ms` 1000..=15000) are ignored. A running loud state is kept (see `NoiseGate`).
    SetNoiseGate {
        threshold: f64,
        hold_ms: u64,
    },
    /// Parent: stream audio continuously.
    SetListen {
        enabled: bool,
    },
    /// Parent: ask the baby for one synthetic alert.
    RequestProbe,
    /// Parent: suppress noise alerts until `until_ms` (same monotonic clock); `None` clears.
    SetSnooze {
        until_ms: Option<u64>,
    },
    /// Parent: ask the baby phone to go silent. There is no remote "off".
    RequestQuiet,
    /// Baby: result of `apply_quiet` (fresh phone state).
    QuietApplied {
        state: DeviceStatus,
    },
    Tick,
}

#[uniffi::remote(Enum)]
pub enum LinkAction {
    OpenSignaling,
    CloseSignaling,
    SendSignaling {
        text: String,
    },
    CreateAttempt {
        attempt: u32,
        offerer: bool,
    },
    CloseAttempt {
        attempt: u32,
    },
    CreateOffer {
        attempt: u32,
    },
    SetRemoteDescription {
        attempt: u32,
        sdp_type: String,
        sdp: String,
    },
    CreateAnswer {
        attempt: u32,
    },
    AddIceCandidate {
        attempt: u32,
        candidate: IceCandidate,
    },
    SendData {
        attempt: u32,
        text: String,
    },
    SetMicEnabled {
        enabled: bool,
    },
    /// Baby: switch the phone to silent (ringer / DND), then answer with `quiet_applied`.
    ApplyQuiet,
    /// Baby: mix a synthetic tone into the outgoing stream; never play it on the baby phone.
    SendProbeTone {
        duration_ms: u64,
    },
    Emit {
        event: LinkEvent,
    },
}

#[uniffi::remote(Enum)]
pub enum LinkEvent {
    /// Emitted whenever the status changes.
    Status {
        status: LinkStatus,
    },
    /// Escalation reached level 2: this must reach the user as an alarm. Once per outage.
    LinkLost {
        down_ms: u64,
    },
    /// Verified again after an outage that reached level 1 or higher.
    LinkRestored {
        down_ms: u64,
    },
    /// A peer with a wrong key tried to connect after a successful pairing (key in `message_keys`).
    SecurityWarning {
        key: String,
    },
    /// The peer stopped monitoring on purpose. Emitted once per stop.
    PeerStopped,
    AlarmAcknowledged {
        until_ms: u64,
    },
    /// Parent: noise alert from the baby. `snoozed` alerts are for the log only.
    NoiseAlert {
        rms: f64,
        probe: bool,
        snoozed: bool,
    },
    PeerHeartbeat {
        seq: u64,
        rms: Option<f64>,
    },
    PeerStatus {
        status: DeviceStatus,
    },
    /// Parent: `<15 %` and not charging, or `<5 %`; once per crossing.
    LowBattery {
        pct: u8,
        critical: bool,
    },
    ChargerUnplugged,
    /// Parent: noise the baby buffered while the link was down. Show `count` only if `complete`.
    MissedEvents {
        count: u32,
        max_rms: f64,
        complete: bool,
        down_ms: u64,
    },
    /// Baby: the noise gate in effect changed through `set_noise_gate`.
    NoiseGateChanged {
        threshold: f64,
        hold_ms: u64,
    },
    /// The listen intent was reset (outage of 60 s or more).
    ListenChanged {
        enabled: bool,
    },
    /// Diagnostic reason of a failed or ended attempt; not a UI key.
    Failed {
        reason: String,
    },
}

#[uniffi::remote(Record)]
pub struct SessionConfig {
    pub role: Role,
    pub room_id: String,
    /// QR pairing PSK (base64url). `None` disables `pair_proof` (dev only).
    pub psk_b64url: Option<String>,
    pub heartbeat_interval_ms: u64,
    /// The session ends (parent: with `LinkLost`) when the verified peer sent no DataChannel
    /// message for this long.
    pub heartbeat_stale_ms: u64,
    pub pairing_timeout_ms: u64,
    pub noise_threshold: f64,
    pub noise_hold_ms: u64,
}

#[uniffi::remote(Enum)]
pub enum Input {
    SignalingOpen,
    SignalingMessage {
        text: String,
    },
    SignalingClosed,
    /// After the platform applied its own offer/answer via `setLocalDescription`.
    LocalDescription {
        sdp_type: String,
        sdp: String,
    },
    LocalIceCandidate {
        candidate: IceCandidate,
    },
    DataChannelOpen,
    DataChannelMessage {
        text: String,
    },
    DataChannelClosed,
    /// `RTCPeerConnectionState` as lowercase string (`connected`, `failed`, …).
    PeerConnectionState {
        state: String,
    },
    /// Baby: microphone RMS 0..1, sampled ~10 Hz.
    AudioLevel {
        rms: f64,
    },
    /// Parent: ask the baby unit to stream audio continuously.
    SetListen {
        enabled: bool,
    },
    /// Parent: ask the baby unit for one synthetic alert.
    RequestProbe,
    /// Timer tick; platforms call this at least every 500 ms.
    Tick,
    Close,
}

#[uniffi::remote(Enum)]
pub enum Action {
    SendSignaling { text: String },
    CreateOffer,
    SetRemoteDescription { sdp_type: String, sdp: String },
    CreateAnswer,
    AddIceCandidate { candidate: IceCandidate },
    SendData { text: String },
    SetMicEnabled { enabled: bool },
    Emit { event: SessionEvent },
    CloseTransport,
}

#[uniffi::remote(Enum)]
pub enum SessionState {
    Connecting,
    Joined,
    Connected,
    Verified,
    Failed,
    Closed,
}

#[uniffi::remote(Enum)]
pub enum SessionEvent {
    StateChanged {
        state: SessionState,
    },
    PairingVerified,
    Heartbeat {
        seq: u64,
        rms: Option<f64>,
        battery: Option<f64>,
    },
    NoiseAlert {
        rms: f64,
        probe: bool,
    },
    /// Parent: the baby unit is unreachable — this must reach the user as an alarm.
    LinkLost {
        reason: String,
    },
    Failed {
        reason: String,
    },
}
