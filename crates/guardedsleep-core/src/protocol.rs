//! Wire format shared with the web client (TypeScript types are generated from this module). Parsers mirror the retired TS validators
//! so both implementations accept and reject the same inputs (see `vectors/conformance.json`).

// Untrusted input is parsed here: return errors instead of panicking (AGENTS.md §5.3).
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

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::invite::InviteMailbox;
use crate::pairing::ParentSecret;

/// Wire protocol version (ADR-017 §1). v2 and v3 peers never mix.
pub const PROTOCOL_VERSION: u32 = 3;
/// Only used by the v2 characterization parser and the `pair_mac` v2 vectors.
pub const LEGACY_PROTOCOL_VERSION: u32 = 2;
const MAX_ID_LEN: usize = 128;
/// Most candidates one `ice` message may carry (an 8 KiB envelope holds about 30 of them).
pub const MAX_BUNDLED_CANDIDATES: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub enum Role {
    Baby,
    Parent,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Baby => "baby",
            Role::Parent => "parent",
        }
    }

    fn parse(v: &Value) -> Option<Role> {
        match v.as_str()? {
            "baby" => Some(Role::Baby),
            "parent" => Some(Role::Parent),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// `ice_servers`: the optional `iceServers` field (STUN URLs, unvalidated; `ice.rs` binds them
    /// to the mailbox host). A field of the wrong shape is `None`: it never costs the `welcome`.
    Welcome {
        client_id: String,
        room_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        ice_servers: Option<Vec<String>>,
    },
    /// TURN (and STUN) URLs with a short-lived credential; only the shape is checked here.
    IceServers(IceServersFrame),
    Peer {
        peer_id: Option<String>,
    },
    Members {
        peers: Vec<Member>,
    },
    Signal {
        from: String,
        payload: Value,
    },
    Error {
        code: String,
        message: String,
    },
    Pong,
}

/// The relay's `ice_servers` message. Written by hand for `Debug` so the credential never prints.
#[derive(Clone, PartialEq, Serialize)]
pub struct IceServersFrame {
    pub urls: Vec<String>,
    pub username: String,
    pub credential: String,
    pub ttl: u64,
}

impl std::fmt::Debug for IceServersFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IceServersFrame")
            .field("urls", &self.urls)
            .field("username", &"<redacted>")
            .field("credential", &"<redacted>")
            .field("ttl", &self.ttl)
            .finish()
    }
}

