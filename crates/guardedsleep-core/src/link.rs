//! `Link`: one monitoring relationship from `start` to `stop` (ADR-017 §2-§11). It owns the
//! mailbox set (the relay mailbox, `mailbox`), the sessions of the sealed signaling envelope, the
//! (at most one) live [`Attempt`], liveness, escalation, listen intent, snooze, peer status and the
//! baby's missed-event buffer. Platforms talk only to the
//! `Link`: they feed [`LinkInput`]s with a **monotonic** millisecond clock (it must keep running
//! in deep sleep; `Date.now()` clamped to never go backwards on the web) and execute the returned
//! [`LinkAction`]s. Call `tick` at least every 500 ms while monitoring; Doze gaps are time jumps.
//!
//! Platform contract worth stating plainly:
//! - After `close_mailbox` the platform must discard further events of that socket, and a failed
//!   `open_mailbox` is reported as `mailbox_closed`. Every signaling message on the relay mailbox
//!   is a sealed envelope (`envelope`); the platform forwards the frames and never reads them.
//! - After `close_attempt` the platform may still deliver late callbacks for that handle; the
//!   core drops them.
//! - `close_attempt` is always emitted before the `create_attempt` that replaces it.

// Peer-supplied messages flow through here: return errors instead of panicking (AGENTS.md §5.3).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

use std::collections::VecDeque;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chacha20::ChaCha20Rng;
use chacha20::rand_core::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use zeroize::Zeroize;

use crate::attempt::{Attempt, AttemptConfig, AttemptEvent, DEFAULT_ATTEMPT_TIMEOUT_MS};
use crate::envelope::{
    CHALLENGE_LEN, CHALLENGE_VALID_MS, MessageClass, OpenError, Purpose, Reply, SealError, Sealed,
    SecretKey, SessionId, SessionState, derive_key,
};
use crate::ice::IceServer;
use crate::invite::{InviteMailbox, validate_mailboxes};
use crate::mailbox::{Mailbox, MailboxId, MailboxKind, MailboxSet, MailboxState};
use crate::noise::{GateChange, NoiseGate};
use crate::pairing::{
    PARENT_SECRET_LEN, ParentSecret, baby_proof_mac, decode_psk, derive_room_id, macs_equal,
    parent_proof_mac,
};
use crate::protocol::{
    DataMessage, DeviceStatus, IceCandidate, MAX_BUNDLED_CANDIDATES, Member, Role, ServerMessage,
    SignalPayload, clean_device_name, encode_data_message, encode_hello, encode_ping,
    encode_signal, parse_server_message, parse_signal_payload, signal_payload_json,
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
/// The presence message (`here`): its content is only a tag; role, session and challenge travel in
/// the authenticated envelope fields.
const HERE_MESSAGE: &str = r#"{"type":"here"}"#;
/// Unsolicited `here` repeats while the peer is a room member but has not confirmed a session:
/// after these delays, then every [`HERE_LONG_MS`].
const HERE_BACKOFF_MS: [u64; 3] = [5_000, 15_000, 60_000];
const HERE_LONG_MS: u64 = 300_000;
/// While an attempt is verified the mailboxes are closed; every 30 minutes they open briefly to
/// check that a recovery path still exists (`signaling-mailboxes`, "Mailboxes closed while
/// connected").
const PROBE_EVERY_MS: u64 = 30 * 60 * 1000;
/// A reachability probe that has not connected after this long counts as unreachable.
const PROBE_TIMEOUT_MS: u64 = 10_000;
/// A pinned baby waits this long after the DTLS verification for the parent proof.
const PARENT_PROOF_WAIT_MS: u64 = 10_000;
/// At most one `unknown_parent_attempt` warning per this long.
const PARENT_WARN_MIN_MS: u64 = 300_000;
/// After this many attempts in a row in which the pinned baby gave no proof the parent says the
/// baby does not recognise it.
const BABY_UNRECOGNISED_AFTER: u32 = 3;
/// The peers' wall clocks may differ this much (seconds) before the `clock_skew` hint appears.
const CLOCK_SKEW_S: u64 = 600;
/// Hint keys (`LinkEvent::Hint`); also in [`message_keys`].
const HINT_CLOCK_SKEW: &str = "clock_skew";
const HINT_RECOVERY: &str = "recovery_path_unavailable";
/// An offer, answer or `request_offer` that got no reply after this long is sent once more, with
/// the same envelope id (`signaling-mailboxes`, "Delivery is acknowledged by the peer").
const RESEND_AFTER_MS: u64 = 6_000;
/// A baby that sees a ready parent waits this long for the parent's `request_offer` before it
/// offers unasked (`Offer direction`). Without it the baby's presence-triggered offer and the
/// parent's request cross and the setup builds two attempts. Longer than a mailbox round trip,
/// shorter than [`RESEND_AFTER_MS`], so a lost request costs one grace, not a resend.
const BABY_OFFER_GRACE_MS: u64 = 2_500;
/// Local ICE candidates are bundled into one `ice` message: the first batch leaves this long after
/// the first candidate, a second batch no sooner than [`ICE_SECOND_BATCH_MS`] after the first.
const ICE_FLUSH_MS: u64 = 400;
const ICE_SECOND_BATCH_MS: u64 = 2_000;
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
        // security_warning keys
        "wrong_key_attempt",
        "unknown_parent_attempt",
        // sticky causes (`LinkStatus::cause`) that tell the user to pair again
        "pair_again",
        "baby_unrecognised",
        // hint keys (`LinkEvent::Hint`)
        HINT_CLOCK_SKEW,
        HINT_RECOVERY,
    ]
}

/// Length of [`LinkConfig::seed`].
pub const SEED_LEN: usize = 32;

/// The seed travels as 64 lowercase hex characters in JSON (config of the wasm binding and
/// simulation traces); UniFFI sees plain bytes.
mod seed_hex {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};
    use zeroize::Zeroizing;

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        // The hex text is the seed too: wipe it when done.
        let mut out = Zeroizing::new(String::with_capacity(bytes.len() * 2));
        for b in bytes {
            out.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
            out.push(char::from_digit(u32::from(b & 0xf), 16).unwrap_or('0'));
        }
        s.serialize_str(&out)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let text = Zeroizing::new(String::deserialize(d)?);
        if !text.len().is_multiple_of(2) || !text.is_ascii() {
            return Err(D::Error::custom("seed must be hex"));
        }
        text.as_bytes()
            .chunks(2)
            .map(|pair| {
                let digit = |c: u8| char::from(c).to_digit(16).map(|v| v as u8);
                match pair {
                    [hi, lo] => match (digit(*hi), digit(*lo)) {
                        (Some(h), Some(l)) => Ok((h << 4) | l),
                        _ => Err(D::Error::custom("seed must be hex")),
                    },
                    _ => Err(D::Error::custom("seed must be hex")),
                }
            })
            .collect()
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct LinkConfig {
    pub role: Role,
    /// Pairing PSK (base64url). The room id is derived from it.
    pub psk_b64url: String,
    /// 32 fresh random bytes for the in-core CSPRNG (envelope nonces, session nonces, parent
    /// secret). The platform draws a new seed for every run (Android `SecureRandom`, web
    /// `crypto.getRandomValues`); reusing one would repeat nonces under a long-lived key.
    /// Secret-class: never logged. `Link::new` fails without exactly 32 bytes, takes the bytes
    /// and wipes its copy. JSON form: 64 hex characters.
    #[serde(with = "seed_hex")]
    #[cfg_attr(feature = "tsify", tsify(type = "string"))]
    pub seed: Vec<u8>,
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
    /// The parent secret the platform persisted from the last [`LinkEvent::ParentPinned`] (baby:
    /// the pinned parent; parent: the secret it drew, acknowledged or not). `None` before the first
    /// pairing. Secret-class: never logged; `Link::new` takes the bytes and wipes its copy. JSON
    /// form: 64 hex characters.
    #[serde(default)]
    #[cfg_attr(feature = "tsify", tsify(optional, type = "string"))]
    pub pinned_parent: Option<ParentSecret>,
    /// The end of the first-pairing window in Unix seconds: the expiry of the invite. Parent:
    /// the raw secret is sent only before it (give the parsed invite's expiry on the first run and
    /// the last `window_until_s` of `ParentPinned` on later runs; `None` after the baby
    /// acknowledged, so the parent only proves). Baby: `parent_pin` is accepted only before it;
    /// with `None` a baby that was paired before accepts no first parent at all.
    #[serde(default)]
    #[cfg_attr(feature = "tsify", tsify(optional))]
    pub pin_window_until_s: Option<u64>,
    /// Parent: this device's name, sent to the baby over the verified DataChannel only (at most 64
    /// characters, control characters removed). Empty: no name. Never a fingerprint or an id.
    #[serde(default)]
    pub device_name: String,
    /// The pairing's mailbox set: every `relay` and `lan` entry is one relay mailbox the core opens
    /// at that address. Baby: the set is also sent to the parent after each verification (the
    /// mailbox update); empty: none is sent.
    #[serde(default)]
    pub mailboxes: Vec<InviteMailbox>,
    /// Development builds only: accept loopback `l:` entries in a received mailbox update, like
    /// `parse_invite`'s flag of the same name.
    #[serde(default)]
    pub allow_dev_loopback: bool,
    /// "Only same Wi-Fi or my own server". Reserved for the project-server entries of the set
    /// (`alpha-connection-model` design section 8); the core does not read it since the Nostr
    /// mailboxes it used to switch off are gone.
    #[serde(default)]
    pub only_local: bool,
}

fn default_true() -> bool {
    true
}

// Hand-written so the PSK and the seed can never reach a log through `{:?}`.
impl std::fmt::Debug for LinkConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkConfig")
            .field("role", &self.role)
            .field("psk_b64url", &"<redacted>")
            .field("seed", &"<redacted>")
            .field("paired_before", &self.paired_before)
            .field("heartbeat_interval_ms", &self.heartbeat_interval_ms)
            .field("alarm_after_ms", &self.alarm_after_ms)
            .field("noise_threshold", &self.noise_threshold)
            .field("noise_hold_ms", &self.noise_hold_ms)
            .field("require_dtls_fingerprint", &self.require_dtls_fingerprint)
            .field(
                "pinned_parent",
                &self.pinned_parent.as_ref().map(|_| "<redacted>"),
            )
            .field("pin_window_until_s", &self.pin_window_until_s)
            .field("device_name", &self.device_name)
            .field("mailboxes", &self.mailboxes)
            .field("allow_dev_loopback", &self.allow_dev_loopback)
            .field("only_local", &self.only_local)
            .finish()
    }
}

