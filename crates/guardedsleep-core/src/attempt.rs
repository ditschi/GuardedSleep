//! One peer-connection lifetime (ADR-017 §2): DataChannel pairing proof, heartbeats and liveness.
//! An [`Attempt`] knows nothing about signaling membership and never retries; the owner (`Link`
//! or the legacy `Session`) creates a new attempt after this one is finished.

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

use crate::pairing::{
    extract_sha256_fingerprint, macs_equal, normalize_dtls_fingerprint, pair_proof_mac_v3,
};
use crate::protocol::{
    DataMessage, IceCandidate, Role, SignalPayload, encode_data_message, parse_data_message,
};

/// An attempt that is not verified within this time is abandoned (ADR-017 §6).
pub const DEFAULT_ATTEMPT_TIMEOUT_MS: u64 = 20_000;

#[derive(Debug, Clone)]
pub struct AttemptConfig {
    pub role: Role,
    /// `None` is the development bypass of the legacy `Session`; a `Link` always has a PSK.
    pub psk: Option<Vec<u8>>,
    pub room: String,
    pub handle: u32,
    pub own_aid: String,
    /// Pre-agreed remote `aid` (from `request_offer`); otherwise learned from the first SDP.
    pub remote_aid: Option<String>,
    /// Baby: the `request_offer` id this attempt answers; echoed as `re` in the offer.
    pub reply_to: Option<String>,
    pub heartbeat_interval_ms: u64,
    pub liveness_ms: u64,
    pub attempt_timeout_ms: u64,
    /// Baby only: refuse a local SDP whose audio m-line is not `a=sendonly` (ADR-018 silence).
    pub require_sendonly: bool,
    /// Verification additionally needs the platform to report the remote DTLS certificate
    /// fingerprint (`on_dtls_fingerprint`) and it must equal the remote SDP fingerprint the proof
    /// was computed over. The legacy `Session` has no such report and sets this to `false`.
    pub require_dtls_fingerprint: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AttemptEvent {
    /// A local SDP / ICE payload for the owner to address and relay.
    Signal(SignalPayload),
    /// Text to send on the DataChannel.
    SendData(String),
    /// Pairing proof verified; heartbeats and application messages are now trusted.
    Verified,
    /// Application message from the verified peer (never `pair_proof`).
    Message(DataMessage),
    /// The peer's proof did not match. The attempt is finished.
    Mismatch,
    /// The DTLS certificate fingerprint reported by the platform differs from the one in the
    /// remote SDP (or is malformed): a man in the middle terminates DTLS. Finished.
    DtlsMismatch,
    /// A verified attempt saw no DataChannel message for `liveness_ms`. Finished.
    LivenessLost,
    /// The attempt failed for the given diagnostic reason. Finished.
    Failed(String),
}

#[derive(Debug)]
pub struct Attempt {
    cfg: AttemptConfig,
    created_at: u64,
    finished: bool,
    local_sdp: Option<String>,
    remote_sdp: Option<String>,
    remote_aid: Option<String>,
    dc_open: bool,
    proof_sent: bool,
    expected_peer_mac: Option<String>,
    early_peer_mac: Option<String>,
    /// Remote SDP fingerprint the proofs were computed over.
    remote_fp: Option<String>,
    /// First DTLS certificate fingerprint reported by the platform (normalized).
    dtls_fp: Option<String>,
    proof_matched: bool,
    verified: bool,
    last_alive: u64,
    seq: u64,
    next_heartbeat_at: u64,
    heartbeat_rms: Option<f64>,
}

impl Attempt {
    pub fn new(cfg: AttemptConfig, now: u64) -> Self {
        let remote_aid = cfg.remote_aid.clone();
        Self {
            cfg,
            created_at: now,
            finished: false,
            local_sdp: None,
            remote_sdp: None,
            remote_aid,
            dc_open: false,
            proof_sent: false,
            expected_peer_mac: None,
            early_peer_mac: None,
            remote_fp: None,
            dtls_fp: None,
            proof_matched: false,
            verified: false,
            last_alive: now,
            seq: 0,
            next_heartbeat_at: 0,
            heartbeat_rms: None,
        }
    }

    pub fn handle(&self) -> u32 {
        self.cfg.handle
    }

    pub fn role(&self) -> Role {
        self.cfg.role
    }

    pub fn own_aid(&self) -> &str {
        &self.cfg.own_aid
    }

    pub fn remote_aid(&self) -> Option<&str> {
        self.remote_aid.as_deref()
    }

    /// True once the proof was verified, also after the attempt finished later.
    pub fn is_verified(&self) -> bool {
        self.verified
    }