fn string_array(v: &Value) -> Option<Vec<String>> {
    v.as_array()?
        .iter()
        .map(|x| x.as_str().map(str::to_owned))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Member {
    pub client_id: String,
    pub role: Role,
}

const ERROR_CODES: &[&str] = &[
    "BAD_JSON",
    "UNSUPPORTED_PROTOCOL",
    "INVALID_MESSAGE",
    "ROOM_FULL",
    "NOT_JOINED",
    "BABY_TAKEN",
    "PARENT_CAP",
    "SIGNAL_TARGET_REQUIRED",
    "RATE_LIMITED",
    "ADMISSION_REQUIRED",
    "ADMISSION_FAILED",
];

fn object(raw: &str) -> Option<Map<String, Value>> {
    match serde_json::from_str::<Value>(raw).ok()? {
        Value::Object(m) => Some(m),
        _ => None,
    }
}

/// Untrusted input from the signaling server; `None` for anything malformed.
pub fn parse_server_message(raw: &str) -> Option<ServerMessage> {
    let o = object(raw)?;
    match o.get("type")?.as_str()? {
        "pong" => Some(ServerMessage::Pong),
        "welcome" => Some(ServerMessage::Welcome {
            client_id: o.get("clientId")?.as_str()?.to_owned(),
            room_id: o.get("roomId")?.as_str()?.to_owned(),
            ice_servers: o.get("iceServers").and_then(string_array),
        }),
        "ice_servers" => Some(ServerMessage::IceServers(IceServersFrame {
            urls: string_array(o.get("urls")?)?,
            username: o.get("username")?.as_str()?.to_owned(),
            credential: o.get("credential")?.as_str()?.to_owned(),
            ttl: o.get("ttl")?.as_u64()?,
        })),
        "peer" => match o.get("peerId")? {
            Value::Null => Some(ServerMessage::Peer { peer_id: None }),
            Value::String(s) => Some(ServerMessage::Peer {
                peer_id: Some(s.clone()),
            }),
            _ => None,
        },
        "members" => {
            let mut peers = Vec::new();
            for item in o.get("peers")?.as_array()? {
                let rec = item.as_object()?;
                peers.push(Member {
                    client_id: rec.get("clientId")?.as_str()?.to_owned(),
                    role: Role::parse(rec.get("role")?)?,
                });
            }
            Some(ServerMessage::Members { peers })
        }
        "signal" => Some(ServerMessage::Signal {
            from: o.get("from")?.as_str()?.to_owned(),
            payload: o.get("payload")?.clone(),
        }),
        "error" => {
            let code = o.get("code")?.as_str()?;
            if !ERROR_CODES.contains(&code) {
                return None;
            }
            Some(ServerMessage::Error {
                code: code.to_owned(),
                message: o.get("message")?.as_str()?.to_owned(),
            })
        }
        _ => None,
    }
}

pub fn encode_hello(role: Role, room_id: &str) -> String {
    json!({ "type": "hello", "role": role.as_str(), "roomId": room_id, "protocolVersion": PROTOCOL_VERSION })
        .to_string()
}

pub fn encode_signal(room_id: &str, to: Option<&str>, payload: Value) -> String {
    let mut m = Map::new();
    m.insert("type".into(), "signal".into());
    m.insert("roomId".into(), room_id.into());
    if let Some(to) = to.filter(|t| !t.is_empty()) {
        m.insert("to".into(), to.into());
    }
    m.insert("payload".into(), payload);
    Value::Object(m).to_string()
}

pub fn valid_room_id(room_id: &str) -> bool {
    !room_id.is_empty() && room_id.len() <= MAX_ID_LEN
}

pub fn encode_ping() -> String {
    json!({ "type": "ping" }).to_string()
}

/// Attempt id: 128 random bits as 32 lowercase hex characters. It filters stale candidates and
/// offers; it is never a trust decision (ADR-017 §3).
pub fn valid_aid(aid: &str) -> bool {
    aid.len() == 32 && aid.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Payload carried inside `signal` messages. `aid` identifies the sender's attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalPayload {
    Sdp {
        aid: String,
        sdp_type: String,
        sdp: String,
        /// Offers only: the `request_offer` id this offer answers, so the parent can adopt it as
        /// its own attempt id and the baby can reject answers of superseded attempts.
        re: Option<String>,
    },
    Ice {
        aid: String,
        candidate: IceCandidate,
    },
    /// Several candidates in one message (`{"type":"ice","aid":..,"candidates":[..]}`): the shape
    /// `Link` sends on every mailbox, because on a public relay every message is an event with a
    /// budget (`signaling-mailboxes`, "Event budget on public relays"). The single-candidate
    /// shape is still accepted.
    IceBundle {
        aid: String,
        candidates: Vec<IceCandidate>,
    },
    /// Parent to baby: "offer me a fresh attempt"; `aid` is the parent's id for that attempt.
    RequestOffer { aid: String },
    /// Deliberate stop of the sending link (ADR-017 §7). It carries nothing: the sealed envelope
    /// authenticates it and binds it to the receiver's session and the sender's session
    /// (`signaling-mailboxes`, "Session-bound freshness"), which replaced the old stop MAC.
    StopNotice,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct IceCandidate {
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_m_line_index: Option<u32>,
}

pub fn parse_signal_payload(v: &Value) -> Option<SignalPayload> {
    let o = v.as_object()?;
    let aid = || {
        let a = o.get("aid")?.as_str()?;
        valid_aid(a).then(|| a.to_owned())
    };
    match o.get("type")?.as_str()? {
        "sdp" => {
            let sdp = o.get("sdp")?.as_object()?;
            let sdp_type = sdp.get("type")?.as_str()?;
            if sdp_type != "offer" && sdp_type != "answer" {
                return None;
            }
            let re = match o.get("re") {
                None => None,
                Some(v) => {
                    let r = v.as_str()?;
                    if !valid_aid(r) {
                        return None;
                    }
                    Some(r.to_owned())
                }
            };
            Some(SignalPayload::Sdp {
                aid: aid()?,
                sdp_type: sdp_type.to_owned(),
                sdp: sdp.get("sdp")?.as_str()?.to_owned(),
                re,
            })
        }
        "ice" => {
            let parse_candidate = |c: &Value| -> Option<IceCandidate> {
                let c = c.as_object()?;
                Some(IceCandidate {
                    candidate: c.get("candidate")?.as_str()?.to_owned(),
                    sdp_mid: c.get("sdpMid").and_then(Value::as_str).map(str::to_owned),
                    sdp_m_line_index: c
                        .get("sdpMLineIndex")
                        .and_then(Value::as_u64)
                        .and_then(|n| u32::try_from(n).ok()),
                })
            };
            if let Some(list) = o.get("candidates") {
                let list = list.as_array()?;
                if list.is_empty() || list.len() > MAX_BUNDLED_CANDIDATES {
                    return None;
                }
                return Some(SignalPayload::IceBundle {
                    aid: aid()?,
                    candidates: list
                        .iter()
                        .map(parse_candidate)
                        .collect::<Option<Vec<_>>>()?,
                });
            }
            Some(SignalPayload::Ice {
                aid: aid()?,
                candidate: parse_candidate(o.get("candidate")?)?,
            })
        }
        "request_offer" => Some(SignalPayload::RequestOffer { aid: aid()? }),
        "stop_notice" => Some(SignalPayload::StopNotice),
        _ => None,
    }
}

fn candidate_json(candidate: &IceCandidate) -> Value {
    let mut c = Map::new();
    c.insert("candidate".into(), candidate.candidate.clone().into());
    c.insert(
        "sdpMid".into(),
        candidate
            .sdp_mid
            .clone()
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    c.insert(
        "sdpMLineIndex".into(),
        candidate
            .sdp_m_line_index
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    Value::Object(c)
}

pub fn signal_payload_json(p: &SignalPayload) -> Value {
    match p {
        SignalPayload::Sdp {
            aid,
            sdp_type,
            sdp,
            re,
        } => {
            let mut o = Map::new();
            o.insert("type".into(), "sdp".into());
            o.insert("aid".into(), aid.clone().into());
            if let Some(re) = re {
                o.insert("re".into(), re.clone().into());
            }
            o.insert("sdp".into(), json!({ "type": sdp_type, "sdp": sdp }));
            Value::Object(o)
        }
        SignalPayload::Ice { aid, candidate } => {
            json!({ "type": "ice", "aid": aid, "candidate": candidate_json(candidate) })
        }
        SignalPayload::IceBundle { aid, candidates } => json!({
            "type": "ice",
            "aid": aid,
            "candidates": candidates.iter().map(candidate_json).collect::<Vec<_>>(),
        }),
        SignalPayload::RequestOffer { aid } => json!({ "type": "request_offer", "aid": aid }),
        SignalPayload::StopNotice => json!({ "type": "stop_notice" }),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub enum Ringer {
    Normal,
    Vibrate,
    Silent,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub enum Dnd {
    Off,
    AlarmsOnly,
    Priority,
    Total,
    Unknown,
}

pub const MAX_NEXT_ALARM_MIN: u32 = 1440;
/// `next_alarm_min` is coarsened to this step (ADR-018 privacy: no exact alarm time).
pub const NEXT_ALARM_STEP_MIN: u32 = 15;

fn coarsen_next_alarm(min: u32) -> u32 {
    (min.min(MAX_NEXT_ALARM_MIN) + NEXT_ALARM_STEP_MIN / 2) / NEXT_ALARM_STEP_MIN
        * NEXT_ALARM_STEP_MIN
}

/// Why a baby unit may stop monitoring reliably (mainly a web/laptop baby). Variant order is the
/// stable wire order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub enum UnitWarning {
    PageHidden,
    WakeLockOff,
    ResumedFromSleep,
    NotCharging,
    MicPermissionLost,
}

/// Maximum entries accepted in `unit_warnings` on the wire.
pub const MAX_UNIT_WARNINGS: usize = 8;

/// Whether the baby unit lets this parent talk (`device_status.talk`, `talk-back` "Honest
/// playability").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub enum TalkStatus {
    On,
    /// Switched off for this pairing on the baby unit.
    Off,
    /// The baby unit would not make a talk audible right now.
    Muted,
    /// This parent's answer has no talk line.
    Unsupported,
}

/// Who talks to the child right now, from the recipient's point of view
/// (`device_status.talker`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub enum Talker {
    None,
    /// The recipient itself.
    You,
    /// Another parent device.
    Other,
}

/// `device_status.talk_audible`: the baby unit's own microphone could not hear three talks in a
/// row, so the voice may not be audible there. A warning only, never a denial.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub enum TalkAudible {
    Ok,
    Uncertain,
}

/// The baby unit's playback check of one talk (`talk_playback`, `talk-back` "Playback check").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub enum PlaybackResult {
    Heard,
    Unconfirmed,
    /// The check could not decide; it counts for nothing and shows no warning.
    Unknown,
}

/// Why the baby unit denied a `talk_request`. The retry hints are milliseconds from the denial.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub enum TalkDenyReason {
    Off,
    Unsupported,
    Muted,
    Busy,
    Cooldown { retry_in_ms: u64 },
    Budget { retry_in_ms: u64 },
    Rate,
}

/// Largest `talk_retry_in_ms` and `retry_in_ms` accepted on the wire (one hour).
pub const MAX_TALK_RETRY_MS: u64 = 3_600_000;

/// Coarse baby-phone state shown to the parent (ADR-018). Every field may be unknown (`None`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct DeviceStatus {
    /// Rounded to 5 by [`DeviceStatus::normalized`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battery_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charging: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ringer: Option<Ringer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dnd: Option<Dnd>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alarms_allowed: Option<bool>,
    /// The app may control Do Not Disturb.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dnd_access: Option<bool>,
    /// Our own silence rule is active.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quiet_active: Option<bool>,
    /// Minutes until the next alarm clock, rounded to multiples of 15 and capped at
    /// [`MAX_NEXT_ALARM_MIN`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_alarm_min: Option<u32>,
    /// The microphone delivered only digital silence for 30 s (set by the core, not the platform).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mic_silent: Option<bool>,
    /// The baby's live noise-gate threshold (0.01..=1.0), set by the core.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub noise_threshold: Option<f64>,
    /// Reliability warnings reported by the baby platform; sorted and deduplicated by
    /// [`DeviceStatus::normalized`] and the parser, omitted on the wire when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unit_warnings: Vec<UnitWarning>,
    /// Talk-back as this recipient sees it (set by the core of the baby, never by the platform).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub talk: Option<TalkStatus>,
    /// Who talks right now, from this recipient's point of view (set by the core).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub talker: Option<Talker>,
    /// Noise detection is paused while a talk plays (set by the core; omitted when not paused).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub noise_paused: Option<bool>,
    /// While a talk is denied for the cooldown or the budget: milliseconds until the next request
    /// can pass, counted from when this status was sent (set by the core).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub talk_retry_in_ms: Option<u64>,
    /// `uncertain` after three talks in a row that the playback check could not confirm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub talk_audible: Option<TalkAudible>,
}

fn sort_dedup_warnings(w: &mut Vec<UnitWarning>) {
    w.sort();
    w.dedup();
}

impl DeviceStatus {
    /// Coarsens the battery to steps of 5 and the alarm horizon to steps of 15 (capped), as sent
    /// on the wire.
    pub fn normalized(&self) -> DeviceStatus {
        let mut s = self.clone();
        s.battery_pct = s.battery_pct.map(|p| {
            let p = u32::from(p.min(100));
            ((p + 2) / 5 * 5) as u8
        });
        s.next_alarm_min = s.next_alarm_min.map(coarsen_next_alarm);
        sort_dedup_warnings(&mut s.unit_warnings);
        s
    }
}

fn opt_bool(o: &Map<String, Value>, key: &str) -> Result<Option<bool>, ()> {
    match o.get(key) {
        None => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(()),
    }
}

fn as_uint(v: &Value) -> Option<u64> {
    if let Some(u) = v.as_u64() {
        return Some(u);
    }
    let f = v.as_f64()?;
    (f >= 0.0 && f.fract() == 0.0 && f < 9.0e15).then_some(f as u64)
}

fn opt_uint(o: &Map<String, Value>, key: &str, max: u64) -> Result<Option<u64>, ()> {
    match o.get(key) {
        None => Ok(None),
        Some(v) => match as_uint(v) {
            Some(n) if n <= max => Ok(Some(n)),
            _ => Err(()),
        },
    }
}

