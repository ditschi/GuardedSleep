//! UniFFI declarations for the core types that cross the Kotlin/Swift boundary. The core carries no
//! binding attributes (ADR-020), so each type is declared again here with `#[uniffi::remote]`. The
//! field and variant lists must match the core definitions exactly; a mismatch fails to compile.
//! Docs: <https://mozilla.github.io/uniffi-rs/latest/proc_macro/index.html#the-uniffiremote-attribute>

use guardedsleep_core::invite::InviteMailbox;
use guardedsleep_core::link::{
    LinkAction, LinkConfig, LinkEvent, LinkInput, LinkState, LinkStatus,
};
use guardedsleep_core::mailbox::MailboxKind;
use guardedsleep_core::pairing::ParentSecret;
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
    /// 32 fresh random bytes for the in-core CSPRNG, drawn anew for every run (Android
    /// `SecureRandom`, web `crypto.getRandomValues`). Secret-class: never logged. `Link::new`
    /// fails without exactly 32 bytes, takes the bytes and wipes its copy. JSON form: 64 hex
    /// characters.
    pub seed: Vec<u8>,
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
    /// The parent secret persisted from `LinkEvent::ParentPinned` (baby: the pinned parent; parent:
    /// the secret drawn for this pairing, acknowledged or not). `None` before the first pairing.
    /// Secret-class: never logged; `Link::new` takes the bytes and wipes its copy.
    pub pinned_parent: Option<ParentSecret>,
    /// The first-pairing window as a Unix second (the invite expiry): the platform persists
    /// `ParentPinned::window_until_s` and passes it back here. Parent: the raw secret is sent only
    /// before it. Baby: `parent_pin` is accepted only before it (a baby that was never paired and
    /// has none accepts a first one). `None` after the acknowledgement or outside a first pairing.
    pub pin_window_until_s: Option<u64>,
    /// Parent: this device's name, sent to the baby over the verified DataChannel only (at most 64
    /// characters). Empty: no name.
    pub device_name: String,
    /// The pairing's mailbox set. Baby: also sent to the parent after each verification. Empty:
    /// none is sent.
    pub mailboxes: Vec<InviteMailbox>,
    /// Development builds only: accept loopback `l:` entries in a received mailbox update.
    pub allow_dev_loopback: bool,
    /// "Only same Wi-Fi or my own server". Reserved for the project-server entries; the core does
    /// not read it at present.
    pub only_local: bool,
}

/// The parent secret (32 bytes) as it crosses the boundary: in `LinkConfig::pinned_parent` and in
/// the one event that carries it. Never log it. The core derives one key per purpose from it with
/// HKDF-SHA256 and never uses the raw bytes as a key.
#[uniffi::remote(Record)]
pub struct ParentSecret {
    pub bytes: Vec<u8>,
}

/// One mailbox of an invite or of a mailbox update: its kind and the URL to open
/// (`wss://host[:port][/path]` for a relay, `ws://ip:port` for a LAN mailbox).
#[uniffi::remote(Record)]
pub struct InviteMailbox {
    pub kind: MailboxKind,
    pub url: String,
}

#[uniffi::remote(Enum)]
pub enum MailboxKind {
    /// Our signaling relay (`signaling-relay`).
    Relay,
    /// A LAN mailbox (`l:` in an invite).
    Lan,
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
    /// The platform's wall clock in Unix seconds: at start, with ticks and after a time change.
    /// Read for the `here` message and the `clock_skew` comparison with the peer only; never used
    /// for liveness or escalation, which run on the monotonic clock.
    WallClock {
        unix_s: u64,
    },
    Stop,
    /// Parent: "I'm checking" - mute levels 2-3 for 5 minutes.
    AcknowledgeAlarm,
    /// The platform's network changed or came back: retry immediately, reset the backoff.
    NetworkAvailable,
    /// The socket of mailbox `id` opened (the answer to `open_mailbox`).
    MailboxOpened {
        id: u32,
    },
    /// The socket of mailbox `id` closed or failed to open (`reason` is diagnostic, not a UI key).
    /// Also the answer to a refused `open_mailbox`.
    MailboxClosed {
        id: u32,
        reason: String,
    },
    /// One inbound frame of mailbox `id`. `bytes` is the WebSocket text frame (UTF-8): every
    /// mailbox kind speaks JSON text frames.
    MailboxFrame {
        id: u32,
        bytes: String,
    },
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
    /// Open a socket for mailbox `id`. `url` empty: the platform's configured address for the kind.
    /// After `close_mailbox` the platform discards further events of that socket; a failed open is
    /// reported as `mailbox_closed`.
    OpenMailbox {
        id: u32,
        kind: MailboxKind,
        url: String,
    },
    /// Send one WebSocket text frame on mailbox `id`.
    MailboxSend {
        id: u32,
        frame: String,
    },
    CloseMailbox {
        id: u32,
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
    /// Persist this secret (and `window_until_s`) next to the PSK, durably, **before executing any
    /// later action of the same output**: the parent's core emits it before the action that sends
    /// it, and again with every resend while the baby has not acknowledged it (`window_until_s` is
    /// then the first-pairing window). It is idempotent. The baby emits it once when it pins; the
    /// acknowledgement on the parent emits it with `window_until_s: None`. A baby counts the
    /// pairing as complete at this event, not when it connects. Pass both back as
    /// `LinkConfig::pinned_parent` / `pin_window_until_s`. The one event that carries a secret:
    /// never log it.
    ParentPinned {
        secret: ParentSecret,
        window_until_s: Option<u64>,
    },
    /// Baby: the parent device that is now trusted for this attempt, by the name it sent over the
    /// verified DataChannel. Empty if it sent none.
    ParentConnected {
        name: String,
    },
    /// Parent: the baby sent its current mailbox set; replace the stored one.
    MailboxesUpdated {
        mailboxes: Vec<InviteMailbox>,
    },
    /// A calm hint appeared or went away: `clock_skew` or `recovery_path_unavailable`.
    Hint {
        key: String,
        active: bool,
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