impl LinkConfig {
    /// `seed` is the 32 fresh random bytes of [`LinkConfig::seed`]; `Link::new` rejects any other
    /// length.
    pub fn with_defaults(role: Role, psk_b64url: String, seed: &[u8]) -> Self {
        Self {
            role,
            psk_b64url,
            seed: seed.to_vec(),
            paired_before: false,
            heartbeat_interval_ms: 10_000,
            alarm_after_ms: 60_000,
            noise_threshold: 0.12,
            noise_hold_ms: 800,
            require_dtls_fingerprint: true,
            pinned_parent: None,
            pin_window_until_s: None,
            device_name: String::new(),
            mailboxes: Vec::new(),
            allow_dev_loopback: false,
            only_local: false,
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
        if self
            .pinned_parent
            .as_ref()
            .is_some_and(|p| p.bytes.len() != PARENT_SECRET_LEN)
        {
            return Err(ConfigError::InvalidParentSecret);
        }
        Ok(psk)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
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
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
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
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
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
        id: MailboxId,
    },
    /// The socket of mailbox `id` closed or failed to open (`reason` is diagnostic, not a UI key).
    /// Also the answer to a refused `open_mailbox`.
    MailboxClosed {
        id: MailboxId,
        reason: String,
    },
    /// One inbound frame of mailbox `id`. `bytes` is the WebSocket text frame (UTF-8): every
    /// mailbox kind speaks JSON text frames.
    MailboxFrame {
        id: MailboxId,
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

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub enum LinkAction {
    /// Open a socket to exactly `url` for mailbox `id`.
    /// After `close_mailbox` the platform discards further events of that socket; a failed open is
    /// reported as `mailbox_closed`.
    OpenMailbox {
        id: MailboxId,
        kind: MailboxKind,
        url: String,
    },
    /// Send one WebSocket text frame on mailbox `id`.
    MailboxSend {
        id: MailboxId,
        frame: String,
    },
    CloseMailbox {
        id: MailboxId,
    },
    /// `ice_servers`: the STUN/TURN servers of the pairing's relays for this attempt (design
    /// section 7); the platform adds a user-entered TURN server. Empty (and then omitted) for a
    /// pairing without a relay mailbox: host candidates only.
    CreateAttempt {
        attempt: u32,
        offerer: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        ice_servers: Vec<IceServer>,
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

// Written by hand so that the one action that carries the parent secret (`parent_pin`) never
// prints it, whatever logs a `{:?}` of an action list.
impl std::fmt::Debug for LinkAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkAction::OpenMailbox { id, kind, url } => f
                .debug_struct("OpenMailbox")
                .field("id", id)
                .field("kind", kind)
                .field("url", url)
                .finish(),
            LinkAction::MailboxSend { id, frame } => f
                .debug_struct("MailboxSend")
                .field("id", id)
                .field("frame", frame)
                .finish(),
            LinkAction::CloseMailbox { id } => {
                f.debug_struct("CloseMailbox").field("id", id).finish()
            }
            LinkAction::CreateAttempt {
                attempt,
                offerer,
                ice_servers,
            } => f
                .debug_struct("CreateAttempt")
                .field("attempt", attempt)
                .field("offerer", offerer)
                .field("ice_servers", ice_servers)
                .finish(),
            LinkAction::CloseAttempt { attempt } => f
                .debug_struct("CloseAttempt")
                .field("attempt", attempt)
                .finish(),
            LinkAction::CreateOffer { attempt } => f
                .debug_struct("CreateOffer")
                .field("attempt", attempt)
                .finish(),
            LinkAction::SetRemoteDescription {
                attempt,
                sdp_type,
                sdp,
            } => f
                .debug_struct("SetRemoteDescription")
                .field("attempt", attempt)
                .field("sdp_type", sdp_type)
                .field("sdp", sdp)
                .finish(),
            LinkAction::CreateAnswer { attempt } => f
                .debug_struct("CreateAnswer")
                .field("attempt", attempt)
                .finish(),
            LinkAction::AddIceCandidate { attempt, candidate } => f
                .debug_struct("AddIceCandidate")
                .field("attempt", attempt)
                .field("candidate", candidate)
                .finish(),
            LinkAction::SendData { attempt, text } => {
                let shown: &dyn std::fmt::Debug = if text.contains("\"parent_pin\"") {
                    &"<parent_pin redacted>"
                } else {
                    text
                };
                f.debug_struct("SendData")
                    .field("attempt", attempt)
                    .field("text", shown)
                    .finish()
            }
            LinkAction::SetMicEnabled { enabled } => f
                .debug_struct("SetMicEnabled")
                .field("enabled", enabled)
                .finish(),
            LinkAction::ApplyQuiet => f.write_str("ApplyQuiet"),
            LinkAction::SendProbeTone { duration_ms } => f
                .debug_struct("SendProbeTone")
                .field("duration_ms", duration_ms)
                .finish(),
            LinkAction::Emit { event } => f.debug_struct("Emit").field("event", event).finish(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
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
    /// The parent secret to persist, next to the PSK, as `LinkConfig::pinned_parent` together with
    /// `window_until_s` as `LinkConfig::pin_window_until_s`. Emitted by the parent when it draws the
    /// secret and again before every repeat of `parent_pin` while the baby has not acknowledged it
    /// (`window_until_s`: the end of the first-pairing window), once more with `None` when the
    /// baby acknowledged it; and by the baby when it pins (`None`). Repeats are idempotent.
    ///
    /// **Binding rule:** the platform must durably persist the secret before it executes any later
    /// action of the same `handle` output (the `parent_pin` message follows in that very output). If
    /// persisting fails it must drop the rest of the output; the next repeat emits the event again.
    /// Secret-class: the one event that carries a secret, so platforms must never log it.
    ParentPinned {
        #[cfg_attr(feature = "tsify", tsify(type = "string"))]
        secret: ParentSecret,
        #[cfg_attr(feature = "tsify", tsify(optional))]
        window_until_s: Option<u64>,
    },
    /// Baby: the parent device that is now trusted for this attempt, by the name it sent over the
    /// verified DataChannel ("Connected to <name>"). Empty if the parent sent none.
    ParentConnected {
        name: String,
    },
    /// Parent: the baby sent its current mailbox set; the platform replaces its stored set.
    MailboxesUpdated {
        mailboxes: Vec<InviteMailbox>,
    },
    /// A calm hint appeared or went away (`active`); `key` is `clock_skew` or
    /// `recovery_path_unavailable`. Not an alarm.
    Hint {
        key: String,
        active: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
enum Mode {
    Idle,
    Running,
    Failed(String),
}

struct Live {
    attempt: Attempt,
}

/// The `(kind, url)` of every entry of a mailbox set that speaks the relay protocol and passes the
/// invite grammar: one relay mailbox each.
fn relay_entries(
    mailboxes: &[InviteMailbox],
    allow_dev_loopback: bool,
) -> Vec<(MailboxKind, String)> {
    mailboxes
        .iter()
        .filter_map(|m| {
            validate_mailboxes(std::slice::from_ref(m), allow_dev_loopback)
                .and_then(|v| v.into_iter().next())
        })
        .map(|m| (m.kind, m.url))
        .collect()
}

/// The parent secret inside the core: wiped on drop, never printed, compared in constant time.
struct SecretBytes([u8; PARENT_SECRET_LEN]);

impl SecretBytes {
    fn from_slice(bytes: &[u8]) -> Option<Self> {
        <[u8; PARENT_SECRET_LEN]>::try_from(bytes).ok().map(Self)
    }

    fn to_parent_secret(&self) -> ParentSecret {
        ParentSecret {
            bytes: self.0.to_vec(),
        }
    }

    fn same(&self, other: &[u8]) -> bool {
        other.len() == self.0.len()
            && self
                .0
                .iter()
                .zip(other)
                .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                == 0
    }
}

impl Clone for SecretBytes {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl Drop for SecretBytes {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// A reachability check of the mailboxes while an attempt is verified.
struct Probe {
    started_at: u64,
    /// Mailboxes of the check that have not answered yet.
    pending: Vec<MailboxId>,
    /// At least one mailbox answered.
    ok: bool,
}

/// An offer, answer or `request_offer` that waits for the peer's reply; sent once more after
/// [`RESEND_AFTER_MS`] (same envelope id).
struct PendingResend {
    sealed: Sealed,
    sent_at: u64,
    resent: bool,
}

/// Local candidates waiting for their batch.
struct IceBuf {
    aid: String,
    candidates: Vec<IceCandidate>,
    due: u64,
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
    /// Cryptographic generator for nonces and secrets, seeded from `LinkConfig::seed`. Kept across
    /// `start` so a restarted `Link` never replays a nonce stream. Consumed by the envelope
    /// (`alpha-connection-model` tasks 2.2 and 3.2).
    #[allow(dead_code)]
    csprng: Option<ChaCha20Rng>,
    /// Last `wall_clock` input (Unix seconds) and the monotonic time it arrived: the `here` message
    /// and the `clock_skew` comparison with the peer read it.
    wall_unix_s: Option<u64>,
    wall_at: u64,
    started_at: u64,

    /// The pinned parent secret (baby: the pinned parent; parent: the one it drew).
    pin: Option<SecretBytes>,
    /// Parent: a secret drawn at the first verification that the baby has not acknowledged yet.
    pin_unacked: bool,
    /// The end of the first-pairing window (Unix seconds), and whether this run drew the secret
    /// without a window (tests, no invite expiry known).
    window_until: Option<u64>,
    drawn_in_run: bool,
    /// What the session layer was last told to require.
    sess_require: bool,
    /// Baby: an attempt arrived but no first parent may be pinned (window closed, nothing
    /// pinned): the pairing must be done again. Parent: the baby repeatedly could not prove the
    /// secret. Both are sticky causes in the status.
    pair_again: bool,
    baby_missing: u32,
    baby_unrecognised: bool,
    /// Baby with a pinned parent: the attempt verified at the DTLS level but the parent proof is
    /// still due (deadline). While set, the microphone path, status, alerts and the heartbeat level
    /// stay closed to that attempt.
    proof_due: Option<u64>,
    /// Baby: when the last `unknown_parent_attempt` warning went out (they are rate-limited).
    last_parent_warn: Option<u64>,
    /// The calm hints currently raised (`clock_skew`, `recovery_path_unavailable`).
    hints: Vec<&'static str>,
    /// While an attempt is verified: when the next reachability check is due, and the one running.
    probe_next_at: u64,
    probe: Option<Probe>,

    /// The mailbox set with the transport state of each.
    mb: MailboxSet,
    /// The peer's wall clock differs from ours by more than 10 minutes.
    peer_skew: bool,
    resend: Option<PendingResend>,
    ice: Option<IceBuf>,
    ice_batches: u32,
    ice_last_flush: Option<u64>,
    /// The key of the sealed signaling envelope (`signaling-mailboxes`, "Sealed signaling envelope").
    env_key: SecretKey,
    /// Sessions, replay windows and challenges of this run. Never cleared on a mailbox reopen.
    sess: SessionState,
    /// The last `here` we sent that carries a challenge for a peer session that has not confirmed
    /// yet, and when it was first sent. If the peer's echo is lost, repeating exactly this message
    /// makes the peer (who confirmed us meanwhile) echo again (`envelope`, "a lost echo").
    pending_reply: Option<(Reply, u64)>,

    live: Option<Live>,
    next_handle: u32,
    next_try_at: u64,
    fail_count: u32,
    hold_until: u64,
    /// Parent: deadline and own `aid` of the outstanding `request_offer`.
    outstanding: Option<(u64, String)>,
    /// Baby: the `aid` of the latest valid `request_offer`.
    pending_request: Option<String>,
    /// Baby: since when a peer has been ready without a `request_offer` having arrived. The baby
    /// offers on its own only after [`BABY_OFFER_GRACE_MS`], so that the parent's request (which
    /// the parent sends as soon as it hears the baby) normally comes first and one attempt serves
    /// the setup.
    ready_since: Option<u64>,

    ever_verified: bool,
    verified_in_run: bool,
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

/// The envelope class of a signaling payload: it fixes the padded size on the wire.
fn class_of(payload: &SignalPayload) -> MessageClass {
    match payload {
        SignalPayload::Sdp { .. } => MessageClass::Description,
        SignalPayload::Ice { .. } | SignalPayload::IceBundle { .. } => MessageClass::Candidates,
        SignalPayload::RequestOffer { .. } | SignalPayload::StopNotice => MessageClass::Control,
    }
}

fn emit(out: &mut Vec<LinkAction>, event: LinkEvent) {
    out.push(LinkAction::Emit { event });
}

fn round4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

impl Link {
    pub fn new(mut cfg: LinkConfig) -> Result<Self, ConfigError> {
        let mut seed: [u8; SEED_LEN] = cfg
            .seed
            .as_slice()
            .try_into()
            .map_err(|_| ConfigError::InvalidSeed)?;
        let csprng = ChaCha20Rng::from_seed(seed);
        seed.zeroize();
        cfg.seed.zeroize();
        Self::build(cfg, &mut Some(csprng))
    }

    /// Everything but the seed check, so `start` can rebuild the state around the live CSPRNG. The
    /// generator is taken out of `slot` only once the config validated, so a failure leaves it
    /// where it was and nobody has to invent a stand-in.
    fn build(cfg: LinkConfig, slot: &mut Option<ChaCha20Rng>) -> Result<Self, ConfigError> {
        let psk = cfg.validate()?;
        let mut csprng = slot.take().ok_or(ConfigError::InvalidSeed)?;
        let room = derive_room_id(&psk);
        let env_key = derive_key(&psk, Purpose::Seal).ok_or(ConfigError::InvalidPsk)?;
        // A fresh session per run: the generator carries on across `start`, so a restarted `Link`
        // never repeats one.
        let mut sess = SessionState::new(cfg.role, SessionId::random(&mut csprng));
        let gate = NoiseGate::new(cfg.noise_threshold, cfg.noise_hold_ms);
        // Take the parent secret out of the config: the core keeps the only (wiped-on-drop) copy.
        let mut cfg = cfg;
        let pin = cfg.pinned_parent.take().and_then(|mut p| {
            let secret = SecretBytes::from_slice(&p.bytes);
            p.bytes.zeroize();
            secret
        });
        let pin_from_config = pin.is_some();
        let role_is_parent = cfg.role == Role::Parent;
        let role_is_baby = cfg.role == Role::Baby;
        let cfg_window = cfg.pin_window_until_s;
        // A baby requires the parent's tag as soon as it holds a pin. A parent that is still inside
        // a first-pairing window does not require the baby's (the baby may never have pinned);
        // the pump turns it on once the window is known to be closed.
        sess.set_parent_pin(
            pin.as_ref().map(|s| s.0.as_slice()),
            role_is_baby && pin_from_config,
        );
        let mb = MailboxSet::new(&relay_entries(&cfg.mailboxes, cfg.allow_dev_loopback));
        let link = Self {
            pin,
            pin_unacked: pin_from_config && role_is_parent && cfg_window.is_some(),
            window_until: cfg_window,
            drawn_in_run: false,
            sess_require: role_is_baby && pin_from_config,
            pair_again: false,
            baby_missing: 0,
            baby_unrecognised: false,
            proof_due: None,
            last_parent_warn: None,
            hints: Vec::new(),
            probe_next_at: 0,
            probe: None,
            wall_at: 0,
            liveness_ms: cfg.liveness_ms(),
            ever_verified: cfg.paired_before,
            cfg,
            psk,
            room,
            mode: Mode::Idle,
            has_started: false,
            now: 0,
            rng: SplitMix64(0),
            csprng: Some(csprng),
            wall_unix_s: None,
            started_at: 0,
            mb,
            peer_skew: false,
            resend: None,
            ice: None,
            ice_batches: 0,
            ice_last_flush: None,
            env_key,
            sess,
            pending_reply: None,
            live: None,
            next_handle: 1,
            next_try_at: 0,
            fail_count: 0,
            hold_until: 0,
            outstanding: None,
            pending_request: None,
            ready_since: None,
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
        };
        Ok(link)
    }

    /// The PSK-derived room id the link joins (22 characters).
    pub fn room_id(&self) -> &str {
        &self.room
    }

    /// Last wall clock the platform reported (Unix seconds).
    pub fn wall_clock_unix_s(&self) -> Option<u64> {
        self.wall_unix_s
    }

    pub fn status(&self) -> Option<&LinkStatus> {
        self.last_status.as_ref()
    }

    /// Handle of the live attempt, if any (for tests and diagnostics).
    pub fn live_attempt(&self) -> Option<u32> {
        self.live.as_ref().map(|l| l.attempt.handle())
    }

    /// The peer's confirmed session is current and reachable on the relay (for tests and
    /// diagnostics).
    pub fn peer_confirmed(&self) -> bool {
        self.peer_ready()
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
            // A plain setter: no timers run, so reporting the clock never moves anything.
            LinkInput::WallClock { unix_s } => {
                self.wall_unix_s = Some(unix_s);
                self.wall_at = now;
                return out;
            }
            LinkInput::Stop => self.on_stop(&mut out),
            LinkInput::AcknowledgeAlarm => self.on_acknowledge(now, &mut out),
            LinkInput::NetworkAvailable => self.on_network_available(now, &mut out),
            LinkInput::MailboxOpened { id } => self.on_signaling_opened(id, now, &mut out),
            LinkInput::MailboxFrame { id, bytes } => {
                self.on_signaling_text(id, now, &bytes, &mut out)
            }
            LinkInput::MailboxClosed { id, .. } => {
                if self
                    .mb
                    .get(id)
                    .is_some_and(|m| m.state != MailboxState::Closed)
                {
                    if self.probing() {
                        // The check failed to connect: no backoff, no reconnect (the mailboxes
                        // stay closed while the attempt is verified). The other mailboxes of the
                        // check may still answer.
                        if let Some(m) = self.mb.get_mut(id) {
                            m.state = MailboxState::Closed;
                        }
                        if let Some(p) = self.probe.as_mut() {
                            p.pending.retain(|x| *x != id);
                        }
                        let (empty, ok) = self
                            .probe
                            .as_ref()
                            .map_or((true, false), |p| (p.pending.is_empty(), p.ok));
                        if empty {
                            self.finish_probe(now, ok, &mut out);
                        }
                    } else {
                        self.signaling_down(id, now, false, &mut out);
                    }
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
                self.resend = None;
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

    fn open_action(&self, i: usize) -> Option<LinkAction> {
        self.mb.relays.get(i).map(|m| LinkAction::OpenMailbox {
            id: m.id,
            kind: m.kind,
            url: m.url.clone(),
        })
    }

    fn close_action(id: MailboxId) -> LinkAction {
        LinkAction::CloseMailbox { id }
    }

    /// A frame for relay mailbox `id`.
    fn send_action(id: MailboxId, frame: String) -> LinkAction {
        LinkAction::MailboxSend { id, frame }
    }

    /// Starts opening relay mailbox `i` (its socket is the platform's, the address is ours).
    fn open_relay(&mut self, i: usize, now: u64, out: &mut Vec<LinkAction>) {
        if let Some(a) = self.open_action(i) {
            if let Some(m) = self.mb.relays.get_mut(i) {
                m.state = MailboxState::Opening;
                m.changed_at = now;
            }
            out.push(a);
        }
    }

    /// Closes the socket of relay mailbox `i` if one is open and forgets what the relay told us.
    fn close_relay(&mut self, i: usize, out: &mut Vec<LinkAction>) {
        let Some(m) = self.mb.relays.get_mut(i) else {
            return;
        };
        if m.state != MailboxState::Closed {
            out.push(Self::close_action(m.id));
        }
        m.state = MailboxState::Closed;
        m.client_id = None;
        m.members.clear();
        m.known_peers.clear();
        m.peer_client = None;
        m.here_sent = 0;
    }

    fn close_all_relays(&mut self, out: &mut Vec<LinkAction>) {
        for i in 0..self.mb.relays.len() {
            self.close_relay(i, out);
        }
    }

    fn live_mut(&mut self, handle: u32) -> Option<&mut Live> {
        self.live
            .as_mut()
            .filter(|l| l.attempt.handle() == handle && !l.attempt.is_finished())
    }

    /// The live attempt verified and, on a baby that pinned a parent, the parent proof passed too.
    fn live_verified(&self) -> bool {
        self.live
            .as_ref()
            .is_some_and(|l| self.trusted(l) && !l.attempt.is_finished())
    }

    /// `l` is verified at the DTLS level and not waiting for a parent proof.
    fn trusted(&self, l: &Live) -> bool {
        l.attempt.is_verified() && self.proof_due.is_none()
    }

    /// A reachability check is running (mailboxes are held closed otherwise).
    fn probing(&self) -> bool {
        self.probe.is_some() && self.live_verified()
    }

    fn other_role(&self) -> Role {
        match self.cfg.role {
            Role::Baby => Role::Parent,
            Role::Parent => Role::Baby,
        }
    }

    /// The room member of relay `i` that speaks for the peer: the one the confirmed session last
    /// spoke from there, otherwise the first member with the peer's role.
    fn peer_member(&self, i: usize) -> Option<&Member> {
        self.peer_member_of(self.mb.relays.get(i)?)
    }

    fn peer_member_of<'a>(&self, relay: &'a Mailbox) -> Option<&'a Member> {
        let other = self.other_role();
        let mut others = relay.members.iter().filter(|m| m.role == other);
        match relay.peer_client.as_deref() {
            Some(id) => relay
                .members
                .iter()
                .find(|m| m.role == other && m.client_id == id)
                .or_else(|| others.next()),
            None => others.next(),
        }
    }

    /// Attempts and requests need a peer that confirmed a session in this run and that is a room
    /// member right now on at least one relay (`signaling-mailboxes`, "Session confirmation").
    fn peer_ready(&self) -> bool {
        (0..self.mb.relays.len()).any(|i| self.relay_ready(i))
    }

    /// Some relay shows a member with the peer's role (not necessarily a confirmed session).
    fn any_peer_present(&self) -> bool {
        self.mb.relays.iter().any(|m| m.peer_present())
    }

    /// Relay `i` is joined, a session of the peer is confirmed and the client id it last spoke
    /// from on this relay is still a member there.
    fn relay_ready(&self, i: usize) -> bool {
        let Some(m) = self.mb.relays.get(i) else {
            return false;
        };
        m.state == MailboxState::Joined
            && self.sess.peer_session().is_some()
            && m.peer_client.as_deref().is_some_and(|id| {
                m.members
                    .iter()
                    .any(|x| x.role == self.other_role() && x.client_id == id)
            })
    }

    /// The `signal` frames that carry `sealed` to the peer: one per joined relay on which a member
    /// with the peer's role exists. The same envelope id goes everywhere; the receiver processes
    /// the first copy.
    fn envelope_actions(&self, sealed: &Sealed) -> Vec<LinkAction> {
        self.mb
            .relays
            .iter()
            .filter_map(|m| {
                if m.state != MailboxState::Joined {
                    return None;
                }
                let to = self.peer_member_of(m)?.client_id.clone();
                Some(self.envelope_action(m.id, &to, sealed))
            })
            .collect()
    }

    fn fresh_aid(&mut self) -> String {
        format!("{:016x}{:016x}", self.rng.next(), self.rng.next())
    }

    // ---- lifecycle -------------------------------------------------------------------------

    fn on_start(&mut self, now: u64, entropy: u64, out: &mut Vec<LinkAction>) {
        let mut cfg = self.cfg.clone();
        cfg.paired_before = self.ever_verified;
        // A pin made in an earlier run of this object carries over.
        cfg.pinned_parent = self.pin.as_ref().map(SecretBytes::to_parent_secret);
        // The seed was consumed in `new`; the generator carries over into the fresh state.
        // A failed build leaves the generator in place.
        let Ok(mut fresh) = Link::build(cfg, &mut self.csprng) else {
            return;
        };
        fresh.now = now;
        fresh.mode = Mode::Running;
        fresh.has_started = true;
        fresh.rng = SplitMix64(entropy);
        fresh.started_at = now;
        fresh.last_success_at = now;
        fresh.next_try_at = now;
        fresh.next_handle = self.next_handle;
        fresh.last_status = self.last_status.take();
        fresh.pin_unacked = self.pin_unacked;
        fresh.window_until = self.window_until;
        fresh.drawn_in_run = self.drawn_in_run;
        fresh.last_parent_warn = self.last_parent_warn;
        fresh.sync_session_pin(now);
        *self = fresh;
        for i in 0..self.mb.relays.len() {
            self.open_relay(i, now, out);
        }
    }

    fn on_stop(&mut self, out: &mut Vec<LinkAction>) {
        if self.mode == Mode::Running {
            self.announce_stop(out);
            self.close_live(out);
            self.close_all_relays(out);
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
        // Sealed and bound to the peer's confirmed session: without one there is nobody who could
        // act on it, and a stop to a stale session would be ignored anyway.
        self.send_payload(&SignalPayload::StopNotice, out);
    }

    fn terminal(&mut self, key: &str, out: &mut Vec<LinkAction>) {
        // A terminal state renders without alarm and zeroes the level, so a link that ever proved
        // the key must stay running; only the explicit user stop ends it.
        if self.ever_verified {
            return;
        }
        self.close_live(out);
        self.close_all_relays(out);
        self.mode = Mode::Failed(key.to_owned());
    }

    fn on_network_available(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        // Platform-originated, so it may also lift the post-mismatch hold.
        self.hold_until = 0;
        for m in &mut self.mb.relays {
            if m.state == MailboxState::Closed {
                m.failures = 0;
                m.next_open_at = now;
            }
        }
        if self.live.as_ref().is_some_and(|l| !self.trusted(l)) {
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

    fn on_signaling_opened(&mut self, id: MailboxId, now: u64, out: &mut Vec<LinkAction>) {
        if self
            .mb
            .get(id)
            .is_none_or(|m| m.state != MailboxState::Opening)
        {
            return;
        }
        if self.probing() {
            // Reachable: it connected. A ping and nothing else, then the sockets are closed again.
            out.push(Self::send_action(id, encode_ping()));
            self.finish_probe(now, true, out);
            return;
        }
        if let Some(m) = self.mb.get_mut(id) {
            m.state = MailboxState::Open;
            m.last_rx = now;
            m.last_ping = now;
        }
        out.push(Self::send_action(
            id,
            encode_hello(self.cfg.role, &self.room),
        ));
        // A request already travelling over another mailbox stays valid (`Parallel mailboxes`).
        let kept = self.outstanding.take().filter(|_| self.peer_ready());
        self.immediate_retry(now);
        self.outstanding = kept;
    }

    fn signaling_down(
        &mut self,
        id: MailboxId,
        now: u64,
        emit_close: bool,
        out: &mut Vec<LinkAction>,
    ) {
        let i = id as usize;
        if !emit_close && let Some(m) = self.mb.get_mut(id) {
            // The platform already closed it: no `close_mailbox` to emit.
            m.state = MailboxState::Closed;
        }
        self.close_relay(i, out);
        // Losing one mailbox must not reset the setup while another one still reaches the peer
        // (`link-supervision`, "Parallel mailboxes"): a flapping relay would otherwise drop the
        // parent's `request_offer` again and again while the baby's attempt waits for its answer
        // until the attempt timeout. Only when no mailbox reaches the peer is the state stale.
        if !self.peer_ready() {
            self.outstanding = None;
            self.pending_request = None;
        }
        let failures = self.mb.get(id).map_or(0, |m| m.failures);
        let wait = self.delay(now, failures);
        if let Some(m) = self.mb.get_mut(id) {
            m.next_open_at = now + wait;
            m.failures += 1;
        }
    }

    fn delay(&self, now: u64, failures: u32) -> u64 {
        match BACKOFF_MS.get(failures as usize) {
            Some(d) => *d,
            None if now.saturating_sub(self.last_success_at) >= LONG_OUTAGE_MS => LONG_CAP_MS,
            None => CAP_MS,
        }
    }

    fn on_signaling_text(
        &mut self,
        id: MailboxId,
        now: u64,
        text: &str,
        out: &mut Vec<LinkAction>,
    ) {
        let i = id as usize;
        let Some(m) = self.mb.relays.get_mut(i) else {
            return;
        };
        if !matches!(m.state, MailboxState::Open | MailboxState::Joined) {
            return;
        }
        m.last_rx = now;
        let Some(msg) = parse_server_message(text) else {
            return;
        };
        match msg {
            ServerMessage::Welcome {
                client_id,
                room_id,
                ice_servers,
            } => {
                if m.state == MailboxState::Open && room_id == self.room {
                    m.ice.on_welcome(m.host.as_deref(), ice_servers);
                    m.state = MailboxState::Joined;
                    m.client_id = Some(client_id);
                    m.failures = 0;
                    m.unsupported = false;
                    self.update_required = false;
                }
            }
            ServerMessage::Members { peers } => {
                if m.state != MailboxState::Joined {
                    return;
                }
                let was_present = self.any_peer_present();
                let other = self.other_role();
                let Some(m) = self.mb.relays.get_mut(i) else {
                    return;
                };
                let me = m.client_id.clone();
                m.members = peers
                    .into_iter()
                    .filter(|x| Some(&x.client_id) != me.as_ref())
                    .collect();
                let others: Vec<String> = m
                    .members
                    .iter()
                    .filter(|x| x.role == other)
                    .map(|x| x.client_id.clone())
                    .collect();
                let fresh: Vec<String> = others
                    .iter()
                    .filter(|c| !m.known_peers.contains(c))
                    .cloned()
                    .collect();
                // The confirmed session counts only while its member is in the room.
                if m.peer_client
                    .as_ref()
                    .is_some_and(|pc| !others.contains(pc))
                {
                    m.peer_client = None;
                }
                if !fresh.is_empty() {
                    m.here_sent = 0;
                }
                m.known_peers = others;
                let present = self.any_peer_present();
                if present && !was_present {
                    // Reappearing is attacker-influenceable, so it never lifts the mismatch hold.
                    self.immediate_retry(now);
                }
                if !present {
                    self.outstanding = None;
                    self.stop_pending_absence = false;
                }
                // Presence over this relay: the room membership is the trigger, the sealed `here`
                // carries the session (design section 2).
                for c in fresh {
                    self.announce(now, i, &c, out);
                }
            }
            ServerMessage::Signal { from, payload } => {
                if m.state != MailboxState::Joined {
                    return;
                }
                self.on_envelope(now, i, &from, &payload, out);
            }
            ServerMessage::Error { code, .. } => {
                if code == "UNSUPPORTED_PROTOCOL" {
                    self.on_unsupported_protocol(id, now, out);
                } else {
                    // Retryable at the signaling layer (BABY_TAKEN, PARENT_CAP, rate limits, ...).
                    self.signaling_down(id, now, true, out);
                }
            }
            ServerMessage::IceServers(frame) => {
                if m.state == MailboxState::Joined {
                    m.ice.on_ice_servers(now, m.host.as_deref(), frame);
                }
            }
            ServerMessage::Peer { .. } | ServerMessage::Pong => {}
        }
    }

    /// Seals `message` as `class` to the peer's confirmed session.
    fn seal_signal(&mut self, class: MessageClass, message: &[u8]) -> Result<Sealed, SealError> {
        let rng = self.csprng.as_mut().ok_or(SealError::Crypto)?;
        self.sess.seal_to_peer(&self.env_key, rng, class, message)
    }

    fn envelope_action(&self, mailbox: MailboxId, to: &str, sealed: &Sealed) -> LinkAction {
        Self::send_action(
            mailbox,
            encode_signal(
                &self.room,
                Some(to),
                json!({ "env": URL_SAFE_NO_PAD.encode(&sealed.bytes) }),
            ),
        )
    }

    /// Sends one signaling payload, sealed to the peer's confirmed session, to every open
    /// mailbox. A message that does not fit its class is a sender bug: it is never truncated and
    /// surfaces as a failure reason.
    fn send_payload(&mut self, payload: &SignalPayload, out: &mut Vec<LinkAction>) {
        let class = class_of(payload);
        let message = signal_payload_json(payload).to_string();
        match self.seal_signal(class, message.as_bytes()) {
            Ok(sealed) => {
                out.extend(self.envelope_actions(&sealed));
                if matches!(
                    payload,
                    SignalPayload::RequestOffer { .. } | SignalPayload::Sdp { .. }
                ) {
                    self.resend = Some(PendingResend {
                        sealed,
                        sent_at: self.now,
                        resent: false,
                    });
                }
            }
            Err(SealError::Oversized) => emit(
                out,
                LinkEvent::Failed {
                    reason: "signal_oversized".into(),
                },
            ),
            // No confirmed peer session (or an exhausted counter): nothing to send to.
            Err(_) => {}
        }
    }

    /// An offer, answer or `request_offer` that nobody answered: once more, same envelope id, to
    /// every open mailbox.
    fn resend_housekeeping(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        let due = self
            .resend
            .as_ref()
            .is_some_and(|r| !r.resent && now.saturating_sub(r.sent_at) >= RESEND_AFTER_MS);
        if !due {
            return;
        }
        let Some(r) = self.resend.as_mut() else {
            return;
        };
        r.resent = true;
        let sealed = r.sealed.clone();
        out.extend(self.envelope_actions(&sealed));
    }

    /// Local candidates leave in batches (see [`ICE_FLUSH_MS`]).
    fn buffer_ice(&mut self, now: u64, aid: String, candidate: IceCandidate) {
        if let Some(buf) = self.ice.as_mut()
            && buf.aid == aid
        {
            if buf.candidates.len() < MAX_BUNDLED_CANDIDATES {
                buf.candidates.push(candidate);
            }
            if buf.candidates.len() >= MAX_BUNDLED_CANDIDATES {
                buf.due = now;
            }
            return;
        }
        let spaced = match (self.ice_batches, self.ice_last_flush) {
            (n, Some(at)) if n >= 1 => at.saturating_add(ICE_SECOND_BATCH_MS),
            _ => 0,
        };
        self.ice = Some(IceBuf {
            aid,
            candidates: vec![candidate],
            due: now.saturating_add(ICE_FLUSH_MS).max(spaced),
        });
    }

    fn ice_housekeeping(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        if !self.ice.as_ref().is_some_and(|b| now >= b.due) {
            return;
        }
        let Some(buf) = self.ice.take() else {
            return;
        };
        if !self.live.as_ref().is_some_and(|l| !l.attempt.is_finished()) {
            return;
        }
        let payload = SignalPayload::IceBundle {
            aid: buf.aid,
            candidates: buf.candidates,
        };
        self.send_payload(&payload, out);
        self.ice_batches = self.ice_batches.saturating_add(1);
        self.ice_last_flush = Some(now);
    }

    /// An unsolicited presence message to the member `to` of relay `i`; schedules the next repeat.
    fn send_here(&mut self, now: u64, i: usize, to: &str, out: &mut Vec<LinkAction>) {
        let Some(id) = self
            .mb
            .relays
            .get(i)
            .filter(|m| m.state == MailboxState::Joined)
            .map(|m| m.id)
        else {
            return;
        };
        let here = self.here_message(now);
        let sealed = match self.csprng.as_mut() {
            Some(rng) => self.sess.seal(
                &self.env_key,
                rng,
                MessageClass::Here,
                SessionId::NONE,
                &here,
            ),
            None => Err(SealError::Crypto),
        };
        if let Ok(sealed) = sealed {
            out.push(self.envelope_action(id, to, &sealed));
        }
        if let Some(m) = self.mb.relays.get_mut(i) {
            m.here_sent = m.here_sent.saturating_add(1);
            m.here_next_at = now.saturating_add(Self::here_wait(m.here_sent));
        }
    }

    /// The `here` that answers a candidate or echoes a challenge (`Opened::reply`).
    fn send_reply(
        &mut self,
        now: u64,
        i: usize,
        to: &str,
        reply: &Reply,
        out: &mut Vec<LinkAction>,
    ) {
        let Some(id) = self.mb.relays.get(i).map(|m| m.id) else {
            return;
        };
        self.reply_with(
            now,
            reply,
            |link, sealed| vec![link.envelope_action(id, to, sealed)],
            out,
        );
    }

    /// The answer to an envelope that came in over one relay goes out over every relay where the
    /// peer is a member: the relay it came over may be gone by the time the peer needs it, and the
    /// copies carry one envelope id, so the peer processes the first.
    fn send_reply_everywhere(&mut self, now: u64, reply: &Reply, out: &mut Vec<LinkAction>) {
        self.reply_with(
            now,
            reply,
            |link, sealed| link.envelope_actions(sealed),
            out,
        );
    }

    fn reply_with(
        &mut self,
        now: u64,
        reply: &Reply,
        route: impl FnOnce(&Self, &Sealed) -> Vec<LinkAction>,
        out: &mut Vec<LinkAction>,
    ) {
        let here = self.here_message(now);
        let Some(rng) = self.csprng.as_mut() else {
            return;
        };
        if let Ok(sealed) = self.sess.seal_reply(&self.env_key, rng, reply, &here) {
            out.extend(route(self, &sealed));
        }
        // A reply with a challenge of ours waits for the peer's echo; remember it for a repeat.
        if reply.challenge != [0; CHALLENGE_LEN] {
            // A repeat keeps the first issue time: the challenge's 60 s do not restart.
            let issued = match self.pending_reply {
                Some((old, at)) if old.challenge == reply.challenge => at,
                _ => now,
            };
            self.pending_reply = Some((*reply, issued));
        }
    }

    /// Repeats `here` to a room member that has not confirmed a session (backoff 5, 15, 60 s, then
    /// every 5 minutes). On our relay this only covers a lost message; the relay is reliable.
    fn here_housekeeping(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        for i in 0..self.mb.relays.len() {
            let Some(m) = self.mb.relays.get(i) else {
                continue;
            };
            if m.state != MailboxState::Joined
                || m.known_peers.is_empty()
                || self.relay_ready(i)
                || now < m.here_next_at
            {
                continue;
            }
            for c in m.known_peers.clone() {
                self.announce(now, i, &c, out);
            }
        }
    }

    /// A presence message for the room member `to`, plus a repeat of the challenge we still wait
    /// on: halfway through the confirmation handshake a plain `here` would not move a peer that
    /// already confirmed us (the repeat makes it echo again), and a peer that restarted needs the
    /// plain one because it does not know our earlier reply.
    fn announce(&mut self, now: u64, i: usize, to: &str, out: &mut Vec<LinkAction>) {
        self.send_here(now, i, to, out);
        let pending = match self.pending_reply {
            Some((reply, at)) if now.saturating_sub(at) <= CHALLENGE_VALID_MS => Some(reply),
            _ => {
                self.pending_reply = None;
                None
            }
        };
        if let Some(reply) = pending {
            self.send_reply(now, i, to, &reply, out);
        }
    }

    fn here_wait(sent: u32) -> u64 {
        HERE_BACKOFF_MS
            .get(sent.saturating_sub(1) as usize)
            .copied()
            .unwrap_or(HERE_LONG_MS)
    }

    /// One inbound `signal` payload of the relay: a sealed envelope or noise. Every failure is
    /// silent and changes nothing: an open error must never cause a reply (no error or timing
    /// oracle for whoever sent it).
    fn on_envelope(
        &mut self,
        now: u64,
        i: usize,
        from: &str,
        payload: &Value,
        out: &mut Vec<LinkAction>,
    ) {
        let Some(text) = payload.get("env").and_then(Value::as_str) else {
            return;
        };
        if text.len() > MessageClass::Candidates.wire_len().div_ceil(3) * 4 {
            return;
        }
        let Ok(bytes) = URL_SAFE_NO_PAD.decode(text) else {
            return;
        };
        self.open_envelope(now, i, from, &bytes, out);
    }

    /// One inbound sealed envelope from the relay (`from`: the sender's client id).
    fn open_envelope(
        &mut self,
        now: u64,
        i: usize,
        from: &str,
        bytes: &[u8],
        out: &mut Vec<LinkAction>,
    ) {
        if bytes.len() > MessageClass::Candidates.wire_len() {
            return;
        }
        let Some(rng) = self.csprng.as_mut() else {
            return;
        };
        let opened = match self.sess.open(&self.env_key, rng, now, bytes) {
            Ok(opened) => opened,
            // Another device that holds the PSK: a warning, never a reply.
            Err(OpenError::NotPinned) => {
                if self.cfg.role == Role::Baby {
                    self.warn_unknown_parent(now, out);
                } else {
                    self.warn_wrong_key(now, out);
                    self.count_untagged_baby(now, out);
                }
                return;
            }
            // The same envelope came through another relay first. The copy still shows that the
            // peer's member on this relay forwards for it, which is what readiness of this relay
            // needs when the first copy came over a relay that has since gone.
            Err(OpenError::Duplicate) => {
                self.learn_peer_route(i, from);
                return;
            }
            Err(_) => return,
        };
        if opened.confirmed {
            // A new session: where the old one was heard says nothing about it.
            for m in &mut self.mb.relays {
                m.peer_client = None;
            }
        }
        if self.sess.peer_session() == Some(opened.sender_session)
            && let Some(m) = self.mb.relays.get_mut(i)
        {
            m.peer_client = Some(from.to_owned());
        }
        // A parent is there, but this baby holds no secret for it and its invite is over: only a
        // new code on this device can pair them. (Nothing here can connect; the hint is sticky
        // until an attempt is trusted.)
        if self.cfg.role == Role::Baby && self.pin.is_none() && !self.baby_window_open(now) {
            self.pair_again = true;
        }
        if let Some(reply) = &opened.reply {
            self.send_reply_everywhere(now, reply, out);
        }
        if opened.confirmed {
            // The peer echoed our challenge: nothing is pending any more.
            self.pending_reply = None;
            self.on_peer_confirmed(now, out);
        }
        if opened.class == MessageClass::Here {
            if self.sess.peer_session() == Some(opened.sender_session) {
                self.check_clock(now, &opened.message, out);
            }
            return;
        }
        let Ok(value) = serde_json::from_slice::<Value>(&opened.message) else {
            return;
        };
        // The class is authenticated: a payload that claims another class is not ours.
        match parse_signal_payload(&value) {
            Some(p) if class_of(&p) == opened.class => {
                self.on_signal(now, i, from, opened.sender_session, p, out);
            }
            _ => {}
        }
    }

    /// Relay `i` carried a duplicate of an authenticated envelope from member `from`: when the
    /// session is confirmed, the relay has no route yet and `from` is a member with the peer's role
    /// there, that member is the route (the same trust `peer_member` already places in it).
    fn learn_peer_route(&mut self, i: usize, from: &str) {
        if self.sess.peer_session().is_none() {
            return;
        }
        let other = self.other_role();
        if let Some(m) = self.mb.relays.get_mut(i)
            && m.peer_client.is_none()
            && m.members
                .iter()
                .any(|x| x.role == other && x.client_id == from)
        {
            m.peer_client = Some(from.to_owned());
        }
    }

    /// A new session of the peer confirmed: whatever was in flight belongs to the previous one.
    fn on_peer_confirmed(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        self.pending_request = None;
        if self.live.as_ref().is_some_and(|l| !self.trusted(l)) {
            self.close_live(out);
        }
        self.immediate_retry(now);
    }

    /// Only the reply to our `hello` (before `welcome`) can mean "your version is not supported";
    /// the same error later is a forgery or noise and must not touch a running link. A verified
    /// attempt keeps running (the media path does not need the relay), so there the error only
    /// closes signaling and leaves a sticky `update_required` hint. The same holds for any link
    /// that verified in this run or before (`paired_before`): a malicious relay must not turn an
    /// outage into a silent terminal state, so escalation continues from `last_alive` and
    /// signaling is retried on the long backoff. Only a link that never verified is terminal.
    fn on_unsupported_protocol(&mut self, id: MailboxId, now: u64, out: &mut Vec<LinkAction>) {
        if self
            .mb
            .get(id)
            .is_none_or(|m| m.state != MailboxState::Open)
        {
            return;
        }
        if let Some(m) = self.mb.get_mut(id) {
            m.unsupported = true;
        }
        // One relay of several that is too old or too new says nothing about the others: the
        // link is only unsupported when every relay of the pairing refuses it.
        let all = self.mb.relays.iter().all(|m| m.unsupported);
        if self.ever_verified || !all {
            if all {
                self.update_required = true;
            }
            self.signaling_down(id, now, true, out);
            if let Some(m) = self.mb.get_mut(id) {
                m.next_open_at = now + LONG_CAP_MS;
            }
        } else {
            self.terminal("update_required", out);
        }
    }

    /// `payload` came out of an envelope that authenticated, named our session and was sent by the
    /// peer's confirmed session `sender`.
    fn on_signal(
        &mut self,
        now: u64,
        i: usize,
        from: &str,
        sender: SessionId,
        payload: SignalPayload,
        out: &mut Vec<LinkAction>,
    ) {
        if !matches!(payload, SignalPayload::StopNotice) {
            // The peer answered (or went on): nothing to resend.
            self.resend = None;
        }
        match payload.clone() {
            SignalPayload::StopNotice => self.on_signal_stop(now, sender, out),
            SignalPayload::RequestOffer { aid } => {
                if self.cfg.role == Role::Baby {
                    self.on_request_offer(now, i, from, aid, out);
                }
            }
            SignalPayload::Sdp { .. } => self.on_remote_sdp(now, i, from, &payload, out),
            SignalPayload::Ice { aid, candidate } => {
                if let Some(l) = self.live.as_ref() {
                    // A verified attempt is done negotiating; late candidates are only noise.
                    if l.attempt.accepts_ice(&aid) {
                        out.push(LinkAction::AddIceCandidate {
                            attempt: l.attempt.handle(),
                            candidate,
                        });
                    }
                }
            }
            SignalPayload::IceBundle { aid, candidates } => {
                if let Some(l) = self.live.as_ref()
                    && l.attempt.accepts_ice(&aid)
                {
                    for candidate in candidates {
                        out.push(LinkAction::AddIceCandidate {
                            attempt: l.attempt.handle(),
                            candidate,
                        });
                    }
                }
            }
        }
    }

    /// A stop that reached here authenticated under the envelope key, names our current session
    /// and comes from the peer's confirmed session, so it is that session's own deliberate stop.
    /// A stored stop of an earlier run cannot get here: its sender session is retired or was never
    /// confirmed. The stop retires the session for good.
    fn on_signal_stop(&mut self, now: u64, sender: SessionId, out: &mut Vec<LinkAction>) {
        self.sess.retire(sender);
        for m in &mut self.mb.relays {
            m.peer_client = None;
        }
        self.stop_peer(now, out);
    }

    /// The peer announced that it stopped (over a mailbox or the DataChannel): the verified attempt
    /// ends without alarm and nothing escalates.
    fn stop_peer(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        let verified_since = self
            .live
            .as_ref()
            .filter(|l| self.trusted(l))
            .map(|l| l.attempt.last_alive());
        self.set_peer_stopped();
        self.close_live(out);
        if let Some(last_alive) = verified_since {
            self.on_verified_attempt_lost(now, last_alive, out);
        }
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
        own_aid: String,
        remote_aid: Option<String>,
        offerer: bool,
        out: &mut Vec<LinkAction>,
    ) {
        // Close-before-create; there is never more than one live attempt.
        self.close_live(out);
        self.ice_batches = 0;
        self.ice_last_flush = None;
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
            ice_servers: self.ice_servers(now),
        });
        if offerer {
            out.push(LinkAction::CreateOffer { attempt: handle });
        }
        self.live = Some(Live { attempt });
    }

    /// The attempt's ICE servers: the union over the relay mailboxes (design section 7).
    fn ice_servers(&mut self, now: u64) -> Vec<IceServer> {
        let mut list = Vec::new();
        for m in &mut self.mb.relays {
            if let Some(host) = m.host.as_deref() {
                m.ice.collect(now, host, &mut list);
            }
        }
        list
    }

    fn close_live(&mut self, out: &mut Vec<LinkAction>) {
        self.proof_due = None;
        self.resend = None;
        self.ice = None;
        if let Some(l) = self.live.take() {
            out.push(LinkAction::CloseAttempt {
                attempt: l.attempt.handle(),
            });
        }
    }

    fn on_request_offer(
        &mut self,
        now: u64,
        i: usize,
        from: &str,
        aid: String,
        out: &mut Vec<LinkAction>,
    ) {
        let from_parent = self.mb.relays.get(i).is_some_and(|r| {
            r.members
                .iter()
                .any(|m| m.client_id == from && m.role == Role::Parent)
        });
        if !from_parent || self.live_verified() {
            return;
        }
        // Duplicate of the request the live (unverified) attempt already answers.
        if self
            .live
            .as_ref()
            .is_some_and(|l| l.attempt.remote_aid() == Some(aid.as_str()))
        {
            return;
        }
        if self.pending_request.as_ref().is_some_and(|a| *a == aid) {
            return;
        }
        self.pending_request = Some(aid);
        // An unverified attempt may be replaced; a request inside the backoff window waits for it.
        if now >= self.next_try_at {
            self.start_baby_attempt(now, out);
        }
    }

    fn start_baby_attempt(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        // A request answers only while a parent is still a member of some relay; without one
        // there is nobody to answer, and an unasked offer needs a peer member as well.
        let other = self.other_role();
        let member = self
            .mb
            .relays
            .iter()
            .any(|r| r.members.iter().any(|m| m.role == other));
        if !member {
            self.pending_request = None;
            return;
        }
        let remote_aid = self.pending_request.take();
        let own = self.fresh_aid();
        self.create_attempt(now, own, remote_aid, true, out);
    }

    fn on_remote_sdp(
        &mut self,
        now: u64,
        i: usize,
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
            let from_baby = self.peer_member(i).is_some_and(|m| m.client_id == from);
            if !from_baby || self.live_verified() {
                return;
            }
            if let Some(l) = self.live.as_ref()
                && l.attempt.remote_aid() == Some(aid)
            {
                return;
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
            self.create_attempt(now, own, None, false, out);
        } else if !matches!((self.cfg.role, sdp_type), (Role::Baby, "answer")) {
            return;
        }
        let Some(l) = self.live.as_mut() else {
            return;
        };
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
                AttemptEvent::Signal(SignalPayload::Ice { aid, candidate }) => {
                    self.buffer_ice(now, aid, candidate);
                }
                AttemptEvent::Signal(payload) => {
                    self.send_payload(&payload, out);
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
        let was_verified = self.trusted(live);
        // A DTLS-verified attempt that never proved its parent is neither a recovery nor a media
        // failure: it only backs off.
        let untrusted_verified = live.attempt.is_verified() && !was_verified;
        let exchanged = live.attempt.sdp_exchanged() && !untrusted_verified;
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
        // The mailboxes were closed while the attempt was verified; the housekeeping reopens them
        // now (every one at once: the peer's new session has to be heard).
        self.probe = None;
        self.set_hint(HINT_RECOVERY, false, out);
        self.last_alive = Some(last_alive);
        if self.outage_start.is_none() {
            self.outage_start = Some(last_alive);
            self.outage_max_audio_gap = 0;
        }
        // The intent died with the attempt; the parent re-sends it after a short outage.
        if self.parent_listen {
            self.parent_listen = false;
            if self.cfg.role == Role::Baby {
                emit(out, LinkEvent::ListenChanged { enabled: false });
            }
        }
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

    /// The attempt verified at the DTLS level. A baby that pinned a parent keeps the attempt
    /// untrusted until the parent proves the secret (no microphone path, no status, nothing that
    /// counts as recovery); everything else is trusted at once.
    fn on_verified(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        // A pinned pairing proves the secret in both directions before either side trusts the
        // attempt: the baby waits for the parent proof, the parent for the baby proof. A parent
        // whose secret the baby has not acknowledged yet is still in the first pairing.
        let pinned = match self.cfg.role {
            // An unpinned baby outside the first-pairing window grants no trust to anybody: it
            // waits for a proof it cannot check and refuses like a missing parent proof.
            Role::Baby => self.pin.is_some() || !self.baby_window_open(now),
            Role::Parent => self.pin.is_some() && !self.parent_unacked(now),
        };
        if pinned {
            if self.cfg.role == Role::Baby && self.pin.is_none() {
                self.pair_again = true;
            }
            self.proof_due = Some(now.saturating_add(PARENT_PROOF_WAIT_MS));
            if self.cfg.role == Role::Parent {
                self.send_parent_credentials(now, out);
            }
            return;
        }
        self.on_trusted(now, out);
    }

    /// The attempt is verified and, where a pin exists, the parent proved it: this is a link.
    fn on_trusted(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        // From here on no internet mailbox is open (design section 1): they would only give relays
        // an all-night connection pattern. Loss and `peer_stopped` reopen them.
        self.hold_mailboxes(now, out);
        let first = !self.verified_in_run;
        self.pair_again = false;
        self.baby_unrecognised = false;
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
                // The gate reports only the quiet-to-loud change, and `send_app` drops it while
                // no attempt is trusted. Noise that is still going on when the link comes up (or
                // comes back) would otherwise never reach the parent, so announce it again.
                if self.gate.is_loud() {
                    self.send_app(
                        &DataMessage::NoiseAlert {
                            rms: self.gate.last_rms(),
                            t: now as f64,
                            probe: false,
                        },
                        out,
                    );
                }
                self.last_sent_status = None;
                self.flush_status(out);
                self.send_mailbox_update(out);
            }
            Role::Parent => {
                // A pinned parent sent its proof when the attempt verified.
                if self.pin.is_none() || self.parent_unacked(now) {
                    self.send_parent_credentials(now, out);
                }
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

    // ---- parent pin ------------------------------------------------------------------------

    /// The secret the session layer tags with (parent) or checks against (baby).
    fn sync_session_pin(&mut self, now: u64) {
        let require = self.pin.is_some() && self.requires_peer_tag(now);
        self.sess_require = require;
        self.sess
            .set_parent_pin(self.pin.as_ref().map(|s| s.0.as_slice()), require);
    }

    /// Whether envelopes of the peer role must carry the peer's pin tag: the baby once pinned, the
    /// parent once the first-pairing window is over for it (acknowledged, or expired).
    fn requires_peer_tag(&self, now: u64) -> bool {
        match self.cfg.role {
            Role::Baby => self.pin.is_some(),
            Role::Parent => self.pin.is_some() && !self.parent_unacked(now),
        }
    }

    /// Parent: the baby has not acknowledged the secret and the first-pairing window is still
    /// open, so the raw secret may still be sent and the baby may not be pinned yet.
    fn parent_unacked(&self, now: u64) -> bool {
        self.pin_unacked && self.window_open(now)
    }

    /// The first-pairing window: until the invite expires. Without an expiry (tests) a secret
    /// drawn in this run stays sendable for the run; a secret from an earlier run never is.
    fn window_open(&self, now: u64) -> bool {
        match self.window_until {
            Some(until) => self.wall_now(now).is_some_and(|w| w < until),
            None => self.drawn_in_run,
        }
    }

    /// Baby: a first parent may still be pinned (the invite is valid; without an expiry, only a
    /// baby that was never paired).
    fn baby_window_open(&self, now: u64) -> bool {
        if self.pin.is_some() {
            return false;
        }
        match self.cfg.pin_window_until_s {
            Some(until) => self.wall_now(now).is_some_and(|w| w < until),
            None => !self.cfg.paired_before,
        }
    }

    /// Both DTLS fingerprints of the live attempt as `(parent, baby)`, as this side sees them.
    fn parent_baby_fingerprints(&self) -> Option<(String, String)> {
        let (local, remote) = self.live.as_ref()?.attempt.fingerprints()?;
        Some(match self.cfg.role {
            Role::Parent => (local.to_owned(), remote.to_owned()),
            Role::Baby => (remote.to_owned(), local.to_owned()),
        })
    }

    /// Parent, right after the attempt verified: the first time the secret and the device name
    /// (only over this DataChannel), later the proof of the secret bound to both fingerprints.
    ///
    /// The secret is drawn once and `ParentPinned` is emitted **before** the action that sends it:
    /// the platform must persist it before it executes the later actions, so a parent that
    /// restarts before the baby's acknowledgement arrives still holds the secret the baby pinned.
    /// Until the acknowledgement arrives in this run the parent repeats the secret (an unpinned or
    /// already pinned baby takes the same one) instead of proving it.
    fn send_parent_credentials(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        let name = clean_device_name(&self.cfg.device_name);
        if self.pin.is_none() {
            // The first pairing, and only inside the window the invite allows.
            if self.cfg.pin_window_until_s.is_some()
                && !self.window_open_for(now, self.cfg.pin_window_until_s)
            {
                return;
            }
            let Some(rng) = self.csprng.as_mut() else {
                return;
            };
            let mut bytes = [0u8; PARENT_SECRET_LEN];
            rng.fill_bytes(&mut bytes);
            let secret = SecretBytes(bytes);
            bytes.zeroize();
            self.window_until = self.cfg.pin_window_until_s;
            self.drawn_in_run = self.window_until.is_none();
            self.pin = Some(secret);
            self.pin_unacked = true;
            // Tag every envelope from now on: the baby may pin before the acknowledgement arrives.
            self.sync_session_pin(now);
        }
        let Some(pin) = self.pin.as_ref() else {
            return;
        };
        if self.parent_unacked(now) {
            let secret = pin.to_parent_secret();
            // Persist before send, on every repeat: the platform stores this before it executes
            // the `parent_pin` action that follows in the same output.
            emit(
                out,
                LinkEvent::ParentPinned {
                    secret: secret.clone(),
                    window_until_s: self.window_until,
                },
            );
            self.send_app(&DataMessage::ParentPin { secret, name }, out);
            return;
        }
        // Acknowledged, or the window is over: the raw secret never goes out again.
        let Some((fp_parent, fp_baby)) = self.parent_baby_fingerprints() else {
            return;
        };
        if let Some(mac) = parent_proof_mac(&pin.0, &fp_parent, &fp_baby) {
            self.send_app(&DataMessage::ParentProof { mac, name }, out);
        }
    }

    fn window_open_for(&self, now: u64, until: Option<u64>) -> bool {
        match until {
            Some(u) => self.wall_now(now).is_some_and(|w| w < u),
            None => true,
        }
    }

    /// Baby: a parent message while the attempt is untrusted (only the proof and the pin count),
    /// or while pinning.
    fn on_parent_proof(&mut self, now: u64, mac: &str, name: String, out: &mut Vec<LinkAction>) {
        if self.proof_due.is_none() {
            return;
        }
        let expected = self.pin.as_ref().and_then(|pin| {
            let (fp_parent, fp_baby) = self.parent_baby_fingerprints()?;
            parent_proof_mac(&pin.0, &fp_parent, &fp_baby)
        });
        match expected {
            Some(expected) if macs_equal(&expected, mac) => self.trust_parent(now, name, out),
            _ => self.refuse_parent(now, "parent_proof_mismatch", out),
        }
    }

    /// Baby: the parent sends its secret. Unpinned, it is pinned (the first verification);
    /// pinned, only the same secret passes (a parent whose acknowledgement was lost).
    fn on_parent_pin(
        &mut self,
        now: u64,
        secret: &ParentSecret,
        name: String,
        out: &mut Vec<LinkAction>,
    ) {
        match self.pin.as_ref() {
            None => {
                // Only while the invite is valid: after that a secret from "the parent" would be
                // a PSK holder's.
                if !self.baby_window_open(now) {
                    return;
                }
                let Some(pinned) = SecretBytes::from_slice(&secret.bytes) else {
                    return;
                };
                emit(
                    out,
                    LinkEvent::ParentPinned {
                        secret: pinned.to_parent_secret(),
                        window_until_s: None,
                    },
                );
                self.pin = Some(pinned);
                self.sync_session_pin(now);
                emit(out, LinkEvent::ParentConnected { name });
                self.send_app(&DataMessage::PinAck, out);
            }
            Some(pin) if pin.same(&secret.bytes) => {
                if self.proof_due.is_some() {
                    self.trust_parent(now, name, out);
                }
                self.send_app(&DataMessage::PinAck, out);
            }
            Some(_) => self.refuse_parent(now, "parent_secret_mismatch", out),
        }
    }

    /// Baby: the parent proved the secret; the attempt becomes a link.
    fn trust_parent(&mut self, now: u64, name: String, out: &mut Vec<LinkAction>) {
        self.proof_due = None;
        // The pinned parent trusts this attempt only after this proof, so it goes first: whatever
        // the baby sends next would be ignored by a parent that is still waiting for it.
        if let (Some(pin), Some((fp_parent, fp_baby))) =
            (self.pin.as_ref(), self.parent_baby_fingerprints())
            && let Some(mac) = baby_proof_mac(&pin.0, &fp_baby, &fp_parent)
        {
            self.send_app(&DataMessage::BabyProof { mac }, out);
        }
        emit(out, LinkEvent::ParentConnected { name });
        self.on_trusted(now, out);
    }

    /// Baby: an attempt whose parent could not prove the secret is closed, warned about (rate
    /// limited) and followed by the same hold as a wrong key.
    fn refuse_parent(&mut self, now: u64, reason: &str, out: &mut Vec<LinkAction>) {
        self.close_live(out);
        emit(
            out,
            LinkEvent::Failed {
                reason: reason.to_owned(),
            },
        );
        self.warn_unknown_parent(now, out);
        let d = self.delay(now, self.fail_count).max(MISMATCH_HOLD_MS);
        self.fail_count += 1;
        self.hold_until = now + d;
        self.next_try_at = self.hold_until;
    }

    /// Parent: a sender of the baby role that is not the pinned baby (rate limited).
    fn warn_wrong_key(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        if self
            .last_parent_warn
            .is_none_or(|t| now.saturating_sub(t) >= PARENT_WARN_MIN_MS)
        {
            self.last_parent_warn = Some(now);
            emit(
                out,
                LinkEvent::SecurityWarning {
                    key: "wrong_key_attempt".into(),
                },
            );
        }
    }

    fn warn_unknown_parent(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        if self
            .last_parent_warn
            .is_some_and(|t| now.saturating_sub(t) < PARENT_WARN_MIN_MS)
        {
            return;
        }
        self.last_parent_warn = Some(now);
        emit(
            out,
            LinkEvent::SecurityWarning {
                key: "unknown_parent_attempt".into(),
            },
        );
    }

    /// Parent: the baby pinned the secret. The window is over for good: the platform stores the
    /// secret without a window, and the parent only proves from now on.
    fn on_pin_ack(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        if self.cfg.role != Role::Parent || !self.pin_unacked {
            return;
        }
        self.pin_unacked = false;
        self.window_until = None;
        self.drawn_in_run = false;
        if let Some(pin) = self.pin.as_ref() {
            emit(
                out,
                LinkEvent::ParentPinned {
                    secret: pin.to_parent_secret(),
                    window_until_s: None,
                },
            );
        }
        self.sync_session_pin(now);
    }

    /// Parent with a pinned pairing: the baby's proof of the secret. Only a valid one makes the
    /// attempt a link; anything else ends it and warns, rate limited.
    fn on_baby_proof(&mut self, now: u64, mac: &str, out: &mut Vec<LinkAction>) {
        if self.cfg.role != Role::Parent || self.proof_due.is_none() {
            return;
        }
        let expected = self.pin.as_ref().and_then(|pin| {
            let (fp_parent, fp_baby) = self.parent_baby_fingerprints()?;
            baby_proof_mac(&pin.0, &fp_baby, &fp_parent)
        });
        match expected {
            Some(expected) if macs_equal(&expected, mac) => {
                self.proof_due = None;
                self.baby_missing = 0;
                self.on_trusted(now, out);
            }
            _ => self.refuse_baby(now, "baby_proof_mismatch", out),
        }
    }

    /// Parent whose secret the baby never acknowledged and whose first-pairing window is over:
    /// every envelope the baby sends lacks the tag, so it never received the secret (it was
    /// reset, or the message was lost). After a few of them the user is told to show a new code.
    /// An acknowledged pairing never counts here: a PSK holder must not be able to raise it.
    fn count_untagged_baby(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        if !(self.pin_unacked && !self.window_open(now)) {
            return;
        }
        self.baby_missing += 1;
        if self.baby_missing >= BABY_UNRECOGNISED_AFTER && !self.baby_unrecognised {
            self.baby_unrecognised = true;
            self.refresh_status(now, out);
        }
    }

    /// Parent: an attempt whose baby could not prove the secret is closed and is no contact: the
    /// outage keeps its start, so escalation runs on. The warning reuses `wrong_key_attempt`.
    fn refuse_baby(&mut self, now: u64, reason: &str, out: &mut Vec<LinkAction>) {
        // A baby that keeps not answering has no pin for this parent (it never received the
        // secret, was reset or re-paired): only a new code on the baby recovers that.
        if reason == "baby_proof_missing" {
            self.baby_missing += 1;
            if self.baby_missing >= BABY_UNRECOGNISED_AFTER {
                self.baby_unrecognised = true;
            }
        }
        self.close_live(out);
        emit(
            out,
            LinkEvent::Failed {
                reason: reason.to_owned(),
            },
        );
        self.warn_wrong_key(now, out);
        self.try_failed(now);
    }

    // ---- mailbox update --------------------------------------------------------------------

    /// Baby: the current mailbox set for the parent to store (after each trusted verification).
    fn send_mailbox_update(&self, out: &mut Vec<LinkAction>) {
        if let Some(mailboxes) =
            validate_mailboxes(&self.cfg.mailboxes, self.cfg.allow_dev_loopback)
        {
            self.send_app(&DataMessage::Mailboxes { mailboxes }, out);
        }
    }

    /// Parent: a mailbox set from the baby replaces the stored one (if it is a valid set).
    fn on_mailbox_update(&mut self, mailboxes: &[InviteMailbox], out: &mut Vec<LinkAction>) {
        if let Some(set) = validate_mailboxes(mailboxes, self.cfg.allow_dev_loopback) {
            // The platform stores the set as sent; the core keeps it for the next mailbox update.
            self.cfg.mailboxes = set.clone();
            // The mailboxes are held closed while verified; the next reopening uses the new set.
            if self.mb.all_closed() {
                self.mb = MailboxSet::new(&relay_entries(&set, self.cfg.allow_dev_loopback));
            }
            emit(out, LinkEvent::MailboxesUpdated { mailboxes: set });
        }
    }

    // ---- mailboxes while verified ----------------------------------------------------------

    /// Closes the internet mailboxes (today only the relay; a `lan` listener would stay) when the
    /// attempt becomes a link. Nothing is a failure here: no backoff, and the first check is due in
    /// 30 minutes.
    fn hold_mailboxes(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        self.close_all_relays(out);
        for m in &mut self.mb.relays {
            m.failures = 0;
            m.next_open_at = now;
        }
        self.outstanding = None;
        self.pending_request = None;
        self.ready_since = None;
        self.probe = None;
        self.resend = None;
        self.ice = None;
        self.probe_next_at = now.saturating_add(PROBE_EVERY_MS);
    }

    /// While an attempt is verified: start a check when it is due and give up on one that does not
    /// answer. A check opens every mailbox without sending anything of the pairing: the relay gets
    /// a connect and an application ping (no `hello`, so no room id).
    fn probe_housekeeping(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        match &self.probe {
            None => {
                if now >= self.probe_next_at && !self.mb.relays.is_empty() && self.mb.all_closed() {
                    let ids: Vec<MailboxId> = self.mb.relays.iter().map(|m| m.id).collect();
                    for i in 0..self.mb.relays.len() {
                        self.open_relay(i, now, out);
                    }
                    self.probe = Some(Probe {
                        started_at: now,
                        pending: ids,
                        ok: false,
                    });
                }
            }
            Some(p) => {
                if now.saturating_sub(p.started_at) >= PROBE_TIMEOUT_MS {
                    let ok = p.ok;
                    self.finish_probe(now, ok, out);
                }
            }
        }
    }

    /// The check ended: close every socket still open and raise or clear the
    /// `recovery_path_unavailable` hint ("nothing reachable" is its verdict).
    fn finish_probe(&mut self, now: u64, reachable: bool, out: &mut Vec<LinkAction>) {
        self.close_all_relays(out);
        self.probe = None;
        self.probe_next_at = now.saturating_add(PROBE_EVERY_MS);
        self.set_hint(HINT_RECOVERY, !reachable, out);
    }

    /// Raises or clears a calm hint, emitting only on change.
    fn set_hint(&mut self, key: &'static str, active: bool, out: &mut Vec<LinkAction>) {
        let present = self.hints.contains(&key);
        if active == present {
            return;
        }
        if active {
            self.hints.push(key);
        } else {
            self.hints.retain(|k| *k != key);
        }
        emit(
            out,
            LinkEvent::Hint {
                key: key.to_owned(),
                active,
            },
        );
    }

    // ---- wall clock ------------------------------------------------------------------------

    /// Our wall clock now (Unix seconds): the last report plus the monotonic time since.
    fn wall_now(&self, now: u64) -> Option<u64> {
        self.wall_unix_s
            .map(|w| w.saturating_add(now.saturating_sub(self.wall_at) / 1000))
    }

    /// The `here` message: a tag plus our wall clock when we know it.
    fn here_message(&self, now: u64) -> Vec<u8> {
        match self.wall_now(now) {
            Some(t) => format!(r#"{{"type":"here","t":{t}}}"#).into_bytes(),
            None => HERE_MESSAGE.as_bytes().to_vec(),
        }
    }

    /// A `here` of the peer's confirmed session carries its wall clock: more than 10 minutes apart
    /// raises the `clock_skew` hint on both devices, closer clears it.
    fn check_clock(&mut self, now: u64, message: &[u8], out: &mut Vec<LinkAction>) {
        let Some(own) = self.wall_now(now) else {
            return;
        };
        let Some(theirs) = serde_json::from_slice::<Value>(message)
            .ok()
            .and_then(|v| v.get("t").and_then(Value::as_u64))
        else {
            return;
        };
        self.peer_skew = own.abs_diff(theirs) > CLOCK_SKEW_S;
        self.refresh_clock_hint(out);
    }

    /// `clock_skew` is up while the peer's clock differs from ours by more than [`CLOCK_SKEW_S`].
    fn refresh_clock_hint(&mut self, out: &mut Vec<LinkAction>) {
        self.set_hint(HINT_CLOCK_SKEW, self.peer_skew, out);
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
        // An attempt that has not proved its peer hears nothing but the proof (or the secret).
        if self.proof_due.is_some() {
            match msg {
                DataMessage::BabyProof { mac } => self.on_baby_proof(now, &mac, out),
                DataMessage::ParentProof { mac, name } => {
                    self.on_parent_proof(now, &mac, name, out)
                }
                DataMessage::ParentPin { secret, name } => {
                    self.on_parent_pin(now, &secret, name, out)
                }
                _ => {}
            }
            return;
        }
        if msg == DataMessage::StopNotice {
            self.on_dc_stop(now, out);
            return;
        }
        match (self.cfg.role, msg) {
            (Role::Baby, DataMessage::Listen { enabled }) => {
                // The baby unit shows "Parent is listening" from this event only.
                if self.parent_listen != enabled {
                    self.parent_listen = enabled;
                    emit(out, LinkEvent::ListenChanged { enabled });
                }
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
            (Role::Baby, DataMessage::ParentPin { secret, name }) => {
                self.on_parent_pin(now, &secret, name, out)
            }
            (Role::Parent, DataMessage::PinAck) => self.on_pin_ack(now, out),
            (Role::Parent, DataMessage::Mailboxes { mailboxes }) => {
                self.on_mailbox_update(&mailboxes, out)
            }
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
        if self.live.is_none() {
            return;
        }
        // The same stop may still be stored on a mailbox: retire the session so it cannot be used.
        if let Some(session) = self.sess.peer_session() {
            self.sess.retire(session);
        }
        for m in &mut self.mb.relays {
            m.peer_client = None;
        }
        self.stop_peer(now, out);
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
        if self.alert_log.len() >= ALERT_LOG_CAP
            && let Some((t, _)) = self.alert_log.pop_front()
        {
            self.alert_log_dropped_t = Some(self.alert_log_dropped_t.map_or(t, |d| d.max(t)));
        }
        self.alert_log.push_back((now, rms));
    }

    fn apply_mic(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        let probing = self.probe_until.is_some_and(|u| now < u);
        // The microphone path opens only for a trusted attempt: verified at the DTLS level and,
        // where a parent is pinned, after the parent proved the secret. A noisy room with no such
        // attempt (an outage, an attempt that has not proven itself) leaves it closed.
        let enabled =
            (self.parent_listen || self.noise_streaming || probing) && self.live_verified();
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
        // Nothing but the acknowledgement goes to an attempt whose parent is not proven yet.
        if self.proof_due.is_some()
            && !matches!(
                msg,
                DataMessage::PinAck
                    | DataMessage::ParentProof { .. }
                    | DataMessage::BabyProof { .. }
            )
        {
            return;
        }
        if let Some(l) = self.live.as_ref()
            && l.attempt.encode_app_message(msg).is_some()
        {
            out.push(LinkAction::SendData {
                attempt: l.attempt.handle(),
                text: encode_data_message(msg),
            });
        }
    }

    // ---- housekeeping ----------------------------------------------------------------------

    fn pump(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        if self.mode == Mode::Running {
            if self.pin.is_some() && self.requires_peer_tag(now) != self.sess_require {
                self.sync_session_pin(now);
            }
            self.signaling_housekeeping(now, out);
            self.here_housekeeping(now, out);
            self.ice_housekeeping(now, out);
            self.resend_housekeeping(now, out);
            if self.cfg.role == Role::Baby {
                // The sound level is not for a parent that has not proven itself: with a pinned
                // parent it stays out of the heartbeats until the attempt is trusted (the
                // heartbeat sent at verification included).
                let rms = (self.pin.is_none() || self.live_verified())
                    .then(|| round4(self.gate.last_rms()));
                if let Some(l) = self.live.as_mut() {
                    l.attempt.set_heartbeat_rms(rms);
                }
            }
            if self.proof_due.is_some_and(|due| now >= due) {
                if self.cfg.role == Role::Baby {
                    self.refuse_parent(now, "parent_proof_missing", out);
                } else {
                    self.refuse_baby(now, "baby_proof_missing", out);
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
        if self.live_verified() {
            // The mailboxes are closed on purpose; only the reachability check runs.
            self.probe_housekeeping(now, out);
            return;
        }
        for i in 0..self.mb.relays.len() {
            let Some(m) = self.mb.relays.get(i) else {
                continue;
            };
            match m.state {
                MailboxState::Closed => {
                    if now >= m.next_open_at {
                        self.open_relay(i, now, out);
                    }
                }
                MailboxState::Opening => {
                    if now.saturating_sub(m.changed_at) >= SIGNALING_OPEN_TIMEOUT_MS {
                        self.signaling_down(m.id, now, true, out);
                    }
                }
                MailboxState::Open | MailboxState::Joined => {
                    let id = m.id;
                    if now.saturating_sub(m.last_rx) >= SIGNALING_DEAD_MS {
                        self.signaling_down(id, now, true, out);
                    } else if now.saturating_sub(m.last_ping) >= SIGNALING_PING_MS {
                        if let Some(m) = self.mb.relays.get_mut(i) {
                            m.last_ping = now;
                        }
                        out.push(Self::send_action(id, encode_ping()));
                    }
                }
            }
        }
    }

    fn schedule(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        if !self.peer_ready() {
            self.ready_since = None;
        }
        if self.live.is_some()
            || !self.peer_ready()
            || (self.peer_stopped && self.stop_pending_absence)
            || now < self.next_try_at
        {
            return;
        }
        match self.cfg.role {
            Role::Baby => {
                if self.pending_request.is_none() {
                    let since = *self.ready_since.get_or_insert(now);
                    if now.saturating_sub(since) < BABY_OFFER_GRACE_MS {
                        return;
                    }
                }
                self.start_baby_attempt(now, out)
            }
            Role::Parent => {
                if self.outstanding.is_some() {
                    return;
                }
                let aid = self.fresh_aid();
                self.send_payload(&SignalPayload::RequestOffer { aid: aid.clone() }, out);
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
        let c = if !self
            .mb
            .relays
            .iter()
            .any(|m| m.state == MailboxState::Joined)
        {
            "server_unreachable"
        } else if !self.peer_ready() {
            "peer_absent"
        } else if self.media_failures >= MEDIA_BLOCKED_AFTER {
            "media_blocked"
        } else {
            return None;
        };
        Some(c.to_owned())
    }

    fn refresh_status(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        // Sticky causes, most urgent first.
        let hint = if self.update_required {
            Some("update_required".to_owned())
        } else if self.pair_again {
            Some("pair_again".to_owned())
        } else if self.baby_unrecognised {
            Some("baby_unrecognised".to_owned())
        } else {
            None
        };
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
        let mut c = LinkConfig::with_defaults(role, encode_psk(&[3u8; 32]), &[7u8; 32]);
        c.mailboxes = vec![InviteMailbox {
            kind: MailboxKind::Relay,
            url: "wss://relay.test".into(),
        }];
        c
    }

    fn started(role: Role) -> (Link, Vec<LinkAction>) {
        let mut l = Link::new(cfg(role)).unwrap();
        let a = l.handle(0, LinkInput::Start { entropy: 42 });
        (l, a)
    }

    #[test]
    fn debug_of_the_action_that_sends_the_parent_secret_hides_it() {
        let secret = "ab".repeat(32);
        let pin = LinkAction::SendData {
            attempt: 1,
            text: format!(r#"{{"type":"parent_pin","v":3,"secret":"{secret}","name":"x"}}"#),
        };
        let shown = format!("{pin:?}");
        assert!(!shown.contains(&secret), "{shown}");
        assert!(shown.contains("redacted"));
        let other = LinkAction::SendData {
            attempt: 1,
            text: r#"{"type":"ping"}"#.to_owned(),
        };
        assert!(format!("{other:?}").contains("ping"));
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
            "seed": "07".repeat(32),
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
    fn construction_needs_a_32_byte_seed() {
        for bad in [0usize, 1, 31, 33, 64] {
            let c = LinkConfig::with_defaults(Role::Baby, encode_psk(&[3u8; 32]), &vec![1u8; bad]);
            assert_eq!(Link::new(c).err(), Some(ConfigError::InvalidSeed), "{bad}");
        }
        // A config without the field does not even parse.
        let missing = serde_json::from_value::<LinkConfig>(json!({
            "role": "baby",
            "psk_b64url": encode_psk(&[3u8; 32]),
            "heartbeat_interval_ms": 10000,
            "alarm_after_ms": 60000,
            "noise_threshold": 0.2,
            "noise_hold_ms": 300
        }));
        assert!(missing.is_err());
    }

    #[test]
    fn the_seed_is_hex_in_json_and_never_in_debug() {
        let c = cfg(Role::Parent);
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["seed"], "07".repeat(32));
        let back: LinkConfig = serde_json::from_value(v).unwrap();
        assert_eq!(back, c);
        let mut bad = serde_json::to_value(&c).unwrap();
        bad["seed"] = json!("zz");
        assert!(serde_json::from_value::<LinkConfig>(bad).is_err());
        let dbg = format!("{c:?}");
        assert!(!dbg.contains("0707"), "{dbg}");
        assert!(!dbg.contains(&c.psk_b64url), "{dbg}");
    }

    #[test]
    fn the_csprng_survives_a_restart_and_a_failed_start() {
        use chacha20::rand_core::Rng;
        let mut l = Link::new(cfg(Role::Baby)).unwrap();
        l.csprng.as_mut().unwrap().next_u64();
        let pos = l.csprng.as_ref().unwrap().get_word_pos();
        let seed = l.csprng.as_ref().unwrap().get_seed();
        assert_ne!(seed, [0; SEED_LEN]);
        // A start with a config that no longer validates leaves the generator where it was.
        l.cfg.heartbeat_interval_ms = 1;
        l.handle(0, LinkInput::Start { entropy: 1 });
        assert_eq!(l.csprng.as_ref().unwrap().get_word_pos(), pos);
        assert_eq!(l.csprng.as_ref().unwrap().get_seed(), seed);
        // A normal start, stop and start keeps the same stream.
        l.cfg.heartbeat_interval_ms = 10_000;
        l.handle(1, LinkInput::Start { entropy: 1 });
        l.handle(2, LinkInput::Stop);
        l.handle(3, LinkInput::Start { entropy: 2 });
        assert_eq!(l.csprng.as_ref().unwrap().get_seed(), seed);
        assert!(l.csprng.as_ref().unwrap().get_word_pos() >= pos);
    }

    #[test]
    fn the_wall_clock_is_stored_and_changes_no_behaviour() {
        let mut a = Link::new(cfg(Role::Baby)).unwrap();
        let mut b = Link::new(cfg(Role::Baby)).unwrap();
        a.handle(0, LinkInput::Start { entropy: 1 });
        b.handle(0, LinkInput::Start { entropy: 1 });
        assert_eq!(a.wall_clock_unix_s(), None);
        assert!(
            a.handle(
                10,
                LinkInput::WallClock {
                    unix_s: 1_800_000_000
                }
            )
            .is_empty()
        );
        assert_eq!(a.wall_clock_unix_s(), Some(1_800_000_000));
        // A jump of a day moves no timer: the same ticks give the same actions.
        a.handle(
            20,
            LinkInput::WallClock {
                unix_s: 1_800_086_400,
            },
        );
        for t in [1_000, 30_000, 70_000] {
            assert_eq!(a.handle(t, LinkInput::Tick), b.handle(t, LinkInput::Tick));
        }
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
        assert!(l.handle(0, LinkInput::MailboxOpened { id: 0 }).is_empty());
        assert!(l.status().is_none());
        let a = l.handle(1, LinkInput::Start { entropy: 1 });
        assert!(matches!(a[0], LinkAction::OpenMailbox { id: 0, .. }));
        l.handle(2, LinkInput::Stop);
        assert_eq!(l.status().unwrap().state, LinkState::Stopped);
        assert!(l.handle(3, LinkInput::Tick).is_empty());
        let a = l.handle(4, LinkInput::Start { entropy: 1 });
        assert!(
            matches!(a[0], LinkAction::OpenMailbox { id: 0, .. }),
            "start again after stop"
        );
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
            LinkInput::MailboxOpened { id: 0 },
            LinkInput::MailboxFrame {
                id: 0,
                bytes: "x".into(),
            },
            LinkInput::MailboxClosed {
                id: 0,
                reason: "closed".into(),
            },
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
        l.handle(1, LinkInput::MailboxOpened { id: 0 });
        l.handle(
            2,
            LinkInput::MailboxFrame {
                id: 0,
                bytes: json!({"type":"error","code":"UNSUPPORTED_PROTOCOL","message":"x",
                })
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
        l.handle(1, LinkInput::MailboxOpened { id: 0 });
        l.handle(
            2,
            LinkInput::MailboxFrame {
                id: 0,
                bytes: json!({"type":"welcome","clientId":"x","roomId":"other",
                })
                .to_string(),
            },
        );
        assert_eq!(l.mb.relays[0].state, MailboxState::Open);
    }

    fn two_relays(role: Role) -> (Link, Vec<LinkAction>) {
        let mut c = cfg(role);
        c.mailboxes = vec![
            InviteMailbox {
                kind: MailboxKind::Relay,
                url: "wss://one.test".into(),
            },
            InviteMailbox {
                kind: MailboxKind::Relay,
                url: "wss://two.test".into(),
            },
            // A duplicate address is the same mailbox.
            InviteMailbox {
                kind: MailboxKind::Relay,
                url: "wss://two.test".into(),
            },
        ];
        let mut l = Link::new(c).unwrap();
        let a = l.handle(0, LinkInput::Start { entropy: 42 });
        (l, a)
    }

    #[test]
    fn every_relay_entry_is_its_own_mailbox_opened_at_its_own_address() {
        let (_, first) = two_relays(Role::Baby);
        let opens: Vec<_> = first
            .iter()
            .filter_map(|a| match a {
                LinkAction::OpenMailbox { id, url, .. } => Some((*id, url.as_str())),
                _ => None,
            })
            .collect();
        assert_eq!(opens, vec![(0, "wss://one.test"), (1, "wss://two.test")]);
    }

    #[test]
    fn one_relay_failing_leaves_the_other_mailbox_alone() {
        let (mut l, _) = two_relays(Role::Baby);
        l.handle(1, LinkInput::MailboxOpened { id: 0 });
        l.handle(1, LinkInput::MailboxOpened { id: 1 });
        let welcome =
            |room: &str| json!({"type":"welcome","clientId":"x","roomId":room}).to_string();
        let room = l.room_id().to_owned();
        l.handle(
            2,
            LinkInput::MailboxFrame {
                id: 1,
                bytes: welcome(&room),
            },
        );
        assert_eq!(l.mb.relays[1].state, MailboxState::Joined);
        let out = l.handle(
            3,
            LinkInput::MailboxClosed {
                id: 0,
                reason: "x".into(),
            },
        );
        // The failed relay retries on its own backoff (none for the first failure); the joined
        // one is not touched.
        assert_eq!(l.mb.relays[0].state, MailboxState::Opening);
        assert_eq!(l.mb.relays[1].state, MailboxState::Joined);
        assert!(
            !out.iter()
                .any(|a| matches!(a, LinkAction::CloseMailbox { id: 1 }))
        );
    }

    #[test]
    fn events_of_an_unknown_mailbox_are_ignored() {
        let (mut l, first) = started(Role::Baby);
        assert!(matches!(
            first.first(),
            Some(LinkAction::OpenMailbox { id: 0, kind: MailboxKind::Relay, url }) if url == "wss://relay.test"
        ));
        assert!(l.handle(1, LinkInput::MailboxOpened { id: 7 }).is_empty());
        assert_eq!(l.mb.relays[0].state, MailboxState::Opening);
        l.handle(2, LinkInput::MailboxOpened { id: 0 });
        l.handle(
            3,
            LinkInput::MailboxClosed {
                id: 7,
                reason: "x".into(),
            },
        );
        let frame = LinkInput::MailboxFrame {
            id: 7,
            bytes: json!({"type":"welcome","clientId":"x","roomId": l.room_id()}).to_string(),
        };
        l.handle(4, frame);
        assert_eq!(
            l.mb.relays[0].state,
            MailboxState::Open,
            "only mailbox 0 exists"
        );
    }

    fn frame(l: &mut Link, now: u64, id: MailboxId, v: serde_json::Value) {
        l.handle(
            now,
            LinkInput::MailboxFrame {
                id,
                bytes: v.to_string(),
            },
        );
    }

    fn join_all(l: &mut Link, now: u64, ids: &[MailboxId]) {
        let room = l.room_id().to_owned();
        for id in ids {
            l.handle(now, LinkInput::MailboxOpened { id: *id });
            frame(
                l,
                now,
                *id,
                json!({"type":"welcome","clientId":"x","roomId":room}),
            );
        }
    }

    fn turn(host: &str, user: &str, ttl: u64) -> serde_json::Value {
        json!({"type":"ice_servers","urls":[format!("stun:{host}:3478"), format!("turn:{host}:3478")],
            "username":user,"credential":"S3CRET","ttl":ttl})
    }

    #[test]
    fn each_relay_without_a_set_gets_its_builtin_stun_and_a_lan_only_pairing_gets_nothing() {
        let (mut l, _) = two_relays(Role::Baby);
        let list = l.ice_servers(0);
        let urls: Vec<_> = list.iter().flat_map(|s| s.urls.clone()).collect();
        assert_eq!(urls, vec!["stun:one.test:3478", "stun:two.test:3478"]);
        assert!(list.iter().all(|s| s.credential.is_none()));

        let mut c = cfg(Role::Baby);
        c.mailboxes = vec![InviteMailbox {
            kind: MailboxKind::Lan,
            url: "ws://192.168.1.5:8787".into(),
        }];
        let mut lan = Link::new(c).unwrap();
        lan.handle(0, LinkInput::Start { entropy: 1 });
        assert!(lan.ice_servers(0).is_empty());
    }

    #[test]
    fn the_attempt_list_is_the_union_of_the_valid_sets_with_a_fallback_per_relay() {
        let (mut l, _) = two_relays(Role::Baby);
        join_all(&mut l, 1_000, &[0, 1]);
        frame(&mut l, 1_000, 0, turn("one.test", "100:a", 600));
        let list = l.ice_servers(2_000);
        // Relay one: its TURN set; relay two: no set, so its STUN fallback.
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].username.as_deref(), Some("100:a"));
        assert_eq!(list[1].urls, vec!["stun:two.test:3478"]);
        frame(&mut l, 3_000, 1, turn("two.test", "200:b", 600));
        let list = l.ice_servers(4_000);
        let users: Vec<_> = list.iter().map(|s| s.username.as_deref()).collect();
        assert_eq!(users, vec![Some("100:a"), Some("200:b")]);
        // The CreateAttempt action carries it and prints no credential.
        let act = LinkAction::CreateAttempt {
            attempt: 1,
            offerer: true,
            ice_servers: list,
        };
        let shown = format!("{act:?}");
        assert!(
            !shown.contains("S3CRET") && !shown.contains("100:a"),
            "{shown}"
        );
    }

    #[test]
    fn a_foreign_host_set_is_dropped_whole_and_an_expired_set_is_not_attached() {
        let (mut l, _) = started(Role::Baby);
        join_all(&mut l, 1_000, &[0]);
        let mut f = turn("relay.test", "100:a", 600);
        f["urls"] = json!(["turn:relay.test:3478", "turn:other.test:3478"]);
        frame(&mut l, 1_000, 0, f);
        let list = l.ice_servers(1_001);
        assert_eq!(list.len(), 1);
        assert!(list[0].credential.is_none(), "fallback only");
        // A 60 s set received at 10 s is offered until 40 s (receipt + ttl - 30 s).
        frame(&mut l, 10_000, 0, turn("relay.test", "100:a", 60));
        assert!(l.ice_servers(40_000)[0].credential.is_some());
        let late = l.ice_servers(40_001);
        assert!(late.iter().all(|s| s.credential.is_none()));
        assert_eq!(late[0].urls, vec!["stun:relay.test:3478"]);
    }

    #[test]
    fn a_set_survives_a_reconnect_and_ice_servers_before_the_welcome_are_ignored() {
        let (mut l, _) = started(Role::Baby);
        l.handle(1_000, LinkInput::MailboxOpened { id: 0 });
        frame(&mut l, 1_000, 0, turn("relay.test", "100:a", 600));
        assert!(
            l.ice_servers(1_001)[0].credential.is_none(),
            "not joined yet"
        );
        join_all(&mut l, 1_000, &[0]);
        frame(&mut l, 1_000, 0, turn("relay.test", "100:a", 600));
        l.handle(
            2_000,
            LinkInput::MailboxClosed {
                id: 0,
                reason: "x".into(),
            },
        );
        assert!(l.ice_servers(3_000)[0].credential.is_some());
    }

    #[test]
    fn malformed_server_input_never_changes_state() {
        let (mut l, _) = started(Role::Parent);
        l.handle(1, LinkInput::MailboxOpened { id: 0 });
        for junk in ["", "{}", "[]", "null", "{\"type\":\"members\"}", "\u{0}"] {
            let a = l.handle(
                2,
                LinkInput::MailboxFrame {
                    id: 0,
                    bytes: junk.into(),
                },
            );
            assert!(
                a.iter()
                    .all(|x| !matches!(x, LinkAction::CreateAttempt { .. }))
            );
        }
        assert_eq!(l.mb.relays[0].state, MailboxState::Open);
    }

    #[test]
    fn the_parent_secret_in_the_config_is_validated_redacted_and_taken() {
        let secret = ParentSecret {
            bytes: (0..32).collect(),
        };
        let mut c = cfg(Role::Baby);
        c.pinned_parent = Some(secret.clone());
        let text = format!("{c:?}");
        assert!(text.contains("pinned_parent"), "{text}");
        assert!(!text.contains("0001"), "{text}");
        // JSON form: 64 hex characters; absent and null mean no pin.
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["pinned_parent"].as_str().map(str::len), Some(64));
        assert_eq!(serde_json::from_value::<LinkConfig>(v).unwrap(), c);
        let mut none = serde_json::to_value(cfg(Role::Baby)).unwrap();
        none.as_object_mut().unwrap().remove("pinned_parent");
        assert_eq!(
            serde_json::from_value::<LinkConfig>(none)
                .unwrap()
                .pinned_parent,
            None
        );
        // A secret of the wrong length is a configuration error.
        c.pinned_parent = Some(ParentSecret { bytes: vec![1; 31] });
        assert_eq!(Link::new(c).err(), Some(ConfigError::InvalidParentSecret));
        // The core keeps the only copy: the config it holds no longer carries the secret.
        let mut c = cfg(Role::Baby);
        c.pinned_parent = Some(secret);
        let l = Link::new(c).unwrap();
        assert!(l.cfg.pinned_parent.is_none());
        assert!(l.pin.is_some() && l.sess.has_parent_pin());
    }

    #[test]
    fn a_pin_survives_stop_and_start_of_the_same_link() {
        let mut c = cfg(Role::Baby);
        c.pinned_parent = Some(ParentSecret { bytes: vec![5; 32] });
        let mut l = Link::new(c).unwrap();
        l.handle(0, LinkInput::Start { entropy: 1 });
        l.handle(1, LinkInput::Stop);
        l.handle(2, LinkInput::Start { entropy: 2 });
        assert!(l.pin.is_some() && l.sess.has_parent_pin());
    }

    #[test]
    fn new_keys_are_listed_for_the_catalogue() {
        let keys = message_keys();
        for k in [
            "unknown_parent_attempt",
            "clock_skew",
            "recovery_path_unavailable",
        ] {
            assert!(keys.contains(&k), "{k}");
        }
    }

    #[test]
    fn new_events_round_trip_and_only_parent_pinned_carries_a_secret() {
        let events = vec![
            LinkEvent::ParentPinned {
                secret: ParentSecret { bytes: vec![7; 32] },
                window_until_s: Some(1_800_000_300),
            },
            LinkEvent::ParentConnected {
                name: "Pixel 8".into(),
            },
            LinkEvent::MailboxesUpdated {
                mailboxes: vec![InviteMailbox {
                    kind: MailboxKind::Relay,
                    url: "wss://relay.example".into(),
                }],
            },
            LinkEvent::Hint {
                key: "clock_skew".into(),
                active: true,
            },
        ];
        for e in events {
            let v = serde_json::to_value(&e).unwrap();
            assert_eq!(serde_json::from_value::<LinkEvent>(v.clone()).unwrap(), e);
            let has_secret = v.to_string().contains(&"07".repeat(32));
            assert_eq!(has_secret, matches!(e, LinkEvent::ParentPinned { .. }));
        }
        let debug = format!(
            "{:?}",
            LinkEvent::ParentPinned {
                secret: ParentSecret { bytes: vec![7; 32] },
                window_until_s: None,
            }
        );
        assert!(!debug.contains("0707") && !debug.contains("[7"), "{debug}");
    }
}