    pub fn created_at(&self) -> u64 {
        self.created_at
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Time of the last DataChannel message from the peer (or of verification).
    pub fn last_alive(&self) -> u64 {
        self.last_alive
    }

    /// Both SDPs were exchanged (offer and answer set).
    pub fn sdp_exchanged(&self) -> bool {
        self.local_sdp.is_some() && self.remote_sdp.is_some()
    }

    /// Baby: RMS to put into outgoing heartbeats.
    pub fn set_heartbeat_rms(&mut self, rms: Option<f64>) {
        self.heartbeat_rms = rms;
    }

    /// Whether an ICE candidate carrying `aid` belongs to this attempt's remote side. Once the
    /// attempt is verified negotiation is over and further candidates are ignored.
    pub fn accepts_ice(&self, aid: &str) -> bool {
        !self.finished && !self.verified && self.remote_aid.as_deref() == Some(aid)
    }

    pub fn on_local_description(
        &mut self,
        now: u64,
        sdp_type: &str,
        sdp: &str,
    ) -> Vec<AttemptEvent> {
        let mut out = Vec::new();
        let expected = match self.cfg.role {
            Role::Baby => "offer",
            Role::Parent => "answer",
        };
        if self.finished || self.local_sdp.is_some() || sdp_type != expected {
            return out;
        }
        if self.cfg.role == Role::Baby && self.cfg.require_sendonly && !audio_is_sendonly(sdp) {
            self.fail("baby_sdp_not_sendonly", &mut out);
            return out;
        }
        self.local_sdp = Some(sdp.to_owned());
        out.push(AttemptEvent::Signal(SignalPayload::Sdp {
            aid: self.cfg.own_aid.clone(),
            sdp_type: sdp_type.to_owned(),
            sdp: sdp.to_owned(),
            re: if sdp_type == "offer" {
                self.cfg.reply_to.clone()
            } else {
                None
            },
        }));
        self.try_start_pairing(now, &mut out);
        out
    }

    pub fn on_local_ice(&mut self, candidate: IceCandidate) -> Vec<AttemptEvent> {
        if self.finished {
            return Vec::new();
        }
        vec![AttemptEvent::Signal(SignalPayload::Ice {
            aid: self.cfg.own_aid.clone(),
            candidate,
        })]
    }

    /// `None` rejects the SDP (wrong type, wrong or own `aid`, or one was already accepted); the
    /// caller must then not apply it to the peer connection.
    pub fn on_remote_description(
        &mut self,
        now: u64,
        aid: &str,
        sdp_type: &str,
        sdp: &str,
    ) -> Option<Vec<AttemptEvent>> {
        let expected = match self.cfg.role {
            Role::Parent => "offer",
            Role::Baby => "answer",
        };
        if self.finished
            || self.remote_sdp.is_some()
            || sdp_type != expected
            || aid == self.cfg.own_aid
            || self.remote_aid.as_deref().is_some_and(|a| a != aid)
        {
            return None;
        }
        self.remote_aid = Some(aid.to_owned());
        self.remote_sdp = Some(sdp.to_owned());
        let mut out = Vec::new();
        self.try_start_pairing(now, &mut out);
        Some(out)
    }

    pub fn on_data_open(&mut self, now: u64) -> Vec<AttemptEvent> {
        let mut out = Vec::new();
        if self.finished {
            return out;
        }
        self.dc_open = true;
        if self.cfg.psk.is_none() {
            self.verify(now, &mut out);
        } else {
            self.try_start_pairing(now, &mut out);
        }
        out
    }

    pub fn on_data_message(&mut self, now: u64, text: &str) -> Vec<AttemptEvent> {
        let mut out = Vec::new();
        if self.finished {
            return out;
        }
        if self.verified {
            self.last_alive = now;
        }
        let Some(msg) = parse_data_message(text) else {
            return out;
        };
        if let DataMessage::PairProof { mac } = msg {
            if self.cfg.psk.is_none() || self.verified || self.proof_matched {
                return out;
            }
            match self.expected_peer_mac.clone() {
                None => {
                    self.early_peer_mac.get_or_insert(mac);
                }
                Some(expected) => self.check_proof(now, &expected, &mac, &mut out),
            }
            return out;
        }
        // Nothing but pair_proof is trusted before the PSK binding succeeded.
        if self.verified {
            out.push(AttemptEvent::Message(msg));
        }
        out
    }

    /// The platform read the remote certificate's SHA-256 fingerprint from the WebRTC stats after
    /// DTLS connected. Only the first report counts. A report that differs from the remote SDP
    /// fingerprint (or is malformed) finishes the attempt; a matching one is the second half of
    /// verification when `require_dtls_fingerprint` is set.
    pub fn on_dtls_fingerprint(&mut self, now: u64, fingerprint: &str) -> Vec<AttemptEvent> {
        let mut out = Vec::new();
        if self.finished || self.dtls_fp.is_some() {
            return out;
        }
        let Some(fp) = normalize_dtls_fingerprint(fingerprint) else {
            self.finished = true;
            out.push(AttemptEvent::DtlsMismatch);
            return out;
        };
        self.dtls_fp = Some(fp);
        self.check_dtls(now, &mut out);
        out
    }

    pub fn on_data_closed(&mut self) -> Vec<AttemptEvent> {
        let mut out = Vec::new();
        self.dc_open = false;
        self.fail("data_channel_closed", &mut out);
        out
    }

    /// `RTCPeerConnectionState` as lowercase string.
    pub fn on_pc_state(&mut self, state: &str) -> Vec<AttemptEvent> {
        let mut out = Vec::new();
        if matches!(state, "failed" | "closed") {
            self.fail("peer_connection_failed", &mut out);
        }
        out
    }

    pub fn tick(&mut self, now: u64) -> Vec<AttemptEvent> {
        let mut out = Vec::new();
        if self.finished {
            return out;
        }
        if !self.verified {
            if now.saturating_sub(self.created_at) >= self.cfg.attempt_timeout_ms {
                self.fail("attempt_timeout", &mut out);
            }
            return out;
        }
        if now.saturating_sub(self.last_alive) > self.cfg.liveness_ms {
            self.finished = true;
            out.push(AttemptEvent::LivenessLost);
            return out;
        }
        if self.dc_open && now >= self.next_heartbeat_at {
            self.send_heartbeat(now, &mut out);
        }
        out
    }

    /// Sends an application message on a verified attempt; `None` before verification.
    pub fn encode_app_message(&self, msg: &DataMessage) -> Option<String> {
        (self.verified && self.dc_open && !self.finished).then(|| encode_data_message(msg))
    }

    fn try_start_pairing(&mut self, now: u64, out: &mut Vec<AttemptEvent>) {
        let Some(psk) = self.cfg.psk.as_deref() else {
            return;
        };
        if !self.dc_open || self.proof_sent || self.verified || self.finished {
            return;
        }
        let (Some(local), Some(remote)) = (self.local_sdp.as_deref(), self.remote_sdp.as_deref())
        else {
            return;
        };
        let (Some(fp_local), Some(fp_remote)) = (
            extract_sha256_fingerprint(local),
            extract_sha256_fingerprint(remote),
        ) else {
            self.fail("fingerprint_invalid", out);
            return;
        };
        let peer_role = match self.cfg.role {
            Role::Baby => Role::Parent,
            Role::Parent => Role::Baby,
        };
        let room = self.cfg.room.as_str();
        let own = pair_proof_mac_v3(psk, self.cfg.role, room, &fp_local, &fp_remote);
        let expected = pair_proof_mac_v3(psk, peer_role, room, &fp_remote, &fp_local);
        let (Some(own), Some(expected)) = (own, expected) else {
            self.fail("invalid_psk", out);
            return;
        };
        out.push(AttemptEvent::SendData(encode_data_message(
            &DataMessage::PairProof { mac: own },
        )));
        self.proof_sent = true;
        self.expected_peer_mac = Some(expected.clone());
        self.remote_fp = Some(fp_remote);
        // A DTLS report that arrived before the SDPs were complete is checked now.
        if self.dtls_fp.is_some() {
            self.check_dtls(now, out);
            if self.finished {
                return;
            }
        }
        if let Some(early) = self.early_peer_mac.take() {
            self.check_proof(now, &expected, &early, out);
        }
    }

    /// Compares the reported DTLS fingerprint with the remote SDP one as soon as both are known.
    fn check_dtls(&mut self, now: u64, out: &mut Vec<AttemptEvent>) {
        let (Some(reported), Some(remote)) = (self.dtls_fp.as_deref(), self.remote_fp.as_deref())
        else {
            return;
        };
        if reported != remote {
            self.finished = true;
            out.push(AttemptEvent::DtlsMismatch);
            return;
        }
        self.try_verify(now, out);
    }

    /// Verified = the peer's proof matched, and (if required) the DTLS fingerprint matched too.
    fn try_verify(&mut self, now: u64, out: &mut Vec<AttemptEvent>) {
        if self.verified || self.finished || !self.proof_matched {
            return;
        }
        if self.cfg.require_dtls_fingerprint && self.dtls_fp.is_none() {
            return;
        }
        self.verify(now, out);
    }

    fn check_proof(
        &mut self,
        now: u64,
        expected: &str,
        received: &str,
        out: &mut Vec<AttemptEvent>,
    ) {
        if macs_equal(expected, received) {
            self.proof_matched = true;
            self.try_verify(now, out);
        } else {
            self.finished = true;
            out.push(AttemptEvent::Mismatch);
        }
    }

    fn verify(&mut self, now: u64, out: &mut Vec<AttemptEvent>) {
        self.verified = true;
        self.last_alive = now;
        out.push(AttemptEvent::Verified);
        self.send_heartbeat(now, out);
    }

    fn send_heartbeat(&mut self, now: u64, out: &mut Vec<AttemptEvent>) {
        self.seq += 1;
        self.next_heartbeat_at = now + self.cfg.heartbeat_interval_ms;
        out.push(AttemptEvent::SendData(encode_data_message(
            &DataMessage::Heartbeat {
                t: now as f64,
                seq: self.seq,
                battery: None,
                rms: self.heartbeat_rms,
            },
        )));
    }

    fn fail(&mut self, reason: &str, out: &mut Vec<AttemptEvent>) {
        if self.finished {
            return;
        }
        self.finished = true;
        out.push(AttemptEvent::Failed(reason.to_owned()));
    }
}

fn section_is_sendonly(dir: Option<&str>, conflicting: bool) -> bool {
    !conflicting && dir == Some("sendonly")
}

/// Minimal SDP check: at least one audio m-line and every audio m-line is `a=sendonly` (its own
/// direction attribute, or the session-level one when it has none). Anything ambiguous fails.
pub fn audio_is_sendonly(sdp: &str) -> bool {
    let mut session_dir: Option<&str> = None;
    let mut in_audio = false;
    let mut in_media = false;
    let mut section_dir: Option<&str> = None;
    let mut conflicting = false;
    let mut audio_sections = 0usize;
    let mut bad = false;
    for raw in sdp.split('\n') {
        let line = raw.trim_end_matches('\r');
        if line.starts_with("m=") {
            if in_audio {
                bad |= !section_is_sendonly(section_dir.or(session_dir), conflicting);
            }
            in_media = true;
            in_audio = line.starts_with("m=audio ");
            if in_audio {
                audio_sections += 1;
            }
            section_dir = None;
            conflicting = false;
            continue;
        }
        let dir = match line {
            "a=sendonly" => Some("sendonly"),
            "a=recvonly" => Some("recvonly"),
            "a=sendrecv" => Some("sendrecv"),
            "a=inactive" => Some("inactive"),
            _ => None,
        };
        let Some(dir) = dir else { continue };
        if !in_media {
            session_dir = Some(dir);
        } else if in_audio {
            if section_dir.is_some_and(|d| d != dir) {
                conflicting = true;
            }
            section_dir = Some(dir);
        }
    }
    if in_audio {
        bad |= !section_is_sendonly(section_dir.or(session_dir), conflicting);
    }
    audio_sections > 0 && !bad
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pairing::pair_proof_mac_v3;

    const PSK: [u8; 32] = [9u8; 32];
    const BABY_AID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const PARENT_AID: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    /// Colon-separated upper-case fingerprint (32 bytes) as in an SDP.
    fn fpc(seed: u8) -> String {
        (0..32u8)
            .map(|i| format!("{:02X}", seed.wrapping_add(i)))
            .collect::<Vec<_>>()
            .join(":")
    }

    /// The same fingerprint as the core normalizes it.
    fn fph(seed: u8) -> String {
        fpc(seed).replace(':', "").to_ascii_lowercase()
    }

    fn sdp(fp: &str) -> String {
        format!(
            "v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=sendonly\r\na=fingerprint:sha-256 {fp}\r\n"
        )
    }

    fn cfg(role: Role, psk: &[u8]) -> AttemptConfig {
        AttemptConfig {
            role,
            psk: Some(psk.to_vec()),
            room: "room".into(),
            handle: 1,
            own_aid: match role {
                Role::Baby => BABY_AID.into(),
                Role::Parent => PARENT_AID.into(),
            },
            remote_aid: None,
            reply_to: None,
            heartbeat_interval_ms: 5_000,
            liveness_ms: 15_000,
            attempt_timeout_ms: 20_000,
            require_sendonly: true,
            require_dtls_fingerprint: false,
        }
    }

    fn texts(ev: &[AttemptEvent]) -> Vec<String> {
        ev.iter()
            .filter_map(|e| match e {
                AttemptEvent::SendData(t) => Some(t.clone()),
                _ => None,
            })
            .collect()
    }

    fn proof_text(mac: &str) -> String {
        encode_data_message(&DataMessage::PairProof { mac: mac.into() })
    }

    /// Wires a baby and a parent attempt up to an open DataChannel; returns the proofs each sent.
    fn open_pair(baby_psk: &[u8], parent_psk: &[u8]) -> (Attempt, Attempt, String, String) {
        let mut baby = Attempt::new(cfg(Role::Baby, baby_psk), 0);
        let mut parent = Attempt::new(cfg(Role::Parent, parent_psk), 0);
        let (bs, ps) = (sdp(&fpc(0xaa)), sdp(&fpc(0xcc)));
        baby.on_local_description(1, "offer", &bs);
        parent
            .on_remote_description(2, BABY_AID, "offer", &bs)
            .unwrap();
        parent.on_local_description(3, "answer", &ps);
        baby.on_remote_description(4, PARENT_AID, "answer", &ps)
            .unwrap();
        let bp = texts(&baby.on_data_open(5)).pop().unwrap();
        let pp = texts(&parent.on_data_open(5)).pop().unwrap();
        (baby, parent, bp, pp)
    }

    #[test]
    fn matching_psk_verifies_both_sides_and_both_send_heartbeats() {
        let (mut baby, mut parent, bp, pp) = open_pair(&PSK, &PSK);
        let pe = parent.on_data_message(6, &bp);
        let be = baby.on_data_message(6, &pp);
        assert!(pe.contains(&AttemptEvent::Verified) && be.contains(&AttemptEvent::Verified));
        assert!(texts(&pe).iter().any(|t| t.contains("\"heartbeat\"")));
        assert!(texts(&be).iter().any(|t| t.contains("\"heartbeat\"")));
        assert!(baby.is_verified() && parent.is_verified());
    }

    #[test]
    fn wrong_psk_is_a_mismatch_and_finishes_the_attempt() {
        let (mut baby, mut parent, bp, pp) = open_pair(&[1u8; 32], &[2u8; 32]);
        assert_eq!(parent.on_data_message(6, &bp), vec![AttemptEvent::Mismatch]);
        assert_eq!(baby.on_data_message(6, &pp), vec![AttemptEvent::Mismatch]);
        assert!(!baby.is_verified() && baby.is_finished());
        assert!(baby.tick(100_000).is_empty());
    }

    #[test]
    fn reflected_proof_never_verifies() {
        let (mut baby, _parent, bp, _pp) = open_pair(&PSK, &PSK);
        // An attacker that only knows the room reflects the baby's own proof back to it.
        assert_eq!(baby.on_data_message(6, &bp), vec![AttemptEvent::Mismatch]);
        assert!(!baby.is_verified());

        let (mut baby, mut parent, bp, pp) = open_pair(&PSK, &PSK);
        assert_eq!(parent.on_data_message(6, &pp), vec![AttemptEvent::Mismatch]);
        let _ = (&mut baby, bp);
    }

    #[test]
    fn proof_is_bound_to_this_attempts_fingerprints() {
        let (_b, mut parent, _bp, _pp) = open_pair(&PSK, &PSK);
        let replay = pair_proof_mac_v3(&PSK, Role::Baby, "room", "ffff", "eeee").unwrap();
        assert_eq!(
            parent.on_data_message(6, &proof_text(&replay)),
            vec![AttemptEvent::Mismatch]
        );
    }

    #[test]
    fn early_peer_proof_is_buffered_until_own_proof_can_be_computed() {
        let mut baby = Attempt::new(cfg(Role::Baby, &PSK), 0);
        let mut parent = Attempt::new(cfg(Role::Parent, &PSK), 0);
        let (bs, ps) = (sdp(&fpc(0xaa)), sdp(&fpc(0xcc)));
        baby.on_local_description(1, "offer", &bs);
        parent
            .on_remote_description(1, BABY_AID, "offer", &bs)
            .unwrap();
        parent.on_local_description(1, "answer", &ps);
        let pp = texts(&parent.on_data_open(2)).pop().unwrap();
        // The parent's proof arrives at the baby before the baby has the answer SDP.
        assert!(baby.on_data_message(3, &pp).is_empty());
        baby.on_data_open(3);
        let ev = baby
            .on_remote_description(4, PARENT_AID, "answer", &ps)
            .unwrap();
        assert!(ev.contains(&AttemptEvent::Verified));
    }

    #[test]
    fn only_the_first_early_proof_is_kept() {
        let mut baby = Attempt::new(cfg(Role::Baby, &PSK), 0);
        let (bs, ps) = (sdp(&fpc(0xaa)), sdp(&fpc(0xcc)));
        baby.on_local_description(1, "offer", &bs);
        baby.on_data_open(1);
        let good = pair_proof_mac_v3(&PSK, Role::Parent, "room", &fph(0xcc), &fph(0xaa)).unwrap();
        baby.on_data_message(2, &proof_text(&"0".repeat(64)));
        baby.on_data_message(2, &proof_text(&good));
        let ev = baby
            .on_remote_description(3, PARENT_AID, "answer", &ps)
            .unwrap();
        assert!(ev.contains(&AttemptEvent::Mismatch));
    }

    #[test]
    fn app_messages_before_verification_are_dropped_and_do_not_count_as_alive() {
        let mut parent = Attempt::new(cfg(Role::Parent, &PSK), 0);
        let ev = parent.on_data_message(1, r#"{"type":"noise_alert","v":3,"rms":0.9,"t":1}"#);
        assert!(ev.is_empty());
    }

    #[test]
    fn verified_messages_are_delivered_and_keep_the_attempt_alive() {
        let (mut baby, mut parent, bp, pp) = open_pair(&PSK, &PSK);
        parent.on_data_message(6, &bp);
        baby.on_data_message(6, &pp);
        let hb = r#"{"type":"heartbeat","v":3,"t":1,"seq":1}"#;
        let ev = parent.on_data_message(10_000, hb);
        assert!(matches!(
            ev.as_slice(),
            [AttemptEvent::Message(DataMessage::Heartbeat { .. })]
        ));
        assert_eq!(parent.last_alive(), 10_000);
        assert!(
            parent
                .tick(24_000)
                .iter()
                .all(|e| *e != AttemptEvent::LivenessLost)
        );
        assert_eq!(parent.tick(25_001), vec![AttemptEvent::LivenessLost]);
        assert!(parent.is_finished() && parent.is_verified());
    }

    #[test]
    fn liveness_counts_any_datachannel_message() {
        let (mut baby, mut parent, bp, pp) = open_pair(&PSK, &PSK);
        parent.on_data_message(6, &bp);
        baby.on_data_message(6, &pp);
        parent.on_data_message(12_000, "garbage");
        assert_eq!(parent.last_alive(), 12_000);
    }

    #[test]
    fn heartbeats_are_sent_on_schedule_by_both_roles() {
        let (mut baby, mut parent, bp, pp) = open_pair(&PSK, &PSK);
        parent.on_data_message(6, &bp);
        baby.on_data_message(6, &pp);
        // Verified at t = 6 with an immediate heartbeat; the next one is due 5 s later.
        for a in [&mut baby, &mut parent] {
            assert!(texts(&a.tick(5_005)).is_empty());
            assert_eq!(texts(&a.tick(5_006)).len(), 1);
            assert!(texts(&a.tick(10_005)).is_empty());
            assert_eq!(texts(&a.tick(10_006)).len(), 1);
        }
    }

    #[test]
    fn baby_heartbeat_carries_rms() {
        let (mut baby, mut parent, bp, pp) = open_pair(&PSK, &PSK);
        parent.on_data_message(6, &bp);
        baby.on_data_message(6, &pp);
        baby.set_heartbeat_rms(Some(0.25));
        let t = texts(&baby.tick(5_006)).pop().unwrap();
        assert!(t.contains("\"rms\":0.25"));
    }

    #[test]
    fn unverified_attempt_times_out() {
        let mut a = Attempt::new(cfg(Role::Baby, &PSK), 100);
        assert!(a.tick(20_099).is_empty());
        assert_eq!(
            a.tick(20_100),
            vec![AttemptEvent::Failed("attempt_timeout".into())]
        );
        assert!(a.is_finished());
        assert!(a.tick(30_000).is_empty());
    }

    #[test]
    fn remote_description_filters_type_aid_and_duplicates() {
        let mut parent = Attempt::new(cfg(Role::Parent, &PSK), 0);
        let s = sdp(&fpc(0xaa));
        assert!(
            parent
                .on_remote_description(0, BABY_AID, "answer", &s)
                .is_none()
        );
        assert!(
            parent
                .on_remote_description(0, PARENT_AID, "offer", &s)
                .is_none()
        );
        assert!(
            parent
                .on_remote_description(0, BABY_AID, "offer", &s)
                .is_some()
        );
        assert!(
            parent
                .on_remote_description(0, BABY_AID, "offer", &s)
                .is_none()
        );
        assert_eq!(parent.remote_aid(), Some(BABY_AID));

        let mut c = cfg(Role::Baby, &PSK);
        c.remote_aid = Some(PARENT_AID.into());
        let mut baby = Attempt::new(c, 0);
        let other = "cccccccccccccccccccccccccccccccc";
        assert!(baby.on_remote_description(0, other, "answer", &s).is_none());
        assert!(
            baby.on_remote_description(0, PARENT_AID, "answer", &s)
                .is_some()
        );
    }

    #[test]
    fn ice_is_filtered_by_remote_aid() {
        let mut parent = Attempt::new(cfg(Role::Parent, &PSK), 0);
        assert!(!parent.accepts_ice(BABY_AID));
        parent
            .on_remote_description(0, BABY_AID, "offer", &sdp(&fpc(0xaa)))
            .unwrap();
        assert!(parent.accepts_ice(BABY_AID));
        assert!(!parent.accepts_ice("cccccccccccccccccccccccccccccccc"));
    }

    #[test]
    fn ice_is_ignored_once_the_attempt_is_verified() {
        let (mut baby, mut parent, bp, pp) = open_pair(&PSK, &PSK);
        assert!(parent.accepts_ice(BABY_AID));
        parent.on_data_message(6, &bp);
        baby.on_data_message(6, &pp);
        assert!(parent.is_verified());
        assert!(!parent.accepts_ice(BABY_AID));
        assert!(!baby.accepts_ice(PARENT_AID));
    }

    #[test]
    fn local_sdp_is_signalled_with_own_aid() {
        let mut baby = Attempt::new(cfg(Role::Baby, &PSK), 0);
        let ev = baby.on_local_description(0, "offer", &sdp(&fpc(0xaa)));
        assert!(matches!(
            ev.as_slice(),
            [AttemptEvent::Signal(SignalPayload::Sdp { aid, .. })] if aid == BABY_AID
        ));
        assert!(
            baby.on_local_description(0, "offer", &sdp(&fpc(0xaa)))
                .is_empty()
        );
        let ice = baby.on_local_ice(IceCandidate {
            candidate: "c".into(),
            sdp_mid: None,
            sdp_m_line_index: None,
        });
        assert!(matches!(
            ice.as_slice(),
            [AttemptEvent::Signal(SignalPayload::Ice { aid, .. })] if aid == BABY_AID
        ));
    }

    #[test]
    fn baby_refuses_local_sdp_that_is_not_sendonly() {
        let fp = fpc(0xaa);
        for body in [
            format!(
                "v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=sendrecv\r\na=fingerprint:sha-256 {fp}\r\n"
            ),
            format!("v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=fingerprint:sha-256 {fp}\r\n"),
            format!("v=0\r\na=fingerprint:sha-256 {fp}\r\n"),
        ] {
            let mut baby = Attempt::new(cfg(Role::Baby, &PSK), 0);
            let ev = baby.on_local_description(0, "offer", &body);
            assert_eq!(
                ev,
                vec![AttemptEvent::Failed("baby_sdp_not_sendonly".into())]
            );
            assert!(baby.is_finished());
        }
    }

    #[test]
    fn sendonly_check_can_be_disabled_for_the_legacy_session() {
        let mut c = cfg(Role::Baby, &PSK);
        c.require_sendonly = false;
        let mut baby = Attempt::new(c, 0);
        let ev = baby.on_local_description(0, "offer", "v=0\r\n");
        assert!(matches!(ev.as_slice(), [AttemptEvent::Signal(_)]));
    }

    #[test]
    fn sendonly_parser_handles_sections_and_session_level_direction() {
        let m = "m=audio 9 UDP/TLS/RTP/SAVPF 111\r\n";
        assert!(audio_is_sendonly(&format!("v=0\r\n{m}a=sendonly\r\n")));
        assert!(audio_is_sendonly(&format!("v=0\r\na=sendonly\r\n{m}")));
        assert!(!audio_is_sendonly(&format!(
            "v=0\r\na=sendonly\r\n{m}a=sendrecv\r\n"
        )));
        assert!(!audio_is_sendonly(&format!(
            "v=0\r\n{m}a=sendonly\r\na=recvonly\r\n"
        )));
        assert!(!audio_is_sendonly(&format!("v=0\r\n{m}a=inactive\r\n")));
        assert!(!audio_is_sendonly("v=0\r\n"));
        // A second audio section that is not sendonly poisons the whole SDP.
        assert!(!audio_is_sendonly(&format!(
            "v=0\r\n{m}a=sendonly\r\n{m}a=sendrecv\r\n"
        )));
        // A video section is not our concern; the audio section still decides.
        assert!(audio_is_sendonly(&format!(
            "v=0\r\n{m}a=sendonly\r\nm=video 9 X 96\r\na=sendrecv\r\n"
        )));
        // LF-only line endings are accepted too.
        assert!(audio_is_sendonly("v=0\nm=audio 9 X 1\na=sendonly\n"));
    }

    #[test]
    fn data_channel_close_and_pc_failure_finish_the_attempt() {
        let mut a = Attempt::new(cfg(Role::Parent, &PSK), 0);
        assert!(a.on_pc_state("connected").is_empty());
        assert!(a.on_pc_state("disconnected").is_empty());
        assert_eq!(
            a.on_pc_state("failed"),
            vec![AttemptEvent::Failed("peer_connection_failed".into())]
        );
        assert!(a.on_pc_state("closed").is_empty());

        let mut b = Attempt::new(cfg(Role::Parent, &PSK), 0);
        assert_eq!(
            b.on_data_closed(),
            vec![AttemptEvent::Failed("data_channel_closed".into())]
        );
    }

    #[test]
    fn missing_or_ambiguous_fingerprints_fail_the_attempt() {
        let two = format!("{}a=fingerprint:sha-256 {}\r\n", sdp(&fpc(0xaa)), fpc(0xbb));
        let hidden = format!(
            "v=0\r\ns=a=fingerprint:sha-256 {}\r\nm=audio 9 X 1\r\na=sendonly\r\n",
            fpc(0xaa)
        );
        for remote in ["v=0\r\n".to_owned(), two, hidden] {
            let mut parent = Attempt::new(cfg(Role::Parent, &PSK), 0);
            parent
                .on_remote_description(0, BABY_AID, "offer", &remote)
                .unwrap();
            parent.on_local_description(0, "answer", &sdp(&fpc(0xcc)));
            let ev = parent.on_data_open(1);
            assert_eq!(ev, vec![AttemptEvent::Failed("fingerprint_invalid".into())]);
        }
        // The local SDP is held to the same rule.
        let mut parent = Attempt::new(cfg(Role::Parent, &PSK), 0);
        parent
            .on_remote_description(0, BABY_AID, "offer", &sdp(&fpc(0xaa)))
            .unwrap();
        parent.on_local_description(0, "answer", "v=0\r\n");
        assert_eq!(
            parent.on_data_open(1),
            vec![AttemptEvent::Failed("fingerprint_invalid".into())]
        );
    }

    #[test]
    fn dev_bypass_verifies_on_data_channel_open() {
        let mut c = cfg(Role::Parent, &PSK);
        c.psk = None;
        let mut a = Attempt::new(c, 0);
        assert!(a.on_data_open(1).contains(&AttemptEvent::Verified));
    }

    fn dtls_cfg(role: Role) -> AttemptConfig {
        AttemptConfig {
            require_dtls_fingerprint: true,
            ..cfg(role, &PSK)
        }
    }

    /// Both sides with `require_dtls_fingerprint`, DataChannel open and proofs sent.
    fn open_dtls_pair() -> (Attempt, Attempt, String, String) {
        let mut baby = Attempt::new(dtls_cfg(Role::Baby), 0);
        let mut parent = Attempt::new(dtls_cfg(Role::Parent), 0);
        let (bs, ps) = (sdp(&fpc(0xaa)), sdp(&fpc(0xcc)));
        baby.on_local_description(1, "offer", &bs);
        parent
            .on_remote_description(2, BABY_AID, "offer", &bs)
            .unwrap();
        parent.on_local_description(3, "answer", &ps);
        baby.on_remote_description(4, PARENT_AID, "answer", &ps)
            .unwrap();
        let bp = texts(&baby.on_data_open(5)).pop().unwrap();
        let pp = texts(&parent.on_data_open(5)).pop().unwrap();
        (baby, parent, bp, pp)
    }

    #[test]
    fn proof_alone_does_not_verify_when_a_dtls_fingerprint_is_required() {
        let (mut baby, mut parent, bp, pp) = open_dtls_pair();
        assert!(parent.on_data_message(6, &bp).is_empty());
        assert!(baby.on_data_message(6, &pp).is_empty());
        assert!(!parent.is_verified() && !baby.is_verified());
        // Without the report the attempt times out like any unverified one.
        assert_eq!(
            parent.tick(20_000),
            vec![AttemptEvent::Failed("attempt_timeout".into())]
        );
    }

    #[test]
    fn matching_dtls_fingerprint_completes_verification_in_either_order() {
        // Proof first, report second (colon form as WebRTC stats deliver it).
        let (mut baby, mut parent, bp, pp) = open_dtls_pair();
        parent.on_data_message(6, &bp);
        baby.on_data_message(6, &pp);
        let ev = parent.on_dtls_fingerprint(7, &fpc(0xaa));
        assert!(ev.contains(&AttemptEvent::Verified));
        assert!(texts(&ev).iter().any(|t| t.contains("\"heartbeat\"")));
        let ev = baby.on_dtls_fingerprint(7, &fph(0xcc));
        assert!(ev.contains(&AttemptEvent::Verified));
        assert!(parent.is_verified() && baby.is_verified());

        // Report first, proof second.
        let (mut baby, mut parent, bp, pp) = open_dtls_pair();
        assert!(parent.on_dtls_fingerprint(6, &fpc(0xaa)).is_empty());
        assert!(baby.on_dtls_fingerprint(6, &fpc(0xcc)).is_empty());
        assert!(
            parent
                .on_data_message(7, &bp)
                .contains(&AttemptEvent::Verified)
        );
        assert!(
            baby.on_data_message(7, &pp)
                .contains(&AttemptEvent::Verified)
        );
    }

    #[test]
    fn dtls_report_before_the_sdps_are_complete_is_kept() {
        let mut baby = Attempt::new(dtls_cfg(Role::Baby), 0);
        let (bs, ps) = (sdp(&fpc(0xaa)), sdp(&fpc(0xcc)));
        baby.on_local_description(1, "offer", &bs);
        assert!(baby.on_dtls_fingerprint(2, &fpc(0xcc)).is_empty());
        baby.on_data_open(3);
        baby.on_remote_description(4, PARENT_AID, "answer", &ps)
            .unwrap();
        let good = pair_proof_mac_v3(&PSK, Role::Parent, "room", &fph(0xcc), &fph(0xaa)).unwrap();
        assert!(
            baby.on_data_message(5, &proof_text(&good))
                .contains(&AttemptEvent::Verified)
        );

        // A kept report that is wrong fails the attempt as soon as the remote SDP is known.
        let mut baby = Attempt::new(dtls_cfg(Role::Baby), 0);
        baby.on_local_description(1, "offer", &bs);
        baby.on_dtls_fingerprint(2, &fpc(0x11));
        baby.on_data_open(3);
        let ev = baby
            .on_remote_description(4, PARENT_AID, "answer", &ps)
            .unwrap();
        assert!(ev.contains(&AttemptEvent::DtlsMismatch));
        assert!(!ev.contains(&AttemptEvent::Verified));
        assert!(baby.is_finished());
    }

    #[test]
    fn dtls_fingerprint_mismatch_never_verifies_even_with_a_matching_proof() {
        for report in [
            fpc(0x11),
            "AA:BB".to_owned(),
            String::new(),
            "zz".repeat(32),
        ] {
            let (mut baby, mut parent, bp, pp) = open_dtls_pair();
            parent.on_data_message(6, &bp);
            baby.on_data_message(6, &pp);
            assert_eq!(
                parent.on_dtls_fingerprint(7, &report),
                vec![AttemptEvent::DtlsMismatch]
            );
            assert!(parent.is_finished() && !parent.is_verified());
            // Nothing revives it, not even the right fingerprint afterwards.
            assert!(parent.on_dtls_fingerprint(8, &fpc(0xaa)).is_empty());
            assert!(parent.on_data_message(9, &bp).is_empty());
            assert!(!parent.is_verified());
        }
    }

    #[test]
    fn only_the_first_dtls_report_counts() {
        let (mut baby, mut parent, bp, pp) = open_dtls_pair();
        assert!(parent.on_dtls_fingerprint(5, &fpc(0xaa)).is_empty());
        assert!(parent.on_dtls_fingerprint(5, &fpc(0x11)).is_empty());
        parent.on_data_message(6, &bp);
        assert!(parent.is_verified());
        let _ = (&mut baby, pp);
    }

    #[test]
    fn dtls_report_is_optional_when_not_required_but_still_checked() {
        let (mut baby, mut parent, bp, pp) = open_pair(&PSK, &PSK);
        parent.on_data_message(6, &bp);
        baby.on_data_message(6, &pp);
        assert!(parent.is_verified());
        let (_b, mut parent2, bp2, _pp) = open_pair(&PSK, &PSK);
        assert_eq!(
            parent2.on_dtls_fingerprint(5, &fpc(0x11)),
            vec![AttemptEvent::DtlsMismatch]
        );
        assert!(parent2.on_data_message(6, &bp2).is_empty());
        assert!(!parent2.is_verified());
    }

    #[test]
    fn fingerprint_confusion_in_the_sdp_never_verifies() {
        // The relay hides the other side's real fingerprint in `s=` and presents its own on the
        // real line; both ends relay the proofs through it. Proofs bind the real lines, so they
        // cannot match, and the DTLS certificate is the relay's.
        let mitm = fpc(0x77);
        let hidden = |real: &str| {
            format!(
                "v=0\r\ns=a=fingerprint:sha-256 {real}\r\nm=audio 9 X 1\r\na=sendonly\r\na=fingerprint:sha-256 {mitm}\r\n"
            )
        };
        let mut baby = Attempt::new(dtls_cfg(Role::Baby), 0);
        let mut parent = Attempt::new(dtls_cfg(Role::Parent), 0);
        let (bs, ps) = (sdp(&fpc(0xaa)), sdp(&fpc(0xcc)));
        baby.on_local_description(1, "offer", &bs);
        parent
            .on_remote_description(2, BABY_AID, "offer", &hidden(&fpc(0xaa)))
            .unwrap();
        parent.on_local_description(3, "answer", &ps);
        baby.on_remote_description(4, PARENT_AID, "answer", &hidden(&fpc(0xcc)))
            .unwrap();
        let bp = texts(&baby.on_data_open(5)).pop().unwrap();
        let pp = texts(&parent.on_data_open(5)).pop().unwrap();
        assert_eq!(parent.on_data_message(6, &bp), vec![AttemptEvent::Mismatch]);
        assert_eq!(baby.on_data_message(6, &pp), vec![AttemptEvent::Mismatch]);
        assert!(!parent.is_verified() && !baby.is_verified());
    }

    #[test]
    fn fuzzed_data_never_panics() {
        use proptest::prelude::*;
        proptest!(|(texts in proptest::collection::vec(".{0,200}", 0..20))| {
            let mut a = Attempt::new(cfg(Role::Parent, &PSK), 0);
            for (i, t) in texts.into_iter().enumerate() {
                a.on_data_message(i as u64, &t);
                a.on_remote_description(i as u64, &t, "offer", &t);
                a.tick(i as u64 * 1000);
            }
        });
    }
}