/// A string-valued optional field with a fixed vocabulary: absent or an unknown word is `None` (a
/// newer peer may add words), a value that is not a string is an error.
fn opt_word<T>(
    o: &Map<String, Value>,
    key: &str,
    word: fn(&str) -> Option<T>,
) -> Result<Option<T>, ()> {
    match o.get(key) {
        None => Ok(None),
        Some(v) => v.as_str().map(word).ok_or(()),
    }
}

fn talk_status_word(w: &str) -> Option<TalkStatus> {
    match w {
        "on" => Some(TalkStatus::On),
        "off" => Some(TalkStatus::Off),
        "muted" => Some(TalkStatus::Muted),
        "unsupported" => Some(TalkStatus::Unsupported),
        _ => None,
    }
}

fn talker_word(w: &str) -> Option<Talker> {
    match w {
        "none" => Some(Talker::None),
        "you" => Some(Talker::You),
        "other" => Some(Talker::Other),
        _ => None,
    }
}

fn talk_audible_word(w: &str) -> Option<TalkAudible> {
    match w {
        "ok" => Some(TalkAudible::Ok),
        "uncertain" => Some(TalkAudible::Uncertain),
        _ => None,
    }
}

fn playback_word(w: &str) -> Option<PlaybackResult> {
    match w {
        "heard" => Some(PlaybackResult::Heard),
        "unconfirmed" => Some(PlaybackResult::Unconfirmed),
        "unknown" => Some(PlaybackResult::Unknown),
        _ => None,
    }
}

/// The reason of a `talk_deny`: an unknown reason, or a cooldown or budget without its retry hint,
/// is `None` (the denial still counts, only its cause is unknown).
fn talk_deny_reason(o: &Map<String, Value>) -> Option<TalkDenyReason> {
    let retry = || as_uint(o.get("retry_in_ms")?).filter(|n| *n <= MAX_TALK_RETRY_MS);
    match o.get("reason")?.as_str()? {
        "off" => Some(TalkDenyReason::Off),
        "unsupported" => Some(TalkDenyReason::Unsupported),
        "muted" => Some(TalkDenyReason::Muted),
        "busy" => Some(TalkDenyReason::Busy),
        "cooldown" => retry().map(|retry_in_ms| TalkDenyReason::Cooldown { retry_in_ms }),
        "budget" => retry().map(|retry_in_ms| TalkDenyReason::Budget { retry_in_ms }),
        "rate" => Some(TalkDenyReason::Rate),
        _ => None,
    }
}

fn parse_device_status(o: &Map<String, Value>) -> Option<DeviceStatus> {
    let ringer = match o.get("ringer") {
        None => None,
        Some(v) => Some(match v.as_str()? {
            "normal" => Ringer::Normal,
            "vibrate" => Ringer::Vibrate,
            "silent" => Ringer::Silent,
            "unknown" => Ringer::Unknown,
            _ => return None,
        }),
    };
    let dnd = match o.get("dnd") {
        None => None,
        Some(v) => Some(match v.as_str()? {
            "off" => Dnd::Off,
            "alarms_only" => Dnd::AlarmsOnly,
            "priority" => Dnd::Priority,
            "total" => Dnd::Total,
            "unknown" => Dnd::Unknown,
            _ => return None,
        }),
    };
    let mut unit_warnings = match o.get("unit_warnings") {
        None => Vec::new(),
        Some(v) => {
            let arr = v.as_array().filter(|a| a.len() <= MAX_UNIT_WARNINGS)?;
            arr.iter()
                .map(|w| {
                    Some(match w.as_str()? {
                        "page_hidden" => UnitWarning::PageHidden,
                        "wake_lock_off" => UnitWarning::WakeLockOff,
                        "resumed_from_sleep" => UnitWarning::ResumedFromSleep,
                        "not_charging" => UnitWarning::NotCharging,
                        "mic_permission_lost" => UnitWarning::MicPermissionLost,
                        _ => return None,
                    })
                })
                .collect::<Option<Vec<_>>>()?
        }
    };
    sort_dedup_warnings(&mut unit_warnings);
    Some(DeviceStatus {
        battery_pct: opt_uint(o, "battery_pct", 100).ok()?.map(|n| n as u8),
        charging: opt_bool(o, "charging").ok()?,
        ringer,
        dnd,
        alarms_allowed: opt_bool(o, "alarms_allowed").ok()?,
        dnd_access: opt_bool(o, "dnd_access").ok()?,
        quiet_active: opt_bool(o, "quiet_active").ok()?,
        next_alarm_min: opt_uint(o, "next_alarm_min", u64::from(MAX_NEXT_ALARM_MIN))
            .ok()?
            .map(|n| coarsen_next_alarm(n as u32)),
        mic_silent: opt_bool(o, "mic_silent").ok()?,
        noise_threshold: match o.get("noise_threshold") {
            None => None,
            Some(v) => Some(v.as_f64().filter(|t| (0.01..=1.0).contains(t))?),
        },
        unit_warnings,
        talk: opt_word(o, "talk", talk_status_word).ok()?,
        talker: opt_word(o, "talker", talker_word).ok()?,
        noise_paused: opt_bool(o, "noise_paused").ok()?,
        talk_retry_in_ms: opt_uint(o, "talk_retry_in_ms", MAX_TALK_RETRY_MS).ok()?,
        talk_audible: opt_word(o, "talk_audible", talk_audible_word).ok()?,
    })
}

/// Application messages on the DataChannel.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DataMessage {
    Heartbeat {
        t: f64,
        seq: u64,
        battery: Option<f64>,
        rms: Option<f64>,
    },
    Listen {
        enabled: bool,
    },
    NoiseAlert {
        rms: f64,
        t: f64,
        probe: bool,
    },
    ProbeTest {
        t: f64,
    },
    PairProof {
        mac: String,
    },
    OpenSettings,
    /// Baby to parent (v3), flattened on the wire.
    DeviceStatus(DeviceStatus),
    /// Baby to parent after a verification that follows an outage (v3).
    MissedEvents {
        count: u32,
        max_rms: f64,
        complete: bool,
        down_ms: u64,
    },
    /// Parent to baby: switch the baby phone to silent (v3). There is no remote "off".
    QuietRequest,
    /// The sender stops monitoring on purpose (v3).
    StopNotice,
    /// Parent to baby, once, at the first verification: the parent secret and the parent device's
    /// name. Only ever sent over a verified DataChannel (`alpha-connection-model` design section 4).
    ParentPin {
        secret: ParentSecret,
        name: String,
    },
    /// Baby to parent: the secret was pinned.
    PinAck,
    /// Parent to baby after every later verification: the proof of the parent secret, bound to both
    /// DTLS fingerprints (`pairing::parent_proof_mac`), and the device's current name.
    ParentProof {
        mac: String,
        name: String,
    },
    /// Baby to a pinned parent, right after the parent proof checked out: the baby's own proof of
    /// the secret, bound to both DTLS fingerprints (`pairing::baby_proof_mac`).
    BabyProof {
        mac: String,
    },
    /// Baby to parent after verification: the mailbox set the parent stores for later recoveries.
    Mailboxes {
        mailboxes: Vec<InviteMailbox>,
    },
    /// Parent to baby: the parent holds the talk button. `id` is the parent's own counter; every
    /// other talk message names it, and a message with another id than the current talk is stale.
    TalkRequest {
        id: u64,
    },
    /// Baby to parent: the talk may start; the baby ends it itself `max_ms` after this message.
    TalkGrant {
        id: u64,
        max_ms: u64,
    },
    /// Baby to parent: no talk. An unknown `reason` parses as `None`.
    TalkDeny {
        id: u64,
        reason: Option<TalkDenyReason>,
    },
    /// Either side ends the talk `id` (the parent released, the baby's deadline or switch).
    TalkEnd {
        id: u64,
    },
    /// Baby to parent, once per talk: the result of the playback check.
    TalkPlayback {
        id: u64,
        result: PlaybackResult,
    },
}

/// Longest device name the baby shows, in characters.
pub const MAX_DEVICE_NAME_CHARS: usize = 64;

