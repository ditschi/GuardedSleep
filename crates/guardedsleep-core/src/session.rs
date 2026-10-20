//! Single-shot session: one signaling join plus exactly one peer-connection [`Attempt`]. It is the
//! thin legacy wrapper kept for the Android PoC and the characterization vectors; recovery,
//! supervision and escalation live in [`crate::link::Link`]. The platform owns sockets, timers,
//! WebRTC and audio; it feeds [`Input`]s (with a monotonic millisecond timestamp) into
//! [`Session::handle`] and executes the returned [`Action`]s.

use serde::{Deserialize, Serialize};

use crate::attempt::{Attempt, AttemptConfig, AttemptEvent};
use crate::noise::{GateChange, NoiseGate};
use crate::pairing::decode_psk;
use crate::protocol::{
    encode_hello, encode_signal, parse_server_message, parse_signal_payload, signal_payload_json,
    valid_room_id, DataMessage, IceCandidate, Role, ServerMessage, SignalPayload,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
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

impl SessionConfig {
    /// Defaults: 10 s heartbeat, 50 s liveness, 15 s attempt timeout.
    pub fn with_defaults(role: Role, room_id: String, psk_b64url: Option<String>) -> Self {
        Self {
            role,
            room_id,
            psk_b64url,
            heartbeat_interval_ms: 10_000,
            heartbeat_stale_ms: 50_000,
            pairing_timeout_ms: 15_000,
            noise_threshold: 0.12,
            noise_hold_ms: 800,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum SessionState {
    Connecting,
    Joined,
    Connected,
    Verified,
    Failed,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    InvalidRoomId,
    InvalidPsk,
    InvalidTiming,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ConfigError::InvalidRoomId => "room id must be 1..=128 characters",
            ConfigError::InvalidPsk => "psk must be non-empty base64url",
            ConfigError::InvalidTiming => "intervals and timeouts must be > 0",
        })
    }
}

pub struct Session {
    cfg: SessionConfig,
    attempt: Attempt,
    finished: bool,
    joined: bool,
    remote_peer: Option<String>,
    link_lost: bool,
    parent_listen: bool,
    noise_streaming: bool,
    mic_enabled: bool,
    gate: NoiseGate,
}

fn session_aid(role: Role, room_id: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(format!("gs-session-aid|{}|{room_id}", role.as_str()));
    digest[..16].iter().map(|b| format!("{b:02x}")).collect()
}

impl Session {
    pub fn new(cfg: SessionConfig) -> Result<Self, ConfigError> {
        if !valid_room_id(&cfg.room_id) {
            return Err(ConfigError::InvalidRoomId);
        }
        let psk = match &cfg.psk_b64url {
            None => None,
            Some(s) => Some(
                decode_psk(s)
                    .filter(|k| !k.is_empty())
                    .ok_or(ConfigError::InvalidPsk)?,
            ),
        };
        if cfg.heartbeat_interval_ms == 0
            || cfg.heartbeat_stale_ms == 0
            || cfg.pairing_timeout_ms == 0
        {
            return Err(ConfigError::InvalidTiming);
        }
        let attempt = Attempt::new(
            AttemptConfig {
                role: cfg.role,
                psk,
                room: cfg.room_id.clone(),
                handle: 0,
                own_aid: session_aid(cfg.role, &cfg.room_id),
                remote_aid: None,
                reply_to: None,
                heartbeat_interval_ms: cfg.heartbeat_interval_ms,
                liveness_ms: cfg.heartbeat_stale_ms,
                attempt_timeout_ms: cfg.pairing_timeout_ms,
                require_sendonly: false,
                require_dtls_fingerprint: false,
            },
            0,
        );
        let gate = NoiseGate::new(cfg.noise_threshold, cfg.noise_hold_ms);
        Ok(Self {
            cfg,
            attempt,
            finished: false,
            joined: false,
            remote_peer: None,
            link_lost: false,
            parent_listen: false,
            noise_streaming: false,
            mic_enabled: false,
            gate,
        })
    }

    pub fn is_verified(&self) -> bool {
        self.attempt.is_verified()
    }

    pub fn handle(&mut self, now_ms: u64, input: Input) -> Vec<Action> {
        let mut out = Vec::new();
        if self.finished {
            return out;
        }
        match input {
            Input::SignalingOpen => {
                out.push(Action::SendSignaling {
                    text: encode_hello(self.cfg.role, &self.cfg.room_id),
                });
                emit_state(&mut out, SessionState::Connecting);
            }
            Input::SignalingMessage { text } => self.on_server_message(now_ms, &text, &mut out),
            Input::SignalingClosed => {
                // Once verified the media path is peer-to-peer; losing signaling is not fatal.
                if !self.attempt.is_verified() {
                    self.fail("signaling_closed", &mut out);
                }
            }
            Input::LocalDescription { sdp_type, sdp } => {
                let ev = self.attempt.on_local_description(now_ms, &sdp_type, &sdp);
                self.apply(now_ms, ev, &mut out);
            }
            Input::LocalIceCandidate { candidate } => {
                let ev = self.attempt.on_local_ice(candidate);
                self.apply(now_ms, ev, &mut out);
            }
            Input::DataChannelOpen => {
                let ev = self.attempt.on_data_open(now_ms);
                self.apply(now_ms, ev, &mut out);
            }
            Input::DataChannelMessage { text } => {
                let ev = self.attempt.on_data_message(now_ms, &text);
                self.apply(now_ms, ev, &mut out);
            }
            Input::DataChannelClosed => {
                let ev = self.attempt.on_data_closed();
                self.apply(now_ms, ev, &mut out);
            }
            Input::PeerConnectionState { state } => {
                if state == "connected" {
                    emit_state(&mut out, SessionState::Connected);
                }
                let ev = self.attempt.on_pc_state(&state);
                self.apply(now_ms, ev, &mut out);
            }
            Input::AudioLevel { rms } => self.on_audio_level(now_ms, rms, &mut out),
            Input::SetListen { enabled } => {
                if self.cfg.role == Role::Parent {
                    self.send_app(&DataMessage::Listen { enabled }, &mut out);
                }
            }
            Input::RequestProbe => {
                if self.cfg.role == Role::Parent {
                    self.send_app(&DataMessage::ProbeTest { t: now_ms as f64 }, &mut out);
                }
            }
            Input::Tick => {
                if self.cfg.role == Role::Baby {
                    self.attempt
                        .set_heartbeat_rms(Some(round4(self.gate.last_rms())));
                }
                let ev = self.attempt.tick(now_ms);
                self.apply(now_ms, ev, &mut out);
            }
            Input::Close => {
                self.finished = true;
                out.push(Action::CloseTransport);
                emit_state(&mut out, SessionState::Closed);
            }
        }
        out
    }

    fn on_server_message(&mut self, now_ms: u64, text: &str, out: &mut Vec<Action>) {
        let Some(msg) = parse_server_message(text) else {
            return;
        };
        match msg {
            ServerMessage::Welcome { room_id, .. } => {
                if room_id == self.cfg.room_id {
                    self.joined = true;
                    emit_state(out, SessionState::Joined);
                }
            }
            ServerMessage::Peer { peer_id: Some(id) } => {
                if self.remote_peer.is_none() {
                    self.remote_peer = Some(id);
                    if self.cfg.role == Role::Baby {
                        out.push(Action::CreateOffer);
                    }
                }
            }
            ServerMessage::Peer { peer_id: None } => {
                if self.attempt.is_verified() {
                    self.link_lost("peer_left", out);
                }
            }
            ServerMessage::Signal { payload, .. } => {
                let Some(p) = parse_signal_payload(&payload) else {
                    return;
                };
                match p {
                    SignalPayload::Sdp {
                        aid, sdp_type, sdp, ..
                    } => {
                        let Some(ev) = self
                            .attempt
                            .on_remote_description(now_ms, &aid, &sdp_type, &sdp)
                        else {
                            return;
                        };
                        out.push(Action::SetRemoteDescription { sdp_type, sdp });
                        if self.cfg.role == Role::Parent {
                            out.push(Action::CreateAnswer);
                        }
                        self.apply(now_ms, ev, out);
                    }
                    SignalPayload::Ice { aid, candidate } => {
                        if self.attempt.accepts_ice(&aid) {
                            out.push(Action::AddIceCandidate { candidate });
                        }
                    }
                    SignalPayload::RequestOffer { .. } | SignalPayload::StopNotice { .. } => {}
                }
            }
            ServerMessage::Error { code, .. } => self.fail(&format!("signaling_error:{code}"), out),
            ServerMessage::Members { .. } | ServerMessage::Pong => {}
        }
    }

    fn apply(&mut self, now_ms: u64, events: Vec<AttemptEvent>, out: &mut Vec<Action>) {
        for ev in events {
            match ev {
                AttemptEvent::Signal(payload) => self.send_signal(payload, out),
                AttemptEvent::SendData(text) => out.push(Action::SendData { text }),
                AttemptEvent::Verified => {
                    out.push(Action::Emit {
                        event: SessionEvent::PairingVerified,
                    });
                    emit_state(out, SessionState::Verified);
                }
                AttemptEvent::Message(msg) => self.on_app_message(now_ms, msg, out),
                AttemptEvent::Mismatch => self.fail("pair_proof_mismatch", out),
                AttemptEvent::DtlsMismatch => self.fail("dtls_fingerprint_mismatch", out),
                AttemptEvent::LivenessLost => self.fail("heartbeat_stale", out),
                AttemptEvent::Failed(reason) => self.fail(&reason, out),
            }
            if self.finished {
                return;
            }
        }
    }

    fn on_app_message(&mut self, now_ms: u64, msg: DataMessage, out: &mut Vec<Action>) {
        match (self.cfg.role, msg) {
            (Role::Baby, DataMessage::Listen { enabled }) => {
                self.parent_listen = enabled;
                self.apply_mic_gate(out);
            }
            (Role::Baby, DataMessage::ProbeTest { .. }) => {
                self.send_app(
                    &DataMessage::NoiseAlert {
                        rms: self.gate.last_rms(),
                        t: now_ms as f64,
                        probe: true,
                    },
                    out,
                );
            }
            (Role::Parent, DataMessage::NoiseAlert { rms, probe, .. }) => {
                out.push(Action::Emit {
                    event: SessionEvent::NoiseAlert { rms, probe },
                });
            }
            (
                Role::Parent,
                DataMessage::Heartbeat {
                    seq, rms, battery, ..
                },
            ) => {
                self.link_lost = false;
                out.push(Action::Emit {
                    event: SessionEvent::Heartbeat { seq, rms, battery },
                });
            }
            _ => {}
        }
    }

    fn on_audio_level(&mut self, now_ms: u64, rms: f64, out: &mut Vec<Action>) {
        if self.cfg.role != Role::Baby {
            return;
        }
        match self.gate.sample(now_ms, rms) {
            Some(GateChange::Loud { rms }) => {
                self.noise_streaming = true;
                self.apply_mic_gate(out);
                self.send_app(
                    &DataMessage::NoiseAlert {
                        rms,
                        t: now_ms as f64,
                        probe: false,
                    },
                    out,
                );
            }
            Some(GateChange::Quiet) => {
                self.noise_streaming = false;
                self.apply_mic_gate(out);
            }
            None => {}
        }
    }

    fn send_app(&self, msg: &DataMessage, out: &mut Vec<Action>) {
        if let Some(text) = self.attempt.encode_app_message(msg) {
            out.push(Action::SendData { text });
        }
    }

    fn apply_mic_gate(&mut self, out: &mut Vec<Action>) {
        let enabled = self.parent_listen || self.noise_streaming;
        if enabled != self.mic_enabled {
            self.mic_enabled = enabled;
            out.push(Action::SetMicEnabled { enabled });
        }
    }

    fn send_signal(&self, payload: SignalPayload, out: &mut Vec<Action>) {
        // Same rule as ws-client.ts: nothing is relayed before `welcome`.
        if !self.joined {
            return;
        }
        let text = encode_signal(
            &self.cfg.room_id,
            self.remote_peer.as_deref(),
            signal_payload_json(&payload),
        );
        out.push(Action::SendSignaling { text });
    }

    fn link_lost(&mut self, reason: &str, out: &mut Vec<Action>) {
        if self.cfg.role == Role::Parent && !self.link_lost {
            self.link_lost = true;
            out.push(Action::Emit {
                event: SessionEvent::LinkLost {
                    reason: reason.to_owned(),
                },
            });
        }
    }

    fn fail(&mut self, reason: &str, out: &mut Vec<Action>) {
        if self.attempt.is_verified() {
            self.link_lost(reason, out);
        }
        self.finished = true;
        out.push(Action::Emit {
            event: SessionEvent::Failed {
                reason: reason.to_owned(),
            },
        });
        emit_state(out, SessionState::Failed);
        out.push(Action::CloseTransport);
    }
}

fn emit_state(out: &mut Vec<Action>, state: SessionState) {
    out.push(Action::Emit {
        event: SessionEvent::StateChanged { state },
    });
}

fn round4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pairing::encode_psk;
    use crate::protocol::parse_data_message;

    const OFFER: &str = "v=0\r\na=fingerprint:sha-256 AA:AB:AC:AD:AE:AF:B0:B1:B2:B3:B4:B5:B6:B7:B8:B9:BA:BB:BC:BD:BE:BF:C0:C1:C2:C3:C4:C5:C6:C7:C8:C9\r\n";
    const ANSWER: &str = "v=0\r\na=fingerprint:sha-256 CC:CD:CE:CF:D0:D1:D2:D3:D4:D5:D6:D7:D8:D9:DA:DB:DC:DD:DE:DF:E0:E1:E2:E3:E4:E5:E6:E7:E8:E9:EA:EB\r\n";

    fn cfg(role: Role, psk: &[u8]) -> SessionConfig {
        let mut c = SessionConfig::with_defaults(role, "room-1".into(), Some(encode_psk(psk)));
        c.heartbeat_interval_ms = 5_000;
        c.heartbeat_stale_ms = 15_000;
        c.noise_hold_ms = 300;
        c
    }

    fn data_texts(actions: &[Action]) -> Vec<String> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::SendData { text } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    fn has_event(actions: &[Action], pred: impl Fn(&SessionEvent) -> bool) -> bool {
        actions
            .iter()
            .any(|a| matches!(a, Action::Emit { event } if pred(event)))
    }

    fn sdp_signal(from: &str, role: Role, sdp_type: &str, sdp: &str) -> Input {
        let payload = signal_payload_json(&SignalPayload::Sdp {
            aid: session_aid(role, "room-1"),
            sdp_type: sdp_type.into(),
            sdp: sdp.into(),
            re: None,
        });
        Input::SignalingMessage {
            text: serde_json::json!({"type":"signal","from":from,"payload":payload}).to_string(),
        }
    }

    fn welcome(id: &str) -> Input {
        Input::SignalingMessage {
            text: format!(r#"{{"type":"welcome","clientId":"{id}","roomId":"room-1"}}"#),
        }
    }

    fn peer(id: &str) -> Input {
        Input::SignalingMessage {
            text: format!(r#"{{"type":"peer","peerId":"{id}"}}"#),
        }
    }

    /// Drives a baby and a parent session against each other with an in-memory relay.
    struct Pair {
        baby: Session,
        parent: Session,
    }

    impl Pair {
        fn connect(baby_psk: &[u8], parent_psk: &[u8]) -> (Self, Vec<Action>, Vec<Action>) {
            let mut baby = Session::new(cfg(Role::Baby, baby_psk)).unwrap();
            let mut parent = Session::new(cfg(Role::Parent, parent_psk)).unwrap();
            baby.handle(0, Input::SignalingOpen);
            parent.handle(0, Input::SignalingOpen);
            baby.handle(0, welcome("b"));
            parent.handle(0, welcome("p"));
            let a = baby.handle(1, peer("p"));
            assert!(a.contains(&Action::CreateOffer));
            parent.handle(1, peer("b"));
            baby.handle(
                2,
                Input::LocalDescription {
                    sdp_type: "offer".into(),
                    sdp: OFFER.into(),
                },
            );
            let a = parent.handle(3, sdp_signal("b", Role::Baby, "offer", OFFER));
            assert!(a.contains(&Action::CreateAnswer));
            parent.handle(
                4,
                Input::LocalDescription {
                    sdp_type: "answer".into(),
                    sdp: ANSWER.into(),
                },
            );
            baby.handle(5, sdp_signal("p", Role::Parent, "answer", ANSWER));
            let mut bo = baby.handle(6, Input::DataChannelOpen);
            let mut po = parent.handle(6, Input::DataChannelOpen);
            let b_proof = data_texts(&bo);
            let p_proof = data_texts(&po);
            for t in p_proof {
                bo.extend(baby.handle(7, Input::DataChannelMessage { text: t }));
            }
            for t in b_proof {
                po.extend(parent.handle(7, Input::DataChannelMessage { text: t }));
            }
            (Self { baby, parent }, bo, po)
        }
    }

    #[test]
    fn matching_psk_verifies_both_sides() {
        let (pair, bo, po) = Pair::connect(&[9u8; 32], &[9u8; 32]);
        assert!(pair.baby.is_verified() && pair.parent.is_verified());
        assert!(has_event(&bo, |e| *e == SessionEvent::PairingVerified));
        assert!(has_event(&po, |e| *e == SessionEvent::PairingVerified));
        // Both roles send a heartbeat immediately after verification (v3).
        assert!(data_texts(&bo).iter().any(|t| t.contains("\"heartbeat\"")));
        assert!(data_texts(&po).iter().any(|t| t.contains("\"heartbeat\"")));
    }

    #[test]
    fn wrong_psk_fails_both_sides() {
        let (pair, bo, po) = Pair::connect(&[1u8; 32], &[2u8; 32]);
        assert!(!pair.baby.is_verified() && !pair.parent.is_verified());
        let mismatch = |e: &SessionEvent| matches!(e, SessionEvent::Failed { reason } if reason == "pair_proof_mismatch");
        assert!(has_event(&bo, mismatch));
        assert!(has_event(&po, mismatch));
        assert!(po.contains(&Action::CloseTransport));
    }

    #[test]
    fn app_messages_before_verification_are_dropped() {
        let mut parent = Session::new(cfg(Role::Parent, &[3u8; 32])).unwrap();
        let a = parent.handle(
            0,
            Input::DataChannelMessage {
                text: r#"{"type":"noise_alert","v":3,"rms":0.9,"t":1}"#.into(),
            },
        );
        assert!(a.is_empty());
    }

    #[test]
    fn candidates_with_a_foreign_aid_are_dropped() {
        let (mut pair, _, _) = Pair::connect(&[5u8; 32], &[5u8; 32]);
        let ice = |aid: &str| {
            let payload = signal_payload_json(&SignalPayload::Ice {
                aid: aid.into(),
                candidate: IceCandidate {
                    candidate: "candidate:1".into(),
                    sdp_mid: None,
                    sdp_m_line_index: None,
                },
            });
            Input::SignalingMessage {
                text: serde_json::json!({"type":"signal","from":"b","payload":payload}).to_string(),
            }
        };
        let own = session_aid(Role::Baby, "room-1");
        // Verified: negotiation is over, even the right aid is ignored.
        assert!(pair.parent.handle(8, ice(&own)).is_empty());
        assert!(pair.parent.handle(8, ice(&"c".repeat(32))).is_empty());

        let mut parent = Session::new(cfg(Role::Parent, &[5u8; 32])).unwrap();
        parent.handle(0, Input::SignalingOpen);
        parent.handle(0, welcome("p"));
        parent.handle(1, peer("b"));
        parent.handle(3, sdp_signal("b", Role::Baby, "offer", OFFER));
        assert!(matches!(
            parent.handle(4, ice(&own)).as_slice(),
            [Action::AddIceCandidate { .. }]
        ));
        assert!(parent.handle(4, ice(&"c".repeat(32))).is_empty());
    }

    #[test]
    fn noise_reaches_parent_as_alert() {
        let (mut pair, _, _) = Pair::connect(&[5u8; 32], &[5u8; 32]);
        pair.baby.handle(100, Input::AudioLevel { rms: 0.5 });
        let loud = pair.baby.handle(500, Input::AudioLevel { rms: 0.5 });
        assert!(loud.contains(&Action::SetMicEnabled { enabled: true }));
        let alert = data_texts(&loud).pop().unwrap();
        assert!(matches!(
            parse_data_message(&alert),
            Some(DataMessage::NoiseAlert { probe: false, .. })
        ));
        let p = pair
            .parent
            .handle(501, Input::DataChannelMessage { text: alert });
        assert!(has_event(&p, |e| matches!(
            e,
            SessionEvent::NoiseAlert { probe: false, .. }
        )));
    }

    #[test]
    fn probe_is_answered_with_a_probe_alert() {
        let (mut pair, _, _) = Pair::connect(&[5u8; 32], &[5u8; 32]);
        let req = data_texts(&pair.parent.handle(100, Input::RequestProbe))
            .pop()
            .unwrap();
        let reply = data_texts(
            &pair
                .baby
                .handle(101, Input::DataChannelMessage { text: req }),
        )
        .pop()
        .unwrap();
        assert!(matches!(
            parse_data_message(&reply),
            Some(DataMessage::NoiseAlert { probe: true, .. })
        ));
    }

    #[test]
    fn parent_detects_stale_peer_once_and_ends_the_session() {
        let (mut pair, _, _) = Pair::connect(&[5u8; 32], &[5u8; 32]);
        assert!(!has_event(&pair.parent.handle(15_000, Input::Tick), |e| {
            matches!(e, SessionEvent::LinkLost { .. })
        }));
        let lost = pair.parent.handle(15_008, Input::Tick);
        assert!(has_event(
            &lost,
            |e| matches!(e, SessionEvent::LinkLost { reason } if reason == "heartbeat_stale")
        ));
        assert!(pair.parent.handle(20_000, Input::Tick).is_empty());
    }

    #[test]
    fn heartbeat_resets_watchdog() {
        let (mut pair, _, _) = Pair::connect(&[5u8; 32], &[5u8; 32]);
        let hb = data_texts(&pair.baby.handle(5_007, Input::Tick))
            .pop()
            .unwrap();
        pair.parent
            .handle(5_008, Input::DataChannelMessage { text: hb });
        let a = pair.parent.handle(20_000, Input::Tick);
        assert!(!has_event(&a, |e| matches!(
            e,
            SessionEvent::LinkLost { .. }
        )));
    }

    #[test]
    fn peer_leaving_raises_link_lost_immediately() {
        let (mut pair, _, _) = Pair::connect(&[5u8; 32], &[5u8; 32]);
        let a = pair.parent.handle(
            100,
            Input::SignalingMessage {
                text: r#"{"type":"peer","peerId":null}"#.into(),
            },
        );
        assert!(has_event(
            &a,
            |e| matches!(e, SessionEvent::LinkLost { reason } if reason == "peer_left")
        ));
    }

    #[test]
    fn pairing_times_out_without_remote_proof() {
        let mut baby = Session::new(cfg(Role::Baby, &[1u8; 32])).unwrap();
        baby.handle(0, welcome("b"));
        baby.handle(0, peer("p"));
        baby.handle(
            0,
            Input::LocalDescription {
                sdp_type: "offer".into(),
                sdp: OFFER.into(),
            },
        );
        baby.handle(0, sdp_signal("p", Role::Parent, "answer", ANSWER));
        baby.handle(0, Input::DataChannelOpen);
        let a = baby.handle(15_000, Input::Tick);
        assert!(has_event(
            &a,
            |e| matches!(e, SessionEvent::Failed { reason } if reason == "attempt_timeout")
        ));
    }

    #[test]
    fn rejects_bad_config() {
        let mut c = cfg(Role::Baby, &[1u8; 32]);
        c.room_id = String::new();
        assert_eq!(Session::new(c).err(), Some(ConfigError::InvalidRoomId));
        let mut c = cfg(Role::Baby, &[1u8; 32]);
        c.psk_b64url = Some("%%%".into());
        assert_eq!(Session::new(c).err(), Some(ConfigError::InvalidPsk));
        let mut c = cfg(Role::Baby, &[1u8; 32]);
        c.heartbeat_stale_ms = 0;
        assert_eq!(Session::new(c).err(), Some(ConfigError::InvalidTiming));
    }

    #[test]
    fn fuzzed_inputs_never_panic() {
        use proptest::prelude::*;
        proptest!(|(texts in proptest::collection::vec(".{0,200}", 0..20))| {
            let mut s = Session::new(cfg(Role::Parent, &[1u8; 32])).unwrap();
            for (i, t) in texts.into_iter().enumerate() {
                s.handle(i as u64, Input::SignalingMessage { text: t.clone() });
                s.handle(i as u64, Input::DataChannelMessage { text: t });
            }
        });
    }
}
