//! `Link`: one monitoring relationship from `start` to `stop` (ADR-017 §2-§11). It owns the
//! signaling-transport state, the (at most one) live [`Attempt`], liveness, escalation, listen
//! intent, snooze, peer status and the baby's missed-event buffer. Platforms talk only to the
//! `Link`: they feed [`LinkInput`]s with a **monotonic** millisecond clock (it must keep running
//! in deep sleep; `Date.now()` clamped to never go backwards on the web) and execute the returned
//! [`LinkAction`]s. Call `tick` at least every 500 ms while monitoring; Doze gaps are time jumps.
//!
//! Platform contract worth stating plainly:
//! - After `close_signaling` the platform must discard further events of that socket, and a
//!   failed `open_signaling` is reported as `signaling_closed`.
//! - After `close_attempt` the platform may still deliver late callbacks for that handle; the
//!   core drops them.
//! - `close_attempt` is always emitted before the `create_attempt` that replaces it.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::attempt::{Attempt, AttemptConfig, AttemptEvent, DEFAULT_ATTEMPT_TIMEOUT_MS};
use crate::noise::{GateChange, NoiseGate};
use crate::pairing::{decode_psk, derive_room_id, link_epoch, macs_equal, stop_notice_mac};
use crate::protocol::{
    encode_data_message, encode_hello, encode_ping, encode_signal, parse_server_message,
    parse_signal_payload, signal_payload_json, DataMessage, DeviceStatus, IceCandidate, Member,
    Role, ServerMessage, SignalPayload,
};
use crate::session::ConfigError;

/// Retry delays before the next attempt, indexed by consecutive failures (ADR-017 §6).
const BACKOFF_MS: [u64; 5] = [0, 1_000, 2_000, 4_000, 8_000];
const CAP_MS: u64 = 15_000;
const LONG_OUTAGE_MS: u64 = 600_000;
const LONG_CAP_MS: u64 = 30_000;
const MISMATCH_HOLD_MS: u64 = 15_000;
const SIGNALING_PING_MS: u64 = 15_000;
const SIGNALING_DEAD_MS: u64 = 35_000;
const SIGNALING_OPEN_TIMEOUT_MS: u64 = 15_000;
const LEVEL1_MS: u64 = 20_000;
const LEVEL3_MS: u64 = 180_000;
const ACK_MS: u64 = 300_000;
const LISTEN_RESUME_MS: u64 = 60_000;
const QUIET_RATE_MS: u64 = 10_000;
const MIC_SILENT_MS: u64 = 30_000;
const PROBE_TONE_MS: u64 = 2_000;
const MEDIA_BLOCKED_AFTER: u32 = 3;
const FIRST_PAIRING_MISMATCHES: u32 = 2;
const ALERT_LOG_CAP: usize = 128;
/// Accepted signaling stop nonces remembered per signaling connection (replay guard).
const STOP_SEEN_CAP: usize = 8;
/// The PSK is 32 random bytes (QR pairing, ADR-004).
const PSK_LEN: usize = 32;
/// Longest gap between `audio_level` samples that still counts as "the baby was listening".
const MAX_AUDIO_GAP_MS: u64 = 2_000;
pub const MIN_NOISE_THRESHOLD: f64 = 0.01;
pub const MAX_NOISE_THRESHOLD: f64 = 1.0;
pub const MIN_NOISE_HOLD_MS: u64 = 1_000;
pub const MAX_NOISE_HOLD_MS: u64 = 15_000;
const LOW_BATTERY_PCT: u8 = 15;
const CRITICAL_BATTERY_PCT: u8 = 5;
pub const MIN_HEARTBEAT_MS: u64 = 5_000;
pub const MAX_HEARTBEAT_MS: u64 = 15_000;