/// A device name as the baby may show it: control (Cc), format (Cf, which includes zero-width and
/// bidirectional override characters) and line and paragraph separator (Zl, Zp) characters removed,
/// trimmed, at most [`MAX_DEVICE_NAME_CHARS`] characters.
pub fn clean_device_name(raw: &str) -> String {
    let cleaned: String = raw.chars().filter(|c| !is_unwanted_in_name(*c)).collect();
    cleaned.trim().chars().take(MAX_DEVICE_NAME_CHARS).collect()
}

/// Unicode general categories Cc, Cf, Zl and Zp. `char` has no category lookup in `std`, so the
/// Cf and separator code points are listed (Unicode 16; new Cf characters are rare).
fn is_unwanted_in_name(c: char) -> bool {
    if c.is_control() {
        return true;
    }
    matches!(
        u32::from(c),
        0x00AD
            | 0x0600..=0x0605
            | 0x061C
            | 0x06DD
            | 0x070F
            | 0x0890..=0x0891
            | 0x08E2
            | 0x180E
            | 0x200B..=0x200F
            | 0x2028..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFEFF
            | 0xFFF9..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0001
            | 0xE0020..=0xE007F
    )
}

fn versioned(o: &Map<String, Value>, version: u32) -> bool {
    o.get("v").and_then(Value::as_f64) == Some(f64::from(version))
}

fn unit_interval(o: &Map<String, Value>, key: &str) -> Result<Option<f64>, ()> {
    match o.get(key) {
        None => Ok(None),
        Some(v) => match v.as_f64() {
            Some(n) if (0.0..=1.0).contains(&n) => Ok(Some(n)),
            _ => Err(()),
        },
    }
}

/// Untrusted input from the remote peer (protocol v3); `None` for unknown or malformed messages.
pub fn parse_data_message(raw: &str) -> Option<DataMessage> {
    parse_data_versioned(raw, PROTOCOL_VERSION)
}

/// v2 characterization parser: only `v: 2` and only the message kinds v2 had. It exists so the
/// golden `parse_data` vectors keep agreeing with the TypeScript v2 client.
pub fn parse_data_message_v2_legacy(raw: &str) -> Option<DataMessage> {
    parse_data_versioned(raw, LEGACY_PROTOCOL_VERSION)
}

fn parse_data_versioned(raw: &str, version: u32) -> Option<DataMessage> {
    let o = object(raw)?;
    let kind = o.get("type")?.as_str()?;
    if !versioned(&o, version) {
        return None;
    }
    let v3 = version == PROTOCOL_VERSION;
    match kind {
        "heartbeat" => Some(DataMessage::Heartbeat {
            t: o.get("t")?.as_f64()?,
            seq: o.get("seq")?.as_f64().filter(|n| *n >= 0.0)? as u64,
            battery: unit_interval(&o, "battery").ok()?,
            rms: unit_interval(&o, "rms").ok()?,
        }),
        "listen" => Some(DataMessage::Listen {
            enabled: o.get("enabled")?.as_bool()?,
        }),
        "noise_alert" => Some(DataMessage::NoiseAlert {
            rms: o.get("rms")?.as_f64()?.clamp(0.0, 1.0),
            t: o.get("t")?.as_f64()?,
            probe: o.get("probe") == Some(&Value::Bool(true)),
        }),
        "probe_test" => Some(DataMessage::ProbeTest {
            t: o.get("t")?.as_f64()?,
        }),
        "pair_proof" => {
            let mac = o.get("mac")?.as_str()?;
            if mac.len() != 64 || !mac.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            Some(DataMessage::PairProof {
                mac: mac.to_ascii_lowercase(),
            })
        }
        "open_settings" => Some(DataMessage::OpenSettings),
        "device_status" if v3 => parse_device_status(&o).map(DataMessage::DeviceStatus),
        "missed_events" if v3 => Some(DataMessage::MissedEvents {
            count: u32::try_from(as_uint(o.get("count")?)?).ok()?,
            max_rms: unit_interval(&o, "max_rms").ok()??,
            complete: o.get("complete")?.as_bool()?,
            down_ms: as_uint(o.get("down_ms")?)?,
        }),
        "quiet_request" if v3 => Some(DataMessage::QuietRequest),
        "stop_notice" if v3 => Some(DataMessage::StopNotice),
        "parent_pin" if v3 => Some(DataMessage::ParentPin {
            secret: serde_json::from_value(o.get("secret")?.clone()).ok()?,
            name: device_name_field(&o),
        }),
        "pin_ack" if v3 => Some(DataMessage::PinAck),
        "baby_proof" if v3 => {
            let mac = o.get("mac")?.as_str()?;
            if mac.len() != 64 || !mac.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            Some(DataMessage::BabyProof {
                mac: mac.to_ascii_lowercase(),
            })
        }
        "parent_proof" if v3 => {
            let mac = o.get("mac")?.as_str()?;
            if mac.len() != 64 || !mac.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            Some(DataMessage::ParentProof {
                mac: mac.to_ascii_lowercase(),
                name: device_name_field(&o),
            })
        }
        "talk_request" if v3 => Some(DataMessage::TalkRequest {
            id: as_uint(o.get("id")?)?,
        }),
        "talk_grant" if v3 => Some(DataMessage::TalkGrant {
            id: as_uint(o.get("id")?)?,
            max_ms: as_uint(o.get("max_ms")?)?,
        }),
        "talk_deny" if v3 => Some(DataMessage::TalkDeny {
            id: as_uint(o.get("id")?)?,
            reason: talk_deny_reason(&o),
        }),
        "talk_end" if v3 => Some(DataMessage::TalkEnd {
            id: as_uint(o.get("id")?)?,
        }),
        "talk_playback" if v3 => Some(DataMessage::TalkPlayback {
            id: as_uint(o.get("id")?)?,
            result: playback_word(o.get("result")?.as_str()?)?,
        }),
        "mailboxes" if v3 => {
            let list = o.get("mailboxes")?.as_array()?;
            if list.is_empty() || list.len() > crate::invite::MAX_MAILBOXES {
                return None;
            }
            let mailboxes = list
                .iter()
                .map(|m| serde_json::from_value::<InviteMailbox>(m.clone()).ok())
                .collect::<Option<Vec<_>>>()?;
            Some(DataMessage::Mailboxes { mailboxes })
        }
        _ => None,
    }
}

/// The optional `name` of a parent message, cleaned; absent or not a string means no name.
fn device_name_field(o: &Map<String, Value>) -> String {
    o.get("name")
        .and_then(Value::as_str)
        .map(clean_device_name)
        .unwrap_or_default()
}

pub fn encode_data_message(m: &DataMessage) -> String {
    let v = PROTOCOL_VERSION;
    match m {
        DataMessage::Heartbeat {
            t,
            seq,
            battery,
            rms,
        } => {
            let mut o = Map::new();
            o.insert("type".into(), "heartbeat".into());
            o.insert("v".into(), v.into());
            o.insert("t".into(), json_number(*t));
            o.insert("seq".into(), (*seq).into());
            if let Some(b) = battery {
                o.insert("battery".into(), json_number(*b));
            }
            if let Some(r) = rms {
                o.insert("rms".into(), json_number(*r));
            }
            Value::Object(o).to_string()
        }
        DataMessage::Listen { enabled } => {
            json!({ "type": "listen", "v": v, "enabled": enabled }).to_string()
        }
        DataMessage::NoiseAlert { rms, t, probe } => {
            let mut o = Map::new();
            o.insert("type".into(), "noise_alert".into());
            o.insert("v".into(), v.into());
            o.insert("rms".into(), json_number(*rms));
            o.insert("t".into(), json_number(*t));
            if *probe {
                o.insert("probe".into(), true.into());
            }
            Value::Object(o).to_string()
        }
        DataMessage::ProbeTest { t } => {
            json!({ "type": "probe_test", "v": v, "t": json_number(*t) }).to_string()
        }
        DataMessage::PairProof { mac } => {
            json!({ "type": "pair_proof", "v": v, "mac": mac }).to_string()
        }
        DataMessage::OpenSettings => json!({ "type": "open_settings", "v": v }).to_string(),
        DataMessage::DeviceStatus(st) => {
            let mut o = Map::new();
            o.insert("type".into(), "device_status".into());
            o.insert("v".into(), v.into());
            if let Ok(Value::Object(fields)) = serde_json::to_value(st) {
                o.extend(fields);
            }
            Value::Object(o).to_string()
        }
        DataMessage::MissedEvents {
            count,
            max_rms,
            complete,
            down_ms,
        } => json!({
            "type": "missed_events", "v": v, "count": count, "max_rms": json_number(*max_rms),
            "complete": complete, "down_ms": down_ms
        })
        .to_string(),
        DataMessage::QuietRequest => json!({ "type": "quiet_request", "v": v }).to_string(),
        DataMessage::StopNotice => json!({ "type": "stop_notice", "v": v }).to_string(),
        DataMessage::ParentPin { secret, name } => json!({
            "type": "parent_pin", "v": v, "secret": secret, "name": clean_device_name(name)
        })
        .to_string(),
        DataMessage::PinAck => json!({ "type": "pin_ack", "v": v }).to_string(),
        DataMessage::BabyProof { mac } => {
            json!({ "type": "baby_proof", "v": v, "mac": mac }).to_string()
        }
        DataMessage::ParentProof { mac, name } => json!({
            "type": "parent_proof", "v": v, "mac": mac, "name": clean_device_name(name)
        })
        .to_string(),
        DataMessage::Mailboxes { mailboxes } => {
            json!({ "type": "mailboxes", "v": v, "mailboxes": mailboxes }).to_string()
        }
        DataMessage::TalkRequest { id } => {
            json!({ "type": "talk_request", "v": v, "id": id }).to_string()
        }
        DataMessage::TalkGrant { id, max_ms } => {
            json!({ "type": "talk_grant", "v": v, "id": id, "max_ms": max_ms }).to_string()
        }
        DataMessage::TalkDeny { id, reason } => {
            let mut o = Map::new();
            o.insert("type".into(), "talk_deny".into());
            o.insert("v".into(), v.into());
            o.insert("id".into(), (*id).into());
            // `{ "kind": "cooldown", "retry_in_ms": n }` flattens into `reason` and `retry_in_ms`.
            if let Some(Value::Object(mut r)) = reason.and_then(|r| serde_json::to_value(r).ok()) {
                if let Some(kind) = r.remove("kind") {
                    o.insert("reason".into(), kind);
                }
                o.extend(r);
            }
            Value::Object(o).to_string()
        }
        DataMessage::TalkEnd { id } => json!({ "type": "talk_end", "v": v, "id": id }).to_string(),
        DataMessage::TalkPlayback { id, result } => {
            json!({ "type": "talk_playback", "v": v, "id": id, "result": result }).to_string()
        }
    }
}

/// Integral floats serialize without a fractional part, matching `JSON.stringify`.
fn json_number(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9.0e15 {
        Value::from(n as i64)
    } else {
        Value::from(n)
    }
}

/// Relay admission nonce (ADR-021 section 1): 128 random bits as 22 base64url characters.
pub fn valid_nonce(nonce: &str) -> bool {
    nonce.len() == 22
        && nonce
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// First message of a relay that requires admission: a fresh single-use nonce for this connection.
pub fn encode_challenge(nonce: &str) -> String {
    json!({ "type": "challenge", "nonce": nonce }).to_string()
}

/// The nonce of a `challenge`, `None` for anything else or a malformed nonce.
pub fn parse_challenge(raw: &str) -> Option<String> {
    let o = object(raw)?;
    if o.get("type")?.as_str()? != "challenge" {
        return None;
    }
    let nonce = o.get("nonce")?.as_str()?;
    valid_nonce(nonce).then(|| nonce.to_owned())
}

/// Joins a room on a relay that requires admission. It replaces `hello`: `mac` is
/// `admission_mac(key, nonce, role, room_id)` over the nonce of this connection's `challenge`.
pub fn encode_admit(role: Role, room_id: &str, mac: &str) -> String {
    json!({
        "type": "admit",
        "role": role.as_str(),
        "roomId": room_id,
        "protocolVersion": PROTOCOL_VERSION,
        "mac": mac,
    })
    .to_string()
}

/// A message from a client to the signaling relay. Mirrors `parseClientMessage` of the TypeScript
/// package, plus the admission `admit`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Ping,
    Hello {
        role: Role,
        room_id: String,
        protocol_version: u64,
    },
    Admit {
        role: Role,
        room_id: String,
        protocol_version: u64,
        /// 64 lowercase hex characters.
        mac: String,
    },
    Signal {
        room_id: String,
        to: Option<String>,
        payload: Value,
    },
}

fn protocol_version_of(v: &Value) -> Option<u64> {
    let n = v.as_number()?;
    n.as_u64().or_else(|| {
        n.as_f64()
            .filter(|f| f.fract() == 0.0 && (0.0..9.0e15).contains(f))
            .map(|f| f as u64)
    })
}