/// Every UI key the core can put into a [`LinkStatus`] (`key`, `cause`) or a `security_warning`
/// event. The i18n check verifies that each one exists in `locales/en.json`.
pub fn message_keys() -> &'static [&'static str] {
    &[
        // failed_needs_user keys
        "pairing_key_mismatch",
        "update_required",
        // cause hints from escalation level 2
        "server_unreachable",
        "peer_absent",
        "media_blocked",
        // security_warning key
        "wrong_key_attempt",
    ]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct LinkConfig {
    pub role: Role,
    /// Pairing PSK (base64url). The room id is derived from it.
    pub psk_b64url: String,
    /// The PSK was verified successfully in an earlier run, so a mismatch is never terminal.
    #[serde(default)]
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
    #[serde(default = "default_true")]
    pub require_dtls_fingerprint: bool,
}

fn default_true() -> bool {
    true
}

impl LinkConfig {
    pub fn with_defaults(role: Role, psk_b64url: String) -> Self {
        Self {
            role,
            psk_b64url,
            paired_before: false,
            heartbeat_interval_ms: 10_000,
            alarm_after_ms: 60_000,
            noise_threshold: 0.12,
            noise_hold_ms: 800,
            require_dtls_fingerprint: true,
        }
    }

    /// `2 * interval + 5 s` (ADR-017 §5).
    pub fn liveness_ms(&self) -> u64 {
        2 * self.heartbeat_interval_ms + 5_000
    }

    fn validate(&self) -> Result<Vec<u8>, ConfigError> {
        let psk = decode_psk(&self.psk_b64url)
            .filter(|k| k.len() == PSK_LEN)
            .ok_or(ConfigError::InvalidPsk)?;
        let interval_ok =
            (MIN_HEARTBEAT_MS..=MAX_HEARTBEAT_MS).contains(&self.heartbeat_interval_ms);
        // Loss must be detected at least 20 s before the alarm level, and the alarm must precede
        // the audible level 3.
        let alarm_ok = interval_ok
            && self.alarm_after_ms >= self.liveness_ms() + 20_000
            && self.alarm_after_ms < LEVEL3_MS;
        if !alarm_ok {
            return Err(ConfigError::InvalidTiming);
        }
        Ok(psk)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
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

#[derive(Debug, Clone, PartialEq)]
enum Mode {
    Idle,
    Running,
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sig {
    Closed,
    Opening,
    /// Socket open, `hello` sent, no `welcome` yet.
    Open,
    Joined,
}

struct Live {
    attempt: Attempt,
    peer_id: String,
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

pub struct Link {
    cfg: LinkConfig,
    psk: Vec<u8>,
    room: String,
    liveness_ms: u64,
    mode: Mode,
    has_started: bool,
    now: u64,
    rng: SplitMix64,
    started_at: u64,

    sig: Sig,
    sig_changed_at: u64,
    sig_failures: u32,
    sig_next_open_at: u64,
    sig_last_rx: u64,
    sig_last_ping: u64,
    client_id: Option<String>,
    members: Vec<Member>,
    peer_present: bool,

    live: Option<Live>,
    next_handle: u32,
    next_try_at: u64,
    fail_count: u32,
    hold_until: u64,
    /// Parent: deadline and own `aid` of the outstanding `request_offer`.
    outstanding: Option<(u64, String)>,
    /// Baby: the latest valid `request_offer` (sender client id, parent's `aid`).
    pending_request: Option<(String, String)>,

    ever_verified: bool,
    verified_in_run: bool,
    /// `link_epoch` of the most recent verified attempt in this run; a signaling stop is only
    /// valid for this epoch, so one recorded earlier can never match a later outage.
    epoch: Option<String>,
    consecutive_mismatches: u32,
    media_failures: u32,
    last_success_at: u64,
    last_alive: Option<u64>,
    outage_start: Option<u64>,
    max_level: u8,
    link_lost_emitted: bool,
    peer_stopped: bool,
    /// After a stop notice, wait until the peer left the room before offering again.
    stop_pending_absence: bool,
    ack_until: Option<u64>,
    last_status: Option<LinkStatus>,

    listen_intent: bool,
    snooze_until: Option<u64>,
    peer_status: Option<DeviceStatus>,
    low_fired: bool,
    critical_fired: bool,

    gate: NoiseGate,
    parent_listen: bool,
    noise_streaming: bool,
    probe_until: Option<u64>,
    mic_enabled: bool,
    /// The relay rejected our protocol version while a verified attempt kept running.
    update_required: bool,
    /// Nonces of accepted signaling stops on this signaling connection (replay guard).
    stop_seen: VecDeque<String>,
    alert_log: VecDeque<(u64, f64)>,
    alert_log_dropped_t: Option<u64>,
    audio_seen: bool,
    last_audio_at: Option<u64>,
    /// Longest gap between audio samples inside the current outage.
    outage_max_audio_gap: u64,
    zero_since: Option<u64>,
    mic_silent: bool,
    platform_status: DeviceStatus,
    last_sent_status: Option<DeviceStatus>,
    last_quiet_at: Option<u64>,
}

fn emit(out: &mut Vec<LinkAction>, event: LinkEvent) {
    out.push(LinkAction::Emit { event });
}

fn round4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

impl Link {
    pub fn new(cfg: LinkConfig) -> Result<Self, ConfigError> {
        let psk = cfg.validate()?;
        let room = derive_room_id(&psk);
        let gate = NoiseGate::new(cfg.noise_threshold, cfg.noise_hold_ms);
        Ok(Self {
            liveness_ms: cfg.liveness_ms(),
            ever_verified: cfg.paired_before,
            epoch: None,
            cfg,
            psk,
            room,
            mode: Mode::Idle,
            has_started: false,
            now: 0,
            rng: SplitMix64(0),
            started_at: 0,
            sig: Sig::Closed,
            sig_changed_at: 0,
            sig_failures: 0,
            sig_next_open_at: 0,
            sig_last_rx: 0,
            sig_last_ping: 0,
            client_id: None,
            members: Vec::new(),
            peer_present: false,
            live: None,
            next_handle: 1,
            next_try_at: 0,
            fail_count: 0,
            hold_until: 0,
            outstanding: None,
            pending_request: None,
            verified_in_run: false,
            consecutive_mismatches: 0,
            media_failures: 0,
            last_success_at: 0,
            last_alive: None,
            outage_start: None,
            max_level: 0,
            link_lost_emitted: false,
            peer_stopped: false,
            stop_pending_absence: false,
            ack_until: None,
            last_status: None,
            listen_intent: false,
            snooze_until: None,
            peer_status: None,
            low_fired: false,
            critical_fired: false,
            gate,
            parent_listen: false,
            noise_streaming: false,
            probe_until: None,
            mic_enabled: false,
            update_required: false,
            stop_seen: VecDeque::new(),
            alert_log: VecDeque::new(),
            alert_log_dropped_t: None,
            audio_seen: false,
            last_audio_at: None,
            outage_max_audio_gap: 0,
            zero_since: None,
            mic_silent: false,
            platform_status: DeviceStatus::default(),
            last_sent_status: None,
            last_quiet_at: None,
        })
    }

    /// The PSK-derived room id the link joins (22 characters).
    pub fn room_id(&self) -> &str {
        &self.room
    }

    pub fn status(&self) -> Option<&LinkStatus> {
        self.last_status.as_ref()
    }

    /// Handle of the live attempt, if any (for tests and diagnostics).
    pub fn live_attempt(&self) -> Option<u32> {
        self.live.as_ref().map(|l| l.attempt.handle())
    }

    /// Link epoch of the most recent verified attempt in this run (for tests and diagnostics).
    pub fn epoch(&self) -> Option<&str> {
        self.epoch.as_deref()
    }

    pub fn handle(&mut self, now_ms: u64, input: LinkInput) -> Vec<LinkAction> {
        let now = now_ms.max(self.now);
        self.now = now;
        let mut out = Vec::new();
        match (&self.mode, &input) {
            (Mode::Idle, LinkInput::Start { .. }) => {}
            (Mode::Idle, _) => return out,
            (Mode::Failed(_), LinkInput::Stop) => {}
            (Mode::Failed(_), _) => return out,
            (Mode::Running, LinkInput::Start { .. }) => return out,
            (Mode::Running, _) => {}
        }
        match input {
            LinkInput::Start { entropy } => self.on_start(now, entropy, &mut out),
            LinkInput::Stop => self.on_stop(&mut out),
            LinkInput::AcknowledgeAlarm => self.on_acknowledge(now, &mut out),
            LinkInput::NetworkAvailable => self.on_network_available(now, &mut out),
            LinkInput::SignalingOpened => self.on_signaling_opened(now, &mut out),
            LinkInput::SignalingMessage { text } => self.on_signaling_text(now, &text, &mut out),
            LinkInput::SignalingClosed => {
                if self.sig != Sig::Closed {
                    self.signaling_down(now, false, &mut out);
                }
            }
            LinkInput::AttemptLocalDescription {
                attempt,
                sdp_type,
                sdp,
            } => {
                if let Some(l) = self.live_mut(attempt) {
                    let ev = l.attempt.on_local_description(now, &sdp_type, &sdp);
                    self.process(now, ev, &mut out);
                }
            }
            LinkInput::AttemptLocalIce { attempt, candidate } => {
                if let Some(l) = self.live_mut(attempt) {
                    let ev = l.attempt.on_local_ice(candidate);
                    self.process(now, ev, &mut out);
                }
            }
            LinkInput::AttemptDataOpen { attempt } => {
                if let Some(l) = self.live_mut(attempt) {
                    let ev = l.attempt.on_data_open(now);
                    self.process(now, ev, &mut out);
                }
            }
            LinkInput::AttemptDataMessage { attempt, text } => {
                if let Some(l) = self.live_mut(attempt) {
                    let ev = l.attempt.on_data_message(now, &text);
                    self.process(now, ev, &mut out);
                }
            }
            LinkInput::AttemptDataClosed { attempt } => {
                if let Some(l) = self.live_mut(attempt) {
                    let ev = l.attempt.on_data_closed();
                    self.process(now, ev, &mut out);
                }
            }
            LinkInput::AttemptDtlsFingerprint {
                attempt,
                fingerprint,
            } => {
                if let Some(l) = self.live_mut(attempt) {
                    let ev = l.attempt.on_dtls_fingerprint(now, &fingerprint);
                    self.process(now, ev, &mut out);
                }
            }
            LinkInput::AttemptPcState { attempt, state } => {
                if let Some(l) = self.live_mut(attempt) {
                    let ev = l.attempt.on_pc_state(&state);
                    self.process(now, ev, &mut out);
                }
            }
            LinkInput::AudioLevel { rms } => self.on_audio_level(now, rms, &mut out),
            LinkInput::DeviceStatus { status } | LinkInput::QuietApplied { state: status } => {
                if self.cfg.role == Role::Baby {
                    self.platform_status = status;
                    self.flush_status(&mut out);
                }
            }
            LinkInput::SetNoiseGate { threshold, hold_ms } => {
                self.on_set_noise_gate(threshold, hold_ms, &mut out)
            }
            LinkInput::SetListen { enabled } => {
                if self.cfg.role == Role::Parent {
                    self.listen_intent = enabled;
                    self.send_app(&DataMessage::Listen { enabled }, &mut out);
                }
            }
            LinkInput::RequestProbe => {
                if self.cfg.role == Role::Parent {
                    self.send_app(&DataMessage::ProbeTest { t: now as f64 }, &mut out);
                }
            }
            LinkInput::SetSnooze { until_ms } => {
                if self.cfg.role == Role::Parent {
                    self.snooze_until = until_ms;
                }
            }
            LinkInput::RequestQuiet => {
                if self.cfg.role == Role::Parent {
                    self.send_app(&DataMessage::QuietRequest, &mut out);
                }
            }
            LinkInput::Tick => {}
        }
        self.pump(now, &mut out);
        out
    }

    fn live_mut(&mut self, handle: u32) -> Option<&mut Live> {
        self.live
            .as_mut()
            .filter(|l| l.attempt.handle() == handle && !l.attempt.is_finished())
    }

    fn live_verified(&self) -> bool {
        self.live
            .as_ref()
            .is_some_and(|l| l.attempt.is_verified() && !l.attempt.is_finished())
    }

    fn other_role(&self) -> Role {
        match self.cfg.role {
            Role::Baby => Role::Parent,
            Role::Parent => Role::Baby,
        }
    }

    fn peer_member(&self) -> Option<&Member> {
        let other = self.other_role();
        self.members.iter().find(|m| m.role == other)
    }

    fn fresh_aid(&mut self) -> String {
        format!("{:016x}{:016x}", self.rng.next(), self.rng.next())
    }

    // ---- lifecycle -------------------------------------------------------------------------

    fn on_start(&mut self, now: u64, entropy: u64, out: &mut Vec<LinkAction>) {
        let mut cfg = self.cfg.clone();
        cfg.paired_before = self.ever_verified;
        let Ok(mut fresh) = Link::new(cfg) else {
            return;
        };
        fresh.now = now;
        fresh.mode = Mode::Running;
        fresh.has_started = true;
        fresh.rng = SplitMix64(entropy);
        fresh.started_at = now;
        fresh.last_success_at = now;
        fresh.next_try_at = now;
        fresh.sig = Sig::Opening;
        fresh.sig_changed_at = now;
        fresh.next_handle = self.next_handle;
        fresh.last_status = self.last_status.take();
        *self = fresh;
        out.push(LinkAction::OpenSignaling);
    }

    fn on_stop(&mut self, out: &mut Vec<LinkAction>) {
        if self.mode == Mode::Running {
            self.announce_stop(out);
            self.close_live(out);
            if self.sig != Sig::Closed {
                out.push(LinkAction::CloseSignaling);
                self.sig = Sig::Closed;
            }
            if self.mic_enabled {
                self.mic_enabled = false;
                out.push(LinkAction::SetMicEnabled { enabled: false });
            }
        }
        self.mode = Mode::Idle;
        self.peer_stopped = false;
        self.stop_pending_absence = false;
        self.ack_until = None;
        self.outstanding = None;
        self.pending_request = None;
    }

    /// Tell the peer this is a deliberate stop, over the DataChannel and over signaling.
    fn announce_stop(&mut self, out: &mut Vec<LinkAction>) {
        if self.live_verified() {
            self.send_app(&DataMessage::StopNotice, out);
        }
        if let (Sig::Joined, Some(epoch)) = (self.sig, self.epoch.clone()) {
            let target = self
                .live
                .as_ref()
                .map(|l| l.peer_id.clone())
                .or_else(|| self.peer_member().map(|m| m.client_id.clone()));
            if let Some(to) = target {
                let aid = self.fresh_aid();
                let mac = stop_notice_mac(&self.psk, self.cfg.role, &self.room, &epoch, &to, &aid);
                out.push(LinkAction::SendSignaling {
                    text: encode_signal(
                        &self.room,
                        Some(&to),
                        signal_payload_json(&SignalPayload::StopNotice {
                            aid: Some(aid),
                            mac,
                        }),
                    ),
                });
            }
        }
    }

    fn terminal(&mut self, key: &str, out: &mut Vec<LinkAction>) {
        // A terminal state renders without alarm and zeroes the level, so a link that ever proved
        // the key must stay running; only the explicit user stop ends it.
        if self.ever_verified {
            return;
        }
        self.close_live(out);
        if self.sig != Sig::Closed {
            out.push(LinkAction::CloseSignaling);
            self.sig = Sig::Closed;
        }
        self.mode = Mode::Failed(key.to_owned());
    }

    fn on_network_available(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        // Platform-originated, so it may also lift the post-mismatch hold.
        self.hold_until = 0;
        if self.sig == Sig::Closed {
            self.sig_failures = 0;
            self.sig_next_open_at = now;
        }
        if self.live.as_ref().is_some_and(|l| !l.attempt.is_verified()) {
            self.close_live(out);
        }
        self.immediate_retry(now);
    }

    fn immediate_retry(&mut self, now: u64) {
        self.fail_count = 0;
        self.next_try_at = now.max(self.hold_until);
        self.outstanding = None;
    }

    fn on_acknowledge(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        if self.cfg.role != Role::Parent || self.ack_active(now) {
            return;
        }
        if self.current_level(now) >= 2 {
            let until = now + ACK_MS;
            self.ack_until = Some(until);
            emit(out, LinkEvent::AlarmAcknowledged { until_ms: until });
        }
    }

    fn ack_active(&self, now: u64) -> bool {
        self.ack_until.is_some_and(|u| now < u)
    }

    // ---- signaling transport ---------------------------------------------------------------

    fn on_signaling_opened(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        if self.sig != Sig::Opening {
            return;
        }
        self.sig = Sig::Open;
        self.sig_last_rx = now;
        self.sig_last_ping = now;
        out.push(LinkAction::SendSignaling {
            text: encode_hello(self.cfg.role, &self.room),
        });
        self.immediate_retry(now);
    }

    fn signaling_down(&mut self, now: u64, emit_close: bool, out: &mut Vec<LinkAction>) {
        if emit_close && self.sig != Sig::Closed {
            out.push(LinkAction::CloseSignaling);
        }
        self.sig = Sig::Closed;
        self.client_id = None;
        self.members.clear();
        self.peer_present = false;
        self.outstanding = None;
        self.pending_request = None;
        self.stop_seen.clear();
        self.sig_next_open_at = now + self.delay(now, self.sig_failures);
        self.sig_failures += 1;
    }

    fn delay(&self, now: u64, failures: u32) -> u64 {
        match BACKOFF_MS.get(failures as usize) {
            Some(d) => *d,
            None if now.saturating_sub(self.last_success_at) >= LONG_OUTAGE_MS => LONG_CAP_MS,
            None => CAP_MS,
        }
    }

    fn on_signaling_text(&mut self, now: u64, text: &str, out: &mut Vec<LinkAction>) {
        if !matches!(self.sig, Sig::Open | Sig::Joined) {
            return;
        }
        self.sig_last_rx = now;
        let Some(msg) = parse_server_message(text) else {
            return;
        };
        match msg {
            ServerMessage::Welcome { client_id, room_id } => {
                if self.sig == Sig::Open && room_id == self.room {
                    self.sig = Sig::Joined;
                    self.client_id = Some(client_id);
                    self.sig_failures = 0;
                    self.update_required = false;
                }
            }
            ServerMessage::Members { peers } => {
                if self.sig != Sig::Joined {
                    return;
                }
                let me = self.client_id.clone();
                self.members = peers
                    .into_iter()
                    .filter(|m| Some(&m.client_id) != me.as_ref())
                    .collect();
                let present = self.peer_member().is_some();
                if present && !self.peer_present {
                    // Reappearing is attacker-influenceable, so it never lifts the mismatch hold.
                    self.immediate_retry(now);
                }
                if !present {
                    self.outstanding = None;
                    self.stop_pending_absence = false;
                }
                self.peer_present = present;
            }
            ServerMessage::Signal { from, payload } => {
                if self.sig != Sig::Joined {
                    return;
                }
                if let Some(p) = parse_signal_payload(&payload) {
                    self.on_signal(now, &from, p, out);
                }
            }
            ServerMessage::Error { code, .. } => {
                if code == "UNSUPPORTED_PROTOCOL" {
                    self.on_unsupported_protocol(now, out);
                } else {
                    // Retryable at the signaling layer (BABY_TAKEN, PARENT_CAP, rate limits, ...).
                    self.signaling_down(now, true, out);
                }
            }
            ServerMessage::Peer { .. } | ServerMessage::Pong => {}
        }
    }

    /// Only the reply to our `hello` (before `welcome`) can mean "your version is not supported";
    /// the same error later is a forgery or noise and must not touch a running link. A verified
    /// attempt keeps running (the media path does not need the relay), so there the error only
    /// closes signaling and leaves a sticky `update_required` hint. The same holds for any link
    /// that verified in this run or before (`paired_before`): a malicious relay must not turn an
    /// outage into a silent terminal state, so escalation continues from `last_alive` and
    /// signaling is retried on the long backoff. Only a link that never verified is terminal.
    fn on_unsupported_protocol(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        if self.sig != Sig::Open {
            return;
        }
        if self.ever_verified {
            self.update_required = true;
            self.signaling_down(now, true, out);
            self.sig_next_open_at = now + LONG_CAP_MS;
        } else {
            self.terminal("update_required", out);
        }
    }

    fn on_signal(
        &mut self,
        now: u64,
        from: &str,
        payload: SignalPayload,
        out: &mut Vec<LinkAction>,
    ) {
        match payload.clone() {
            SignalPayload::StopNotice { aid, mac } => self.on_signal_stop(aid, mac, out),
            SignalPayload::RequestOffer { aid } => {
                if self.cfg.role == Role::Baby {
                    self.on_request_offer(now, from, aid, out);
                }
            }
            SignalPayload::Sdp { .. } => self.on_remote_sdp(now, from, &payload, out),
            SignalPayload::Ice { aid, candidate } => {
                if let Some(l) = self.live.as_ref() {
                    // A verified attempt is done negotiating; late candidates are only noise.
                    if l.peer_id == from && l.attempt.accepts_ice(&aid) {
                        out.push(LinkAction::AddIceCandidate {
                            attempt: l.attempt.handle(),
                            candidate,
                        });
                    }
                }
            }
        }
    }

    /// `from` on the signaling path is relay-asserted, so a stop counts only with a valid PSK MAC
    /// bound to our last-verified link epoch (a relay can recycle client ids but not epochs, so a
    /// recorded stop never matches a later outage), our current client id and a nonce not seen
    /// before. Before any verification in this run it is ignored. A verified peer announces its
    /// stop over the DataChannel instead.
    fn on_signal_stop(
        &mut self,
        aid: Option<String>,
        mac: Option<String>,
        out: &mut Vec<LinkAction>,
    ) {
        let (Some(aid), Some(mac), Some(me)) = (aid, mac, self.client_id.as_deref()) else {
            return;
        };
        let Some(epoch) = self.epoch.as_deref() else {
            return;
        };
        let Some(expected) =
            stop_notice_mac(&self.psk, self.other_role(), &self.room, epoch, me, &aid)
        else {
            return;
        };
        if !macs_equal(&expected, &mac) || self.stop_seen.contains(&aid) {
            return;
        }
        if self.stop_seen.len() >= STOP_SEEN_CAP {
            self.stop_seen.pop_front();
        }
        self.stop_seen.push_back(aid);
        if self.live_verified() {
            return;
        }
        self.close_live(out);
        self.set_peer_stopped();
    }

    fn set_peer_stopped(&mut self) {
        // The event is emitted by `refresh_status` on the transition into `peer_stopped`.
        self.peer_stopped = true;
        self.stop_pending_absence = true;
    }

    // ---- attempt creation and routing ------------------------------------------------------

    fn create_attempt(
        &mut self,
        now: u64,
        peer_id: String,
        own_aid: String,
        remote_aid: Option<String>,
        offerer: bool,
        out: &mut Vec<LinkAction>,
    ) {
        // Close-before-create; there is never more than one live attempt.
        self.close_live(out);
        let handle = self.next_handle;
        self.next_handle = self.next_handle.wrapping_add(1).max(1);
        let attempt = Attempt::new(
            AttemptConfig {
                role: self.cfg.role,
                psk: Some(self.psk.clone()),
                room: self.room.clone(),
                handle,
                own_aid,
                reply_to: if offerer { remote_aid.clone() } else { None },
                remote_aid,
                heartbeat_interval_ms: self.cfg.heartbeat_interval_ms,
                liveness_ms: self.liveness_ms,
                attempt_timeout_ms: DEFAULT_ATTEMPT_TIMEOUT_MS,
                require_sendonly: self.cfg.role == Role::Baby,
                require_dtls_fingerprint: self.cfg.require_dtls_fingerprint,
            },
            now,
        );
        out.push(LinkAction::CreateAttempt {
            attempt: handle,
            offerer,
        });
        if offerer {
            out.push(LinkAction::CreateOffer { attempt: handle });
        }
        self.live = Some(Live { attempt, peer_id });
    }

    fn close_live(&mut self, out: &mut Vec<LinkAction>) {
        if let Some(l) = self.live.take() {
            out.push(LinkAction::CloseAttempt {
                attempt: l.attempt.handle(),
            });
        }
    }

    fn on_request_offer(&mut self, now: u64, from: &str, aid: String, out: &mut Vec<LinkAction>) {
        let from_parent = self
            .members
            .iter()
            .any(|m| m.client_id == from && m.role == Role::Parent);
        if !from_parent || self.live_verified() {
            return;
        }
        // Duplicate of the request the live (unverified) attempt already answers.
        if self
            .live
            .as_ref()
            .is_some_and(|l| l.peer_id == from && l.attempt.remote_aid() == Some(aid.as_str()))
        {
            return;
        }
        if self
            .pending_request
            .as_ref()
            .is_some_and(|(_, a)| *a == aid)
        {
            return;
        }
        self.pending_request = Some((from.to_owned(), aid));
        // An unverified attempt may be replaced; a request inside the backoff window waits for it.
        if now >= self.next_try_at {
            self.start_baby_attempt(now, out);
        }
    }

    fn start_baby_attempt(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        let (to, remote_aid) = match self.pending_request.take() {
            Some((from, aid))
                if self
                    .members
                    .iter()
                    .any(|m| m.client_id == from && m.role == Role::Parent) =>
            {
                (from, Some(aid))
            }
            _ => match self.peer_member() {
                Some(m) => (m.client_id.clone(), None),
                None => return,
            },
        };
        let own = self.fresh_aid();
        self.create_attempt(now, to, own, remote_aid, true, out);
    }

    fn on_remote_sdp(
        &mut self,
        now: u64,
        from: &str,
        payload: &SignalPayload,
        out: &mut Vec<LinkAction>,
    ) {
        let SignalPayload::Sdp {
            aid,
            sdp_type,
            sdp,
            re,
        } = payload
        else {
            return;
        };
        let (aid, sdp_type, sdp, re) =
            (aid.as_str(), sdp_type.as_str(), sdp.as_str(), re.as_deref());
        if self.cfg.role == Role::Parent && sdp_type == "offer" {
            let from_baby = self.peer_member().is_some_and(|m| m.client_id == from);
            if !from_baby || self.live_verified() {
                return;
            }
            if let Some(l) = self.live.as_ref() {
                if l.peer_id == from && l.attempt.remote_aid() == Some(aid) {
                    return;
                }
            }
            if self.outstanding.as_ref().is_some_and(|(_, own)| own == aid) {
                return;
            }
            // Adopt the request id as own attempt id only for the offer that answers it.
            let answers_request =
                matches!((&self.outstanding, re), (Some((_, r)), Some(re)) if r == re);
            let own = match self.outstanding.take() {
                Some((_, requested)) if answers_request => requested,
                other => {
                    // An unsolicited offer keeps the request alive for its real answer.
                    self.outstanding = other;
                    self.fresh_aid()
                }
            };
            self.create_attempt(now, from.to_owned(), own, None, false, out);
        } else if !matches!((self.cfg.role, sdp_type), (Role::Baby, "answer")) {
            return;
        }
        let Some(l) = self.live.as_mut() else {
            return;
        };
        if l.peer_id != from {
            return;
        }
        let handle = l.attempt.handle();
        match l.attempt.on_remote_description(now, aid, sdp_type, sdp) {
            Some(ev) => {
                out.push(LinkAction::SetRemoteDescription {
                    attempt: handle,
                    sdp_type: sdp_type.to_owned(),
                    sdp: sdp.to_owned(),
                });
                if self.cfg.role == Role::Parent {
                    out.push(LinkAction::CreateAnswer { attempt: handle });
                }
                self.process(now, ev, out);
            }
            None => {
                // A parent attempt that cannot take its offer is useless; a baby keeps waiting.
                if self.cfg.role == Role::Parent {
                    self.close_live(out);
                }
            }
        }
    }

    // ---- attempt events --------------------------------------------------------------------

    fn process(&mut self, now: u64, events: Vec<AttemptEvent>, out: &mut Vec<LinkAction>) {
        for ev in events {
            if self.mode != Mode::Running {
                return;
            }
            let Some(live) = self.live.as_ref() else {
                return;
            };
            let handle = live.attempt.handle();
            match ev {
                AttemptEvent::Signal(payload) => {
                    if self.sig == Sig::Joined {
                        out.push(LinkAction::SendSignaling {
                            text: encode_signal(
                                &self.room,
                                Some(&live.peer_id),
                                signal_payload_json(&payload),
                            ),
                        });
                    }
                }
                AttemptEvent::SendData(text) => out.push(LinkAction::SendData {
                    attempt: handle,
                    text,
                }),
                AttemptEvent::Verified => self.on_verified(now, out),
                AttemptEvent::Message(msg) => self.on_peer_message(now, msg, out),
                AttemptEvent::Mismatch => self.on_mismatch(now, "pair_proof_mismatch", out),
                AttemptEvent::DtlsMismatch => {
                    self.on_mismatch(now, "dtls_fingerprint_mismatch", out)
                }
                AttemptEvent::LivenessLost => self.on_attempt_ended(now, "liveness_lost", out),
                AttemptEvent::Failed(reason) => self.on_attempt_ended(now, &reason, out),
            }
        }
    }

    fn try_failed(&mut self, now: u64) {
        let d = self.delay(now, self.fail_count);
        self.fail_count += 1;
        self.next_try_at = (now + d).max(self.hold_until);
    }

    fn on_attempt_ended(&mut self, now: u64, reason: &str, out: &mut Vec<LinkAction>) {
        let Some(live) = self.live.as_ref() else {
            return;
        };
        let was_verified = live.attempt.is_verified();
        let exchanged = live.attempt.sdp_exchanged();
        let last_alive = live.attempt.last_alive();
        self.close_live(out);
        emit(
            out,
            LinkEvent::Failed {
                reason: reason.to_owned(),
            },
        );
        if was_verified {
            self.on_verified_attempt_lost(now, last_alive, out);
        } else {
            if exchanged {
                self.media_failures += 1;
            }
            self.try_failed(now);
        }
    }

    fn on_verified_attempt_lost(&mut self, now: u64, last_alive: u64, out: &mut Vec<LinkAction>) {
        self.last_alive = Some(last_alive);
        if self.outage_start.is_none() {
            self.outage_start = Some(last_alive);
            self.outage_max_audio_gap = 0;
        }
        self.parent_listen = false;
        self.apply_mic(now, out);
        self.last_sent_status = None;
        self.immediate_retry(now);
    }

    /// A peer that proved the wrong key, or whose DTLS certificate differs from its SDP.
    fn on_mismatch(&mut self, now: u64, reason: &str, out: &mut Vec<LinkAction>) {
        self.consecutive_mismatches += 1;
        self.close_live(out);
        emit(
            out,
            LinkEvent::Failed {
                reason: reason.into(),
            },
        );
        if !self.ever_verified {
            if self.consecutive_mismatches >= FIRST_PAIRING_MISMATCHES {
                self.terminal("pairing_key_mismatch", out);
            } else {
                self.try_failed(now);
            }
        } else {
            emit(
                out,
                LinkEvent::SecurityWarning {
                    key: "wrong_key_attempt".into(),
                },
            );
            let d = self.delay(now, self.fail_count).max(MISMATCH_HOLD_MS);
            self.fail_count += 1;
            self.hold_until = now + d;
            self.next_try_at = self.hold_until;
        }
    }

    fn on_verified(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        let first = !self.verified_in_run;
        if let Some(l) = self.live.as_ref() {
            if let Some(remote) = l.attempt.remote_aid() {
                self.epoch = Some(link_epoch(l.attempt.own_aid(), remote));
            }
        }
        self.ever_verified = true;
        self.verified_in_run = true;
        self.consecutive_mismatches = 0;
        self.fail_count = 0;
        self.hold_until = 0;
        self.media_failures = 0;
        self.last_success_at = now;
        let outage_start = self.outage_start.take();
        let down_ms = outage_start.map_or(0, |s| now.saturating_sub(s));
        if outage_start.is_some() && self.max_level >= 1 {
            emit(out, LinkEvent::LinkRestored { down_ms });
        }
        self.max_level = 0;
        self.link_lost_emitted = false;
        self.ack_until = None;
        self.peer_stopped = false;
        self.stop_pending_absence = false;
        self.last_alive = Some(now);
        match self.cfg.role {
            Role::Baby => {
                self.send_missed_events(first, outage_start, down_ms, out);
                self.last_sent_status = None;
                self.flush_status(out);
            }
            Role::Parent => {
                if self.listen_intent {
                    if first || outage_start.is_none() || down_ms < LISTEN_RESUME_MS {
                        self.send_app(&DataMessage::Listen { enabled: true }, out);
                    } else {
                        self.listen_intent = false;
                        emit(out, LinkEvent::ListenChanged { enabled: false });
                    }
                }
            }
        }
    }

    fn send_missed_events(
        &mut self,
        first: bool,
        outage_start: Option<u64>,
        down_ms: u64,
        out: &mut Vec<LinkAction>,
    ) {
        let start = outage_start.unwrap_or(self.started_at);
        let mut count = 0u32;
        let mut max_rms = 0.0f64;
        for (t, rms) in &self.alert_log {
            if *t >= start {
                count = count.saturating_add(1);
                max_rms = max_rms.max(*rms);
            }
        }
        let overflowed = self.alert_log_dropped_t.is_some_and(|t| t >= start);
        // A silent mic path (no samples while no attempt exists) must not read as "no sounds".
        let trailing_gap = self
            .now
            .saturating_sub(self.last_audio_at.unwrap_or(0).max(start));
        let listened = self.outage_max_audio_gap.max(trailing_gap) <= MAX_AUDIO_GAP_MS;
        self.outage_max_audio_gap = 0;
        let complete = !first && !overflowed && listened;
        self.alert_log.clear();
        self.alert_log_dropped_t = None;
        if !first || count > 0 {
            self.send_app(
                &DataMessage::MissedEvents {
                    count,
                    max_rms: round4(max_rms),
                    complete,
                    down_ms,
                },
                out,
            );
        }
    }

    fn on_peer_message(&mut self, now: u64, msg: DataMessage, out: &mut Vec<LinkAction>) {
        if msg == DataMessage::StopNotice {
            self.on_dc_stop(now, out);
            return;
        }
        match (self.cfg.role, msg) {
            (Role::Baby, DataMessage::Listen { enabled }) => {
                self.parent_listen = enabled;
                self.apply_mic(now, out);
            }
            (Role::Baby, DataMessage::ProbeTest { .. }) => {
                out.push(LinkAction::SendProbeTone {
                    duration_ms: PROBE_TONE_MS,
                });
                self.probe_until = Some(now + PROBE_TONE_MS);
                self.apply_mic(now, out);
                self.send_app(
                    &DataMessage::NoiseAlert {
                        rms: self.gate.last_rms(),
                        t: now as f64,
                        probe: true,
                    },
                    out,
                );
            }
            (Role::Baby, DataMessage::QuietRequest) => {
                if self
                    .last_quiet_at
                    .is_none_or(|t| now.saturating_sub(t) >= QUIET_RATE_MS)
                {
                    self.last_quiet_at = Some(now);
                    out.push(LinkAction::ApplyQuiet);
                }
            }
            (Role::Parent, DataMessage::NoiseAlert { rms, probe, .. }) => {
                let snoozed = !probe && self.snooze_until.is_some_and(|u| now < u);
                emit(
                    out,
                    LinkEvent::NoiseAlert {
                        rms,
                        probe,
                        snoozed,
                    },
                );
            }
            (Role::Parent, DataMessage::Heartbeat { seq, rms, .. }) => {
                emit(out, LinkEvent::PeerHeartbeat { seq, rms });
            }
            (Role::Parent, DataMessage::DeviceStatus(s)) => self.on_peer_status(s, out),
            (
                Role::Parent,
                DataMessage::MissedEvents {
                    count,
                    max_rms,
                    complete,
                    down_ms,
                },
            ) => emit(
                out,
                LinkEvent::MissedEvents {
                    count,
                    max_rms,
                    complete,
                    down_ms,
                },
            ),
            _ => {}
        }
    }

    /// Authenticated by the verified attempt, so the attempt can end right away.
    fn on_dc_stop(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        let Some(l) = self.live.as_ref() else {
            return;
        };
        let last_alive = l.attempt.last_alive();
        self.set_peer_stopped();
        self.close_live(out);
        self.on_verified_attempt_lost(now, last_alive, out);
    }

    fn on_peer_status(&mut self, s: DeviceStatus, out: &mut Vec<LinkAction>) {
        let prev = self.peer_status.replace(s.clone());
        if prev.as_ref() != Some(&s) {
            emit(out, LinkEvent::PeerStatus { status: s.clone() });
        }
        if prev.as_ref().and_then(|p| p.charging) == Some(true) && s.charging == Some(false) {
            emit(out, LinkEvent::ChargerUnplugged);
        }
        let Some(pct) = s.battery_pct else {
            return;
        };
        let charging = s.charging == Some(true);
        let critical = pct < CRITICAL_BATTERY_PCT;
        let low = pct < LOW_BATTERY_PCT && !charging && !critical;
        if critical && !self.critical_fired {
            self.critical_fired = true;
            self.low_fired = true;
            emit(
                out,
                LinkEvent::LowBattery {
                    pct,
                    critical: true,
                },
            );
        }
        if !critical {
            self.critical_fired = false;
        }
        if low && !self.low_fired {
            self.low_fired = true;
            emit(
                out,
                LinkEvent::LowBattery {
                    pct,
                    critical: false,
                },
            );
        }
        if pct >= LOW_BATTERY_PCT || charging {
            self.low_fired = false;
        }
    }

    // ---- baby: audio, mic, status ----------------------------------------------------------

    fn on_audio_level(&mut self, now: u64, rms: f64, out: &mut Vec<LinkAction>) {
        if self.cfg.role != Role::Baby {
            return;
        }
        let silent_sample = !rms.is_finite() || rms == 0.0;
        if let Some(start) = self.outage_start {
            let gap = now.saturating_sub(self.last_audio_at.unwrap_or(0).max(start));
            self.outage_max_audio_gap = self.outage_max_audio_gap.max(gap);
        }
        self.last_audio_at = Some(now);
        self.audio_seen = true;
        if silent_sample {
            self.zero_since.get_or_insert(now);
        } else {
            self.zero_since = None;
            if self.mic_silent {
                self.mic_silent = false;
                self.flush_status(out);
            }
        }
        match self.gate.sample(now, rms) {
            Some(GateChange::Loud { rms }) => {
                self.noise_streaming = true;
                self.apply_mic(now, out);
                self.log_alert(now, rms);
                self.send_app(
                    &DataMessage::NoiseAlert {
                        rms,
                        t: now as f64,
                        probe: false,
                    },
                    out,
                );
            }
            Some(GateChange::Quiet) => {
                self.noise_streaming = false;
                self.apply_mic(now, out);
            }
            None => {}
        }
    }

    fn on_set_noise_gate(&mut self, threshold: f64, hold_ms: u64, out: &mut Vec<LinkAction>) {
        let valid = (MIN_NOISE_THRESHOLD..=MAX_NOISE_THRESHOLD).contains(&threshold)
            && (MIN_NOISE_HOLD_MS..=MAX_NOISE_HOLD_MS).contains(&hold_ms);
        if self.cfg.role != Role::Baby || !valid {
            return;
        }
        if self.gate.threshold() == threshold && self.gate.hold_ms() == hold_ms {
            return;
        }
        self.gate.set_params(threshold, hold_ms);
        emit(out, LinkEvent::NoiseGateChanged { threshold, hold_ms });
        self.flush_status(out);
    }

    fn log_alert(&mut self, now: u64, rms: f64) {
        if self.alert_log.len() >= ALERT_LOG_CAP {
            if let Some((t, _)) = self.alert_log.pop_front() {
                self.alert_log_dropped_t = Some(self.alert_log_dropped_t.map_or(t, |d| d.max(t)));
            }
        }
        self.alert_log.push_back((now, rms));
    }

    fn apply_mic(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        let probing = self.probe_until.is_some_and(|u| now < u);
        let enabled = self.parent_listen || self.noise_streaming || probing;
        if enabled != self.mic_enabled {
            self.mic_enabled = enabled;
            out.push(LinkAction::SetMicEnabled { enabled });
        }
    }

    fn merged_status(&self) -> DeviceStatus {
        let mut s = self.platform_status.normalized();
        s.mic_silent = self.audio_seen.then_some(self.mic_silent);
        s.noise_threshold = Some(round4(self.gate.threshold()));
        s
    }

    fn flush_status(&mut self, out: &mut Vec<LinkAction>) {
        if self.cfg.role != Role::Baby || !self.live_verified() {
            return;
        }
        let s = self.merged_status();
        if self.last_sent_status.as_ref() == Some(&s) {
            return;
        }
        self.send_app(&DataMessage::DeviceStatus(s.clone()), out);
        self.last_sent_status = Some(s);
    }

    fn send_app(&self, msg: &DataMessage, out: &mut Vec<LinkAction>) {
        if let Some(l) = self.live.as_ref() {
            if l.attempt.encode_app_message(msg).is_some() {
                out.push(LinkAction::SendData {
                    attempt: l.attempt.handle(),
                    text: encode_data_message(msg),
                });
            }
        }
    }

    // ---- housekeeping ----------------------------------------------------------------------

    fn pump(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        if self.mode == Mode::Running {
            self.signaling_housekeeping(now, out);
            if self.cfg.role == Role::Baby {
                let rms = round4(self.gate.last_rms());
                if let Some(l) = self.live.as_mut() {
                    l.attempt.set_heartbeat_rms(Some(rms));
                }
            }
            if let Some(l) = self.live.as_mut() {
                let ev = l.attempt.tick(now);
                self.process(now, ev, out);
            }
        }
        if self.mode == Mode::Running {
            if self.outstanding.as_ref().is_some_and(|(d, _)| now >= *d) {
                self.outstanding = None;
                if self.live.is_none() {
                    self.try_failed(now);
                }
            }
            self.schedule(now, out);
            self.mic_housekeeping(now, out);
        }
        self.refresh_status(now, out);
    }

    fn signaling_housekeeping(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        match self.sig {
            Sig::Closed => {
                if now >= self.sig_next_open_at {
                    self.sig = Sig::Opening;
                    self.sig_changed_at = now;
                    out.push(LinkAction::OpenSignaling);
                }
            }
            Sig::Opening => {
                if now.saturating_sub(self.sig_changed_at) >= SIGNALING_OPEN_TIMEOUT_MS {
                    self.signaling_down(now, true, out);
                }
            }
            Sig::Open | Sig::Joined => {
                if now.saturating_sub(self.sig_last_rx) >= SIGNALING_DEAD_MS {
                    self.signaling_down(now, true, out);
                } else if now.saturating_sub(self.sig_last_ping) >= SIGNALING_PING_MS {
                    self.sig_last_ping = now;
                    out.push(LinkAction::SendSignaling {
                        text: encode_ping(),
                    });
                }
            }
        }
    }

    fn schedule(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        if self.live.is_some()
            || self.sig != Sig::Joined
            || !self.peer_present
            || (self.peer_stopped && self.stop_pending_absence)
            || now < self.next_try_at
        {
            return;
        }
        match self.cfg.role {
            Role::Baby => self.start_baby_attempt(now, out),
            Role::Parent => {
                if self.outstanding.is_some() {
                    return;
                }
                let Some(baby) = self.peer_member().map(|m| m.client_id.clone()) else {
                    return;
                };
                let aid = self.fresh_aid();
                out.push(LinkAction::SendSignaling {
                    text: encode_signal(
                        &self.room,
                        Some(&baby),
                        signal_payload_json(&SignalPayload::RequestOffer { aid: aid.clone() }),
                    ),
                });
                self.outstanding = Some((now + DEFAULT_ATTEMPT_TIMEOUT_MS, aid));
            }
        }
    }

    fn mic_housekeeping(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        if self.cfg.role != Role::Baby {
            return;
        }
        if self.probe_until.is_some_and(|u| now >= u) {
            self.probe_until = None;
        }
        self.apply_mic(now, out);
        if !self.mic_silent
            && self
                .zero_since
                .is_some_and(|z| now.saturating_sub(z) >= MIC_SILENT_MS)
        {
            self.mic_silent = true;
            self.flush_status(out);
        }
    }

    // ---- status and escalation -------------------------------------------------------------

    fn natural_level(&self, now: u64) -> u8 {
        let down = now.saturating_sub(self.last_alive.unwrap_or(self.started_at));
        if down >= LEVEL3_MS {
            3
        } else if down >= self.cfg.alarm_after_ms {
            2
        } else if down >= LEVEL1_MS {
            1
        } else {
            0
        }
    }

    /// Level while the link is down and running; 0 otherwise.
    fn current_level(&self, now: u64) -> u8 {
        if self.mode != Mode::Running || self.peer_stopped || self.live_verified() {
            return 0;
        }
        self.natural_level(now)
    }

    fn cause(&self) -> Option<String> {
        let c = if self.sig != Sig::Joined {
            "server_unreachable"
        } else if !self.peer_present {
            "peer_absent"
        } else if self.media_failures >= MEDIA_BLOCKED_AFTER {
            "media_blocked"
        } else {
            return None;
        };
        Some(c.to_owned())
    }

    fn refresh_status(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        let hint = self.update_required.then(|| "update_required".to_owned());
        let plain = |state, key: Option<String>, cause: Option<String>| LinkStatus {
            state,
            level: 0,
            key,
            cause,
            acknowledged_until: None,
            last_alive_ms: None,
        };
        let status = match self.mode.clone() {
            Mode::Idle => {
                if !self.has_started {
                    return;
                }
                plain(LinkState::Stopped, None, None)
            }
            Mode::Failed(key) => plain(LinkState::FailedNeedsUser, Some(key), None),
            Mode::Running => {
                if self.peer_stopped {
                    LinkStatus {
                        last_alive_ms: self.last_alive,
                        ..plain(LinkState::PeerStopped, None, hint)
                    }
                } else if self.live_verified() {
                    plain(LinkState::Connected, None, hint)
                } else {
                    if self.ack_until.is_some_and(|u| now >= u) {
                        self.ack_until = None;
                    }
                    let natural = self.natural_level(now);
                    self.max_level = self.max_level.max(natural);
                    if natural >= 2 && !self.link_lost_emitted {
                        self.link_lost_emitted = true;
                        let down = now.saturating_sub(self.last_alive.unwrap_or(self.started_at));
                        emit(out, LinkEvent::LinkLost { down_ms: down });
                    }
                    let level = if self.ack_until.is_some() {
                        natural.min(2)
                    } else {
                        natural
                    };
                    LinkStatus {
                        state: if self.verified_in_run {
                            LinkState::Reconnecting
                        } else {
                            LinkState::Connecting
                        },
                        level,
                        key: None,
                        cause: hint.or_else(|| if level >= 2 { self.cause() } else { None }),
                        acknowledged_until: self.ack_until,
                        last_alive_ms: self.last_alive,
                    }
                }
            }
        };
        if self.last_status.as_ref() != Some(&status) {
            let entered_stopped = status.state == LinkState::PeerStopped
                && self
                    .last_status
                    .as_ref()
                    .is_none_or(|s| s.state != LinkState::PeerStopped);
            self.last_status = Some(status.clone());
            if entered_stopped {
                emit(out, LinkEvent::PeerStopped);
            }
            emit(out, LinkEvent::Status { status });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pairing::encode_psk;
    use serde_json::json;

    fn cfg(role: Role) -> LinkConfig {
        LinkConfig::with_defaults(role, encode_psk(&[3u8; 32]))
    }

    fn started(role: Role) -> (Link, Vec<LinkAction>) {
        let mut l = Link::new(cfg(role)).unwrap();
        let a = l.handle(0, LinkInput::Start { entropy: 42 });
        (l, a)
    }

    #[test]
    fn config_validation_enforces_interval_and_liveness_invariants() {
        let ok = |interval: u64, alarm: u64| {
            let mut c = cfg(Role::Baby);
            c.heartbeat_interval_ms = interval;
            c.alarm_after_ms = alarm;
            Link::new(c).is_ok()
        };
        assert!(ok(10_000, 60_000));
        assert!(ok(5_000, 35_000), "liveness 15 s + 20 s");
        assert!(!ok(5_000, 34_999));
        assert!(!ok(4_999, 60_000));
        assert!(ok(15_000, 55_000), "liveness 35 s + 20 s");
        assert!(!ok(15_000, 54_999));
        assert!(!ok(15_001, 120_000));
        assert!(!ok(0, 60_000));
        assert!(!ok(10_000, 180_000), "alarm must precede level 3");
        assert!(ok(10_000, 179_999));
        let mut c = cfg(Role::Baby);
        c.psk_b64url = "%%%".into();
        assert_eq!(Link::new(c).err(), Some(ConfigError::InvalidPsk));
        let mut c = cfg(Role::Baby);
        c.psk_b64url = String::new();
        assert_eq!(Link::new(c).err(), Some(ConfigError::InvalidPsk));
        for len in [1usize, 16, 31, 33, 64] {
            let mut c = cfg(Role::Baby);
            c.psk_b64url = encode_psk(&vec![5u8; len]);
            assert_eq!(
                Link::new(c).err(),
                Some(ConfigError::InvalidPsk),
                "{len}-byte PSK"
            );
        }
    }

    #[test]
    fn dtls_fingerprint_is_required_by_default_and_old_configs_still_parse() {
        assert!(cfg(Role::Baby).require_dtls_fingerprint);
        let c: LinkConfig = serde_json::from_value(json!({
            "role": "baby",
            "psk_b64url": encode_psk(&[3u8; 32]),
            "heartbeat_interval_ms": 10000,
            "alarm_after_ms": 60000,
            "noise_threshold": 0.2,
            "noise_hold_ms": 300
        }))
        .unwrap();
        assert!(c.require_dtls_fingerprint);
    }

    #[test]
    fn liveness_is_twice_the_interval_plus_five_seconds() {
        let mut c = cfg(Role::Parent);
        assert_eq!(c.liveness_ms(), 25_000);
        c.heartbeat_interval_ms = 15_000;
        assert_eq!(c.liveness_ms(), 35_000);
        c.heartbeat_interval_ms = 5_000;
        assert_eq!(c.liveness_ms(), 15_000);
    }

    #[test]
    fn room_id_is_derived_from_the_psk() {
        let l = Link::new(cfg(Role::Baby)).unwrap();
        assert_eq!(l.room_id(), derive_room_id(&[3u8; 32]));
    }

    #[test]
    fn backoff_schedule_and_long_outage_cap() {
        let mut l = Link::new(cfg(Role::Baby)).unwrap();
        l.last_success_at = 0;
        let table: Vec<u64> = (0..8).map(|i| l.delay(1_000, i)).collect();
        assert_eq!(
            table,
            vec![0, 1_000, 2_000, 4_000, 8_000, 15_000, 15_000, 15_000]
        );
        assert_eq!(l.delay(599_999, 9), 15_000);
        assert_eq!(l.delay(600_000, 9), 30_000);
        assert_eq!(
            l.delay(600_000, 2),
            2_000,
            "the cap only applies to the tail"
        );
    }

    #[test]
    fn inputs_before_start_and_after_stop_are_ignored() {
        let mut l = Link::new(cfg(Role::Baby)).unwrap();
        assert!(l.handle(0, LinkInput::Tick).is_empty());
        assert!(l.handle(0, LinkInput::SignalingOpened).is_empty());
        assert!(l.status().is_none());
        let a = l.handle(1, LinkInput::Start { entropy: 1 });
        assert_eq!(a[0], LinkAction::OpenSignaling);
        l.handle(2, LinkInput::Stop);
        assert_eq!(l.status().unwrap().state, LinkState::Stopped);
        assert!(l.handle(3, LinkInput::Tick).is_empty());
        let a = l.handle(4, LinkInput::Start { entropy: 1 });
        assert_eq!(a[0], LinkAction::OpenSignaling, "start again after stop");
    }

    #[test]
    fn status_events_are_emitted_only_on_change() {
        let (mut l, a) = started(Role::Parent);
        assert_eq!(
            a.iter()
                .filter(|x| matches!(
                    x,
                    LinkAction::Emit {
                        event: LinkEvent::Status { .. }
                    }
                ))
                .count(),
            1
        );
        for t in [100u64, 200, 300] {
            let a = l.handle(t, LinkInput::Tick);
            assert!(
                a.iter().all(|x| !matches!(x, LinkAction::Emit { .. })),
                "{a:?}"
            );
        }
    }

    #[test]
    fn escalation_levels_follow_last_alive_thresholds() {
        let (mut l, _) = started(Role::Parent);
        let level = |l: &mut Link, t: u64| {
            l.handle(t, LinkInput::Tick);
            l.status().unwrap().level
        };
        // Keep signaling alive so only the level logic is under test.
        assert_eq!(level(&mut l, 19_999), 0);
        assert_eq!(level(&mut l, 20_000), 1);
        assert_eq!(level(&mut l, 59_999), 1);
        assert_eq!(level(&mut l, 60_000), 2);
        assert_eq!(level(&mut l, 179_999), 2);
        assert_eq!(level(&mut l, 180_000), 3);
    }

    #[test]
    fn custom_alarm_threshold_moves_level_2() {
        let mut c = cfg(Role::Parent);
        c.alarm_after_ms = 90_000;
        let mut l = Link::new(c).unwrap();
        l.handle(0, LinkInput::Start { entropy: 1 });
        l.handle(89_999, LinkInput::Tick);
        assert_eq!(l.status().unwrap().level, 1);
        l.handle(90_000, LinkInput::Tick);
        assert_eq!(l.status().unwrap().level, 2);
    }

    #[test]
    fn link_lost_is_emitted_once_per_outage() {
        let (mut l, _) = started(Role::Parent);
        let mut lost = 0;
        for t in (0..200_000).step_by(500) {
            for a in l.handle(t, LinkInput::Tick) {
                if matches!(
                    a,
                    LinkAction::Emit {
                        event: LinkEvent::LinkLost { .. }
                    }
                ) {
                    lost += 1;
                }
            }
        }
        assert_eq!(lost, 1);
    }

    #[test]
    fn fresh_attempt_ids_are_deterministic_128_bit_hex() {
        let mut a = Link::new(cfg(Role::Baby)).unwrap();
        let mut b = Link::new(cfg(Role::Baby)).unwrap();
        a.rng = SplitMix64(7);
        b.rng = SplitMix64(7);
        let (x, y) = (a.fresh_aid(), b.fresh_aid());
        assert_eq!(x, y);
        assert_eq!(x.len(), 32);
        assert!(crate::protocol::valid_aid(&x));
        assert_ne!(x, a.fresh_aid());
    }

    #[test]
    fn splitmix_matches_the_reference_sequence() {
        let mut r = SplitMix64(1_234_567);
        assert_eq!(r.next(), 6_457_827_717_110_365_317);
        assert_eq!(r.next(), 3_203_168_211_198_807_973);
    }

    #[test]
    fn surface_types_round_trip_through_their_json_contract() {
        let inputs = vec![
            LinkInput::Start { entropy: 1 },
            LinkInput::Stop,
            LinkInput::AcknowledgeAlarm,
            LinkInput::NetworkAvailable,
            LinkInput::SignalingOpened,
            LinkInput::SignalingMessage { text: "x".into() },
            LinkInput::SignalingClosed,
            LinkInput::AttemptLocalDescription {
                attempt: 1,
                sdp_type: "offer".into(),
                sdp: "v=0".into(),
            },
            LinkInput::AttemptLocalIce {
                attempt: 1,
                candidate: IceCandidate {
                    candidate: "c".into(),
                    sdp_mid: None,
                    sdp_m_line_index: Some(0),
                },
            },
            LinkInput::AttemptDataOpen { attempt: 1 },
            LinkInput::AttemptDataMessage {
                attempt: 1,
                text: "t".into(),
            },
            LinkInput::AttemptDataClosed { attempt: 1 },
            LinkInput::AttemptDtlsFingerprint {
                attempt: 1,
                fingerprint: "AB:CD".into(),
            },
            LinkInput::AttemptPcState {
                attempt: 1,
                state: "failed".into(),
            },
            LinkInput::AudioLevel { rms: 0.5 },
            LinkInput::DeviceStatus {
                status: DeviceStatus {
                    battery_pct: Some(50),
                    ..Default::default()
                },
            },
            LinkInput::SetListen { enabled: true },
            LinkInput::RequestProbe,
            LinkInput::SetSnooze { until_ms: Some(5) },
            LinkInput::SetSnooze { until_ms: None },
            LinkInput::RequestQuiet,
            LinkInput::QuietApplied {
                state: DeviceStatus::default(),
            },
            LinkInput::Tick,
        ];
        for i in inputs {
            let v = serde_json::to_value(&i).unwrap();
            assert!(v["type"].is_string());
            assert_eq!(serde_json::from_value::<LinkInput>(v).unwrap(), i);
        }
        let v = serde_json::to_value(LinkInput::AcknowledgeAlarm).unwrap();
        assert_eq!(v, json!({"type": "acknowledge_alarm"}));
        let status = LinkStatus {
            state: LinkState::PeerStopped,
            level: 0,
            key: None,
            cause: None,
            acknowledged_until: None,
            last_alive_ms: Some(1_234),
        };
        let v = serde_json::to_value(LinkEvent::Status {
            status: status.clone(),
        })
        .unwrap();
        assert_eq!(v["status"]["state"], "peer_stopped");
        assert_eq!(
            serde_json::from_value::<LinkEvent>(v).unwrap(),
            LinkEvent::Status { status }
        );
        let a = LinkAction::Emit {
            event: LinkEvent::NoiseAlert {
                rms: 0.5,
                probe: false,
                snoozed: true,
            },
        };
        assert_eq!(
            serde_json::from_value::<LinkAction>(serde_json::to_value(&a).unwrap()).unwrap(),
            a
        );
    }

    #[test]
    fn message_keys_lists_every_ui_key() {
        let keys = message_keys();
        for k in [
            "pairing_key_mismatch",
            "update_required",
            "server_unreachable",
            "peer_absent",
            "media_blocked",
            "wrong_key_attempt",
        ] {
            assert!(keys.contains(&k), "{k}");
        }
        let mut sorted = keys.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), keys.len());
    }

    #[test]
    fn only_the_parent_snoozes_and_requests() {
        let (mut l, _) = started(Role::Baby);
        l.handle(
            1,
            LinkInput::SetSnooze {
                until_ms: Some(10_000),
            },
        );
        assert!(l.snooze_until.is_none());
        assert!(l.handle(2, LinkInput::RequestProbe).is_empty());
        assert!(l.handle(3, LinkInput::RequestQuiet).is_empty());
        let (mut p, _) = started(Role::Parent);
        p.handle(
            1,
            LinkInput::SetSnooze {
                until_ms: Some(10_000),
            },
        );
        assert_eq!(p.snooze_until, Some(10_000));
        p.handle(2, LinkInput::SetSnooze { until_ms: None });
        assert_eq!(p.snooze_until, None);
    }

    #[test]
    fn terminal_state_only_accepts_stop() {
        let (mut l, _) = started(Role::Baby);
        l.handle(1, LinkInput::SignalingOpened);
        l.handle(
            2,
            LinkInput::SignalingMessage {
                text: json!({"type":"error","code":"UNSUPPORTED_PROTOCOL","message":"x"})
                    .to_string(),
            },
        );
        assert_eq!(l.status().unwrap().state, LinkState::FailedNeedsUser);
        assert!(l.handle(3, LinkInput::Tick).is_empty());
        assert!(l.handle(4, LinkInput::NetworkAvailable).is_empty());
        assert!(l.handle(5, LinkInput::Start { entropy: 1 }).is_empty());
        l.handle(6, LinkInput::Stop);
        assert_eq!(l.status().unwrap().state, LinkState::Stopped);
    }

    #[test]
    fn terminal_is_refused_once_the_key_was_verified() {
        let mut cfg = cfg(Role::Baby);
        cfg.paired_before = true;
        let mut l = Link::new(cfg).unwrap();
        l.handle(0, LinkInput::Start { entropy: 1 });
        let mut out = Vec::new();
        l.terminal("pairing_key_mismatch", &mut out);
        l.terminal("update_required", &mut out);
        assert!(out.is_empty());
        assert_eq!(l.mode, Mode::Running);
        let (mut fresh, _) = started(Role::Baby);
        fresh.terminal("update_required", &mut out);
        assert_eq!(fresh.mode, Mode::Failed("update_required".into()));
    }

    #[test]
    fn wrong_room_in_welcome_is_ignored() {
        let (mut l, _) = started(Role::Baby);
        l.handle(1, LinkInput::SignalingOpened);
        l.handle(
            2,
            LinkInput::SignalingMessage {
                text: json!({"type":"welcome","clientId":"x","roomId":"other"}).to_string(),
            },
        );
        assert_eq!(l.sig, Sig::Open);
    }

    #[test]
    fn malformed_server_input_never_changes_state() {
        let (mut l, _) = started(Role::Parent);
        l.handle(1, LinkInput::SignalingOpened);
        for junk in ["", "{}", "[]", "null", "{\"type\":\"members\"}", "\u{0}"] {
            let a = l.handle(2, LinkInput::SignalingMessage { text: junk.into() });
            assert!(a
                .iter()
                .all(|x| !matches!(x, LinkAction::CreateAttempt { .. })));
        }
        assert_eq!(l.sig, Sig::Open);
    }
}