/// Untrusted input from a client; `None` for anything malformed.
pub fn parse_client_message(raw: &str) -> Option<ClientMessage> {
    let o = object(raw)?;
    match o.get("type")?.as_str()? {
        "ping" => Some(ClientMessage::Ping),
        t @ ("hello" | "admit") => {
            let role = Role::parse(o.get("role")?)?;
            let room_id = o.get("roomId")?.as_str()?;
            if !valid_room_id(room_id) {
                return None;
            }
            let protocol_version = protocol_version_of(o.get("protocolVersion")?)?;
            if t == "hello" {
                return Some(ClientMessage::Hello {
                    role,
                    room_id: room_id.to_owned(),
                    protocol_version,
                });
            }
            let mac = o.get("mac")?.as_str()?;
            let hex =
                mac.len() == 64 && mac.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
            hex.then(|| ClientMessage::Admit {
                role,
                room_id: room_id.to_owned(),
                protocol_version,
                mac: mac.to_owned(),
            })
        }
        "signal" => {
            let room_id = o.get("roomId")?.as_str()?;
            if !valid_room_id(room_id) {
                return None;
            }
            let payload = o.get("payload")?.clone();
            let to = match o.get("to") {
                None => None,
                Some(Value::String(t)) if valid_room_id(t) => Some(t.clone()),
                Some(_) => return None,
            };
            Some(ClientMessage::Signal {
                room_id: room_id.to_owned(),
                to,
                payload,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AID: &str = "00112233445566778899aabbccddeeff";

    #[test]
    fn server_messages_round_trip_from_ts_shapes() {
        assert_eq!(
            parse_server_message(r#"{"type":"peer","peerId":null}"#),
            Some(ServerMessage::Peer { peer_id: None })
        );
        assert_eq!(parse_server_message(r#"{"type":"peer","peerId":5}"#), None);
        assert_eq!(
            parse_server_message(r#"{"type":"error","code":"NOPE","message":"x"}"#),
            None
        );
        assert!(
            parse_server_message(r#"{"type":"error","code":"RATE_LIMITED","message":"x"}"#)
                .is_some()
        );
        assert_eq!(parse_server_message("not json"), None);
        assert_eq!(parse_server_message("[]"), None);
    }

    #[test]
    fn data_messages_require_protocol_version() {
        assert!(parse_data_message(r#"{"type":"listen","v":3,"enabled":true}"#).is_some());
        assert!(parse_data_message(r#"{"type":"listen","v":2,"enabled":true}"#).is_none());
        assert!(parse_data_message(r#"{"type":"listen","enabled":true}"#).is_none());
        assert!(
            parse_data_message(r#"{"type":"heartbeat","v":3,"t":1,"seq":1,"rms":1.5}"#).is_none()
        );
    }

    #[test]
    fn legacy_parser_is_v2_only_and_knows_only_v2_kinds() {
        assert!(
            parse_data_message_v2_legacy(r#"{"type":"listen","v":2,"enabled":true}"#).is_some()
        );
        assert!(
            parse_data_message_v2_legacy(r#"{"type":"listen","v":3,"enabled":true}"#).is_none()
        );
        assert!(parse_data_message_v2_legacy(r#"{"type":"stop_notice","v":2}"#).is_none());
        assert!(parse_data_message_v2_legacy(r#"{"type":"quiet_request","v":2}"#).is_none());
        assert!(parse_data_message(r#"{"type":"stop_notice","v":3}"#).is_some());
    }

    #[test]
    fn heartbeat_encoding_matches_json_stringify() {
        let s = encode_data_message(&DataMessage::Heartbeat {
            t: 1_700_000_000_000.0,
            seq: 3,
            battery: None,
            rms: Some(0.25),
        });
        assert_eq!(
            s,
            r#"{"type":"heartbeat","v":3,"t":1700000000000,"seq":3,"rms":0.25}"#
        );
    }

    #[test]
    fn device_names_lose_format_bidi_and_separator_characters() {
        // Bidi overrides and isolates, zero-width characters, line and paragraph separators, BOM.
        let hostile =
            "\u{202E}Pixel\u{200B} 8\u{2066}\u{2069}\u{200F}\u{2028}\u{2029}\u{FEFF}\u{202A}";
        assert_eq!(clean_device_name(hostile), "Pixel 8");
        assert_eq!(
            clean_device_name("\u{202A}\u{202B}\u{202C}\u{202D}\u{202E}"),
            ""
        );
        assert_eq!(clean_device_name("Zoë 日本"), "Zoë 日本");
    }

    #[test]
    fn device_names_lose_control_characters_and_are_cut_at_64_characters() {
        assert_eq!(clean_device_name("  Pixel 8\n"), "Pixel 8");
        assert_eq!(clean_device_name("\u{7}a\u{0}b\u{1b}[31m"), "ab[31m");
        assert_eq!(clean_device_name(&"é".repeat(100)).chars().count(), 64);
        assert_eq!(clean_device_name(""), "");
        // A message carries the cleaned name, whatever the sender put in.
        let wire = encode_data_message(&DataMessage::ParentProof {
            mac: "ab".repeat(32),
            name: format!("\u{7}{}", "x".repeat(200)),
        });
        let back = parse_data_message(&wire).unwrap();
        assert!(matches!(back, DataMessage::ParentProof { name, .. } if name == "x".repeat(64)));
    }

    #[test]
    fn the_parent_secret_travels_only_in_parent_pin_and_never_in_debug() {
        let msg = DataMessage::ParentPin {
            secret: ParentSecret {
                bytes: vec![0xab; 32],
            },
            name: "Pixel 8".into(),
        };
        assert!(encode_data_message(&msg).contains(&"ab".repeat(32)));
        assert!(!format!("{msg:?}").contains("abab"));
        for other in [
            DataMessage::PinAck,
            DataMessage::StopNotice,
            DataMessage::ParentProof {
                mac: "cd".repeat(32),
                name: String::new(),
            },
        ] {
            assert!(!encode_data_message(&other).contains(&"ab".repeat(32)));
        }
    }

    #[test]
    fn every_data_message_round_trips() {
        let msgs = vec![
            DataMessage::ParentPin {
                secret: ParentSecret {
                    bytes: (0..32).collect(),
                },
                name: "Pixel 8".into(),
            },
            DataMessage::PinAck,
            DataMessage::BabyProof {
                mac: "5a".repeat(32),
            },
            DataMessage::ParentProof {
                mac: "0f".repeat(32),
                name: String::new(),
            },
            DataMessage::Mailboxes {
                mailboxes: vec![InviteMailbox {
                    kind: crate::mailbox::MailboxKind::Relay,
                    url: "wss://relay.example".into(),
                }],
            },
            DataMessage::Heartbeat {
                t: 5.0,
                seq: 2,
                battery: None,
                rms: None,
            },
            DataMessage::Listen { enabled: true },
            DataMessage::NoiseAlert {
                rms: 0.5,
                t: 9.0,
                probe: true,
            },
            DataMessage::ProbeTest { t: 3.0 },
            DataMessage::PairProof {
                mac: "a".repeat(64),
            },
            DataMessage::OpenSettings,
            DataMessage::DeviceStatus(DeviceStatus {
                battery_pct: Some(85),
                charging: Some(true),
                ringer: Some(Ringer::Silent),
                dnd: Some(Dnd::AlarmsOnly),
                alarms_allowed: Some(true),
                dnd_access: Some(false),
                quiet_active: Some(true),
                next_alarm_min: Some(1440),
                mic_silent: Some(false),
                noise_threshold: Some(0.12),
                unit_warnings: vec![UnitWarning::PageHidden, UnitWarning::MicPermissionLost],
                talk: Some(TalkStatus::On),
                talker: Some(Talker::You),
                noise_paused: Some(true),
                talk_retry_in_ms: Some(10_000),
                talk_audible: Some(TalkAudible::Ok),
            }),
            DataMessage::DeviceStatus(DeviceStatus::default()),
            DataMessage::MissedEvents {
                count: 4,
                max_rms: 0.75,
                complete: false,
                down_ms: 90_000,
            },
            DataMessage::QuietRequest,
            DataMessage::StopNotice,
        ];
        for m in msgs {
            assert_eq!(parse_data_message(&encode_data_message(&m)), Some(m));
        }
    }

    #[test]
    fn device_status_is_strictly_validated() {
        let ok = |extra: &str| {
            parse_data_message(&format!(r#"{{"type":"device_status","v":3{extra}}}"#))
        };
        assert!(ok("").is_some());
        assert!(ok(r#","battery_pct":100"#).is_some());
        assert!(ok(r#","battery_pct":101"#).is_none());
        assert!(ok(r#","battery_pct":-1"#).is_none());
        assert!(ok(r#","battery_pct":12.5"#).is_none());
        assert!(ok(r#","battery_pct":"5""#).is_none());
        assert!(ok(r#","charging":1"#).is_none());
        assert!(ok(r#","ringer":"loud""#).is_none());
        assert!(ok(r#","ringer":3"#).is_none());
        assert!(ok(r#","dnd":"none""#).is_none());
        assert!(ok(r#","dnd":"total""#).is_some());
        assert!(ok(r#","next_alarm_min":1440"#).is_some());
        assert!(ok(r#","next_alarm_min":1441"#).is_none());
        assert!(ok(r#","mic_silent":"yes""#).is_none());
        assert!(ok(r#","noise_threshold":0.12"#).is_some());
        assert!(ok(r#","noise_threshold":1"#).is_some());
        assert!(ok(r#","noise_threshold":0.01"#).is_some());
        assert!(ok(r#","noise_threshold":0.009"#).is_none());
        assert!(ok(r#","noise_threshold":1.5"#).is_none());
        assert!(ok(r#","noise_threshold":"0.1""#).is_none());
        assert!(ok(r#","unknown_future_field":1"#).is_some());
    }

    #[test]
    fn unit_warnings_are_validated_sorted_and_deduplicated() {
        let parse = |extra: &str| {
            parse_data_message(&format!(r#"{{"type":"device_status","v":3,{extra}}}"#))
        };
        let warnings = |extra: &str| match parse(extra) {
            Some(DataMessage::DeviceStatus(s)) => s.unit_warnings,
            other => panic!("{other:?}"),
        };
        assert_eq!(warnings(r#""unit_warnings":[]"#), vec![]);
        assert_eq!(
            warnings(r#""unit_warnings":["not_charging","page_hidden","not_charging"]"#),
            vec![UnitWarning::PageHidden, UnitWarning::NotCharging]
        );
        assert!(parse(r#""unit_warnings":["bogus"]"#).is_none());
        assert!(parse(r#""unit_warnings":[1]"#).is_none());
        assert!(parse(r#""unit_warnings":"page_hidden""#).is_none());
        assert!(parse(r#""unit_warnings":null"#).is_none());
        let eight = r#""unit_warnings":["page_hidden","page_hidden","page_hidden","page_hidden","page_hidden","page_hidden","page_hidden","page_hidden"]"#;
        assert_eq!(warnings(eight), vec![UnitWarning::PageHidden]);
        let nine = eight.replace("]", r#","page_hidden"]"#);
        assert!(parse(&nine).is_none());
    }

    #[test]
    fn unit_warnings_encode_in_wire_order_and_are_omitted_when_empty() {
        let enc = |w: Vec<UnitWarning>| {
            encode_data_message(&DataMessage::DeviceStatus(
                DeviceStatus {
                    unit_warnings: w,
                    ..Default::default()
                }
                .normalized(),
            ))
        };
        assert_eq!(enc(vec![]), r#"{"type":"device_status","v":3}"#);
        assert_eq!(
            enc(vec![
                UnitWarning::MicPermissionLost,
                UnitWarning::WakeLockOff,
                UnitWarning::WakeLockOff
            ]),
            r#"{"type":"device_status","v":3,"unit_warnings":["wake_lock_off","mic_permission_lost"]}"#
        );
    }

    fn talk(raw: &str) -> Option<DataMessage> {
        parse_data_message(raw)
    }

    #[test]
    fn talk_messages_round_trip_and_keep_their_wire_shape() {
        let msgs = [
            (
                DataMessage::TalkRequest { id: 7 },
                r#"{"type":"talk_request","v":3,"id":7}"#,
            ),
            (
                DataMessage::TalkGrant {
                    id: 7,
                    max_ms: 30_000,
                },
                r#"{"type":"talk_grant","v":3,"id":7,"max_ms":30000}"#,
            ),
            (
                DataMessage::TalkDeny {
                    id: 7,
                    reason: Some(TalkDenyReason::Busy),
                },
                r#"{"type":"talk_deny","v":3,"id":7,"reason":"busy"}"#,
            ),
            (
                DataMessage::TalkDeny {
                    id: 7,
                    reason: Some(TalkDenyReason::Cooldown {
                        retry_in_ms: 12_000,
                    }),
                },
                r#"{"type":"talk_deny","v":3,"id":7,"reason":"cooldown","retry_in_ms":12000}"#,
            ),
            (
                DataMessage::TalkDeny {
                    id: 7,
                    reason: Some(TalkDenyReason::Budget { retry_in_ms: 1 }),
                },
                r#"{"type":"talk_deny","v":3,"id":7,"reason":"budget","retry_in_ms":1}"#,
            ),
            (
                DataMessage::TalkDeny {
                    id: 7,
                    reason: None,
                },
                r#"{"type":"talk_deny","v":3,"id":7}"#,
            ),
            (
                DataMessage::TalkEnd { id: 7 },
                r#"{"type":"talk_end","v":3,"id":7}"#,
            ),
            (
                DataMessage::TalkPlayback {
                    id: 7,
                    result: PlaybackResult::Unconfirmed,
                },
                r#"{"type":"talk_playback","v":3,"id":7,"result":"unconfirmed"}"#,
            ),
        ];
        for (m, wire) in msgs {
            assert_eq!(encode_data_message(&m), wire);
            assert_eq!(parse_data_message(wire), Some(m));
        }
        for reason in ["off", "unsupported", "muted", "busy", "rate"] {
            let raw = format!(r#"{{"type":"talk_deny","v":3,"id":1,"reason":"{reason}"}}"#);
            assert!(matches!(
                talk(&raw),
                Some(DataMessage::TalkDeny {
                    reason: Some(_),
                    ..
                })
            ));
        }
    }

    #[test]
    fn talk_messages_need_a_valid_id_and_protocol_v3() {
        for raw in [
            r#"{"type":"talk_request","v":3}"#,
            r#"{"type":"talk_request","v":3,"id":"1"}"#,
            r#"{"type":"talk_request","v":3,"id":-1}"#,
            r#"{"type":"talk_request","v":3,"id":1.5}"#,
            r#"{"type":"talk_request","v":3,"id":null}"#,
            r#"{"type":"talk_request","v":2,"id":1}"#,
            r#"{"type":"talk_request","id":1}"#,
            r#"{"type":"talk_grant","v":3,"id":1}"#,
            r#"{"type":"talk_grant","v":3,"max_ms":1}"#,
            r#"{"type":"talk_grant","v":3,"id":1,"max_ms":-5}"#,
            r#"{"type":"talk_end","v":3}"#,
            r#"{"type":"talk_deny","v":3,"reason":"busy"}"#,
            r#"{"type":"talk_playback","v":3,"id":1}"#,
            r#"{"type":"talk_playback","v":3,"id":1,"result":"loud"}"#,
            r#"{"type":"talk_playback","v":3,"id":1,"result":7}"#,
        ] {
            assert_eq!(talk(raw), None, "{raw}");
        }
        assert_eq!(
            parse_data_message_v2_legacy(r#"{"type":"talk_request","v":2,"id":1}"#),
            None
        );
    }

    #[test]
    fn an_unknown_deny_reason_parses_as_none_and_extra_fields_are_ignored() {
        for raw in [
            r#"{"type":"talk_deny","v":3,"id":4,"reason":"too_loud"}"#,
            r#"{"type":"talk_deny","v":3,"id":4,"reason":7}"#,
            // a cooldown without its hint, or with an absurd one, has no usable reason
            r#"{"type":"talk_deny","v":3,"id":4,"reason":"cooldown"}"#,
            r#"{"type":"talk_deny","v":3,"id":4,"reason":"budget","retry_in_ms":99999999}"#,
        ] {
            assert_eq!(
                talk(raw),
                Some(DataMessage::TalkDeny {
                    id: 4,
                    reason: None
                }),
                "{raw}"
            );
        }
        assert_eq!(
            talk(r#"{"type":"talk_end","v":3,"id":9,"future":true}"#),
            Some(DataMessage::TalkEnd { id: 9 })
        );
    }

    #[test]
    fn no_message_can_change_the_talk_back_settings() {
        // Settings are a local input of the baby platform; nothing on the wire maps to them.
        for raw in [
            r#"{"type":"set_talk_back","v":3,"enabled":true,"level":"high","chime":false}"#,
            r#"{"type":"talk_settings","v":3,"enabled":true}"#,
        ] {
            assert_eq!(talk(raw), None, "{raw}");
        }
        // A request that carries settings fields is still only a request.
        assert_eq!(
            talk(r#"{"type":"talk_request","v":3,"id":2,"enabled":true,"level":"high"}"#),
            Some(DataMessage::TalkRequest { id: 2 })
        );
    }

    #[test]
    fn talk_status_fields_parse_encode_and_stay_optional() {
        let raw = r#"{"type":"device_status","v":3,"talk":"muted","talker":"other","noise_paused":true,"talk_retry_in_ms":12000,"talk_audible":"uncertain"}"#;
        let Some(DataMessage::DeviceStatus(s)) = talk(raw) else {
            panic!("status");
        };
        assert_eq!(s.talk, Some(TalkStatus::Muted));
        assert_eq!(s.talker, Some(Talker::Other));
        assert_eq!(s.noise_paused, Some(true));
        assert_eq!(s.talk_retry_in_ms, Some(12_000));
        assert_eq!(s.talk_audible, Some(TalkAudible::Uncertain));
        assert_eq!(encode_data_message(&DataMessage::DeviceStatus(s)), raw);
        // Unknown words are no value, not an error; wrong types are errors.
        let Some(DataMessage::DeviceStatus(s)) =
            talk(r#"{"type":"device_status","v":3,"talk":"loud","talker":"nobody"}"#)
        else {
            panic!("status");
        };
        assert_eq!((s.talk, s.talker), (None, None));
        for raw in [
            r#"{"type":"device_status","v":3,"talk":1}"#,
            r#"{"type":"device_status","v":3,"noise_paused":"yes"}"#,
            r#"{"type":"device_status","v":3,"talk_retry_in_ms":-1}"#,
            r#"{"type":"device_status","v":3,"talk_retry_in_ms":3600001}"#,
            r#"{"type":"device_status","v":3,"talk_audible":false}"#,
        ] {
            assert_eq!(talk(raw), None, "{raw}");
        }
        // Fields that are not set stay off the wire.
        assert_eq!(
            encode_data_message(&DataMessage::DeviceStatus(DeviceStatus::default())),
            r#"{"type":"device_status","v":3}"#
        );
    }

    #[test]
    fn device_status_normalization_rounds_to_five_and_caps_alarm() {
        let n = |p: u8| {
            DeviceStatus {
                battery_pct: Some(p),
                ..Default::default()
            }
            .normalized()
            .battery_pct
            .unwrap()
        };
        assert_eq!(n(0), 0);
        assert_eq!(n(2), 0);
        assert_eq!(n(3), 5);
        assert_eq!(n(87), 85);
        assert_eq!(n(98), 100);
        assert_eq!(n(200), 100);
        let s = DeviceStatus {
            next_alarm_min: Some(99_999),
            ..Default::default()
        }
        .normalized();
        assert_eq!(s.next_alarm_min, Some(1440));
    }

    #[test]
    fn missed_events_are_strictly_validated() {
        let p =
            |body: &str| parse_data_message(&format!(r#"{{"type":"missed_events","v":3,{body}}}"#));
        assert!(p(r#""count":2,"max_rms":0.5,"complete":true,"down_ms":10"#).is_some());
        assert!(p(r#""count":-2,"max_rms":0.5,"complete":true,"down_ms":10"#).is_none());
        assert!(p(r#""count":2,"max_rms":1.5,"complete":true,"down_ms":10"#).is_none());
        assert!(p(r#""count":2,"max_rms":0.5,"complete":"y","down_ms":10"#).is_none());
        assert!(p(r#""count":2,"max_rms":0.5,"complete":true"#).is_none());
        assert!(p(r#""count":5000000000,"max_rms":0.5,"complete":true,"down_ms":1"#).is_none());
    }

    #[test]
    fn signal_payloads_require_a_valid_aid() {
        let sdp = |aid: &str| json!({ "type": "sdp", "aid": aid, "sdp": { "type": "offer", "sdp": "x" } });
        assert!(parse_signal_payload(&sdp(AID)).is_some());
        assert!(parse_signal_payload(&sdp("short")).is_none());
        assert!(parse_signal_payload(&sdp(&AID.to_uppercase())).is_none());
        assert!(parse_signal_payload(&sdp(&"g".repeat(32))).is_none());
        assert!(
            parse_signal_payload(&json!({"type":"sdp","sdp":{"type":"offer","sdp":"x"}})).is_none()
        );
        assert!(
            parse_signal_payload(&json!({"type":"ice","candidate":{"candidate":"c"}})).is_none()
        );
        assert!(parse_signal_payload(&json!({"type":"request_offer"})).is_none());
        let with_re = |re: Value| json!({ "type": "sdp", "aid": AID, "re": re, "sdp": { "type": "offer", "sdp": "x" } });
        assert!(parse_signal_payload(&with_re(json!(AID))).is_some());
        assert!(parse_signal_payload(&with_re(json!("nope"))).is_none());
        assert!(parse_signal_payload(&with_re(json!(5))).is_none());
        assert_eq!(
            parse_signal_payload(&json!({"type":"request_offer","aid":AID})),
            Some(SignalPayload::RequestOffer { aid: AID.into() })
        );
        // The stop carries nothing; a leftover MAC from an old peer is ignored, not trusted.
        for stop in [
            json!({"type":"stop_notice"}),
            json!({"type":"stop_notice","aid":AID,"mac":"ab".repeat(32)}),
        ] {
            assert_eq!(parse_signal_payload(&stop), Some(SignalPayload::StopNotice));
        }
    }

    #[test]
    fn next_alarm_is_rounded_to_15_minutes_when_sent_and_parsed() {
        let n = |m: u32| {
            DeviceStatus {
                next_alarm_min: Some(m),
                ..Default::default()
            }
            .normalized()
            .next_alarm_min
            .unwrap()
        };
        assert_eq!(
            [n(0), n(7), n(8), n(22), n(23), n(1440), n(99_999)],
            [0, 0, 15, 15, 30, 1440, 1440]
        );
        let parsed = |m: u32| match parse_data_message(&format!(
            r#"{{"type":"device_status","v":3,"next_alarm_min":{m}}}"#
        )) {
            Some(DataMessage::DeviceStatus(s)) => s.next_alarm_min,
            other => panic!("{other:?}"),
        };
        assert_eq!(parsed(7), Some(0));
        assert_eq!(parsed(37), Some(30));
        assert_eq!(parsed(1439), Some(1440));
    }

    #[test]
    fn noise_alert_rms_is_clamped_to_the_unit_interval() {
        let rms = |r: &str| match parse_data_message(&format!(
            r#"{{"type":"noise_alert","v":3,"rms":{r},"t":1}}"#
        )) {
            Some(DataMessage::NoiseAlert { rms, .. }) => rms,
            other => panic!("{other:?}"),
        };
        assert_eq!(rms("7.5"), 1.0);
        assert_eq!(rms("-3"), 0.0);
        assert_eq!(rms("0.4"), 0.4);
    }

    #[test]
    fn an_ice_bundle_is_never_empty_or_oversized() {
        let one = r#"{"candidate":"candidate:1","sdpMid":"0","sdpMLineIndex":0}"#;
        let bundle = |n: usize| {
            let list = vec![one; n].join(",");
            serde_json::from_str::<Value>(&format!(
                r#"{{"type":"ice","aid":"{AID}","candidates":[{list}]}}"#
            ))
            .unwrap()
        };
        assert!(parse_signal_payload(&bundle(0)).is_none());
        assert!(parse_signal_payload(&bundle(1)).is_some());
        assert!(parse_signal_payload(&bundle(MAX_BUNDLED_CANDIDATES)).is_some());
        assert!(parse_signal_payload(&bundle(MAX_BUNDLED_CANDIDATES + 1)).is_none());
        let bad = serde_json::from_str::<Value>(&format!(
            r#"{{"type":"ice","aid":"{AID}","candidates":[{one},7]}}"#
        ))
        .unwrap();
        assert!(parse_signal_payload(&bad).is_none());
    }

    #[test]
    fn signal_payloads_round_trip() {
        let all = vec![
            SignalPayload::Sdp {
                aid: AID.into(),
                sdp_type: "answer".into(),
                sdp: "v=0".into(),
                re: None,
            },
            SignalPayload::Sdp {
                aid: AID.into(),
                sdp_type: "offer".into(),
                sdp: "v=0".into(),
                re: Some("ffeeddccbbaa99887766554433221100".into()),
            },
            SignalPayload::Ice {
                aid: AID.into(),
                candidate: IceCandidate {
                    candidate: "candidate:1".into(),
                    sdp_mid: Some("0".into()),
                    sdp_m_line_index: Some(0),
                },
            },
            SignalPayload::IceBundle {
                aid: AID.into(),
                candidates: vec![
                    IceCandidate {
                        candidate: "candidate:1".into(),
                        sdp_mid: Some("0".into()),
                        sdp_m_line_index: Some(0),
                    },
                    IceCandidate {
                        candidate: "candidate:2".into(),
                        sdp_mid: None,
                        sdp_m_line_index: None,
                    },
                ],
            },
            SignalPayload::RequestOffer { aid: AID.into() },
            SignalPayload::StopNotice,
        ];
        for p in all {
            assert_eq!(parse_signal_payload(&signal_payload_json(&p)), Some(p));
        }
    }

    #[test]
    fn hello_announces_protocol_v3() {
        let v: Value = serde_json::from_str(&encode_hello(Role::Baby, "r")).unwrap();
        assert_eq!(v["protocolVersion"], 3);
    }

    #[test]
    fn client_messages_match_the_ts_parser() {
        assert_eq!(
            parse_client_message(r#"{"type":"ping"}"#),
            Some(ClientMessage::Ping)
        );
        let hello = encode_hello(Role::Baby, "r1");
        assert_eq!(
            parse_client_message(&hello),
            Some(ClientMessage::Hello {
                role: Role::Baby,
                room_id: "r1".into(),
                protocol_version: 3
            })
        );
        assert_eq!(
            parse_client_message(
                r#"{"type":"hello","role":"admin","roomId":"r","protocolVersion":3}"#
            ),
            None
        );
        assert_eq!(
            parse_client_message(
                r#"{"type":"hello","role":"baby","roomId":"","protocolVersion":3}"#
            ),
            None
        );
        assert_eq!(
            parse_client_message(
                r#"{"type":"hello","role":"baby","roomId":"r","protocolVersion":"3"}"#
            ),
            None
        );
        assert!(matches!(
            parse_client_message(r#"{"type":"signal","roomId":"r","payload":null}"#),
            Some(ClientMessage::Signal { to: None, .. })
        ));
        assert_eq!(
            parse_client_message(r#"{"type":"signal","roomId":"r"}"#),
            None
        );
        assert_eq!(
            parse_client_message(r#"{"type":"signal","roomId":"r","to":"","payload":1}"#),
            None
        );
        assert_eq!(parse_client_message("[]"), None);
        assert_eq!(parse_client_message("nope"), None);
    }

    #[test]
    fn admission_messages_round_trip() {
        let nonce = "AAAAAAAAAAAAAAAAAAAAAA";
        assert!(valid_nonce(nonce));
        assert!(!valid_nonce("short"));
        assert!(!valid_nonce("AAAAAAAAAAAAAAAAAAAAA!"));
        assert_eq!(
            parse_challenge(&encode_challenge(nonce)).as_deref(),
            Some(nonce)
        );
        assert_eq!(parse_challenge(r#"{"type":"challenge","nonce":"x"}"#), None);
        assert_eq!(parse_challenge(r#"{"type":"pong"}"#), None);
        let mac = "ab".repeat(32);
        assert_eq!(
            parse_client_message(&encode_admit(Role::Parent, "r1", &mac)),
            Some(ClientMessage::Admit {
                role: Role::Parent,
                room_id: "r1".into(),
                protocol_version: 3,
                mac
            })
        );
        assert_eq!(
            parse_client_message(&encode_admit(Role::Parent, "r1", "AB")),
            None
        );
    }
}
