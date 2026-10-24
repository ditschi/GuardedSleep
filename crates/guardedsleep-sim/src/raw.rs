//! Raw protocol clients ("strangers"): a third party in the room that speaks the signaling and
//! pairing protocol directly, without a `Link`, to mount the ADR-017 section 12 attacks. The
//! proof-level attackers (`WrongPsk`, `Reflect`) hold the room's PSK, as an attacker who got the
//! invite does, so their sealed envelopes authenticate and only the pairing proof and the DTLS
//! binding stop them. `ForgedOffer` is the outsider without the PSK and sends plaintext junk.

use guardedsleep_core::envelope::MessageClass;
use guardedsleep_core::pairing::{decode_psk, extract_sha256_fingerprint, pair_proof_mac_v3};
use guardedsleep_core::protocol::{
    DataMessage, Role, ServerMessage, SignalPayload, encode_data_message, encode_hello,
    encode_signal, parse_data_message, parse_server_message, parse_signal_payload,
    signal_payload_json,
};
use guardedsleep_core::testing::SealedPeer;
use serde_json::Value;

use crate::scenario::Step;
use crate::world::{Time, ToClient, World, colon_upper, synthetic_psk};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawMode {
    /// Plays the parent, answers the baby's offer and sends a proof made with another PSK.
    WrongPsk,
    /// Plays the parent and echoes the baby's own proof back (reflection attack).
    Reflect,
    /// Sits in the room and sends forged offers, requests, candidates and stop notices.
    ForgedOffer,
}

pub struct RawState {
    pub mode: RawMode,
    pub room: String,
    pub psk: Vec<u8>,
    pub spoof: bool,
    pub act_after_ms: u64,
    pub client_id: Option<String>,
    pub members: Vec<(String, Role)>,
    pub my_aid: Option<String>,
    pub last_request: Option<Time>,
    pub pc: Option<u32>,
    pub baby_fp: Option<String>,
    pub acted: bool,
    /// The PSK holder's sealed side (`WrongPsk`, `Reflect`); `None` for the PSK-less outsider.
    pub peer: Option<SealedPeer>,
    /// The room member the last `here` went to.
    pub greeted: Option<String>,
}

fn class_of(p: &SignalPayload) -> MessageClass {
    match p {
        SignalPayload::Sdp { .. } => MessageClass::Description,
        SignalPayload::Ice { .. } => MessageClass::Candidates,
        SignalPayload::RequestOffer { .. } | SignalPayload::StopNotice => MessageClass::Control,
    }
}

impl World {
    pub(crate) fn stranger_join(&mut self, st: &Step) {
        let mode = match st.mode.as_deref() {
            Some("wrong_psk") => RawMode::WrongPsk,
            Some("reflect") => RawMode::Reflect,
            Some("forged_offer") => RawMode::ForgedOffer,
            other => {
                self.violations
                    .push(format!("stranger_join: unknown mode {other:?}"));
                return;
            }
        };
        let role = st.role.unwrap_or(Role::Parent);
        let name = st.id.clone().unwrap_or_else(|| "stranger".into());
        // The PSK holders use the room's real PSK for envelopes; their proof key stays "B".
        let peer = (mode != RawMode::ForgedOffer)
            .then(|| {
                let room_psk = decode_psk(&self.actors[0].cfg.psk_b64url)?;
                Some(SealedPeer::new(role, &room_psk, 0x61))
            })
            .flatten();
        let raw = RawState {
            mode,
            room: self.target_room(),
            psk: synthetic_psk("B"),
            spoof: st.spoof_from.unwrap_or(false),
            act_after_ms: st.act_after_ms.unwrap_or(3_000),
            client_id: None,
            members: Vec::new(),
            my_aid: None,
            last_request: None,
            pc: None,
            baby_fp: None,
            acted: false,
            peer,
            greeted: None,
        };
        let a = self.add_raw_actor(&name, role, raw);
        self.open_signaling(a);
        self.schedule_raw_tick(a, 2_000);
    }

    pub(crate) fn raw_on_signaling(&mut self, a: usize, msg: ToClient) {
        match msg {
            ToClient::Opened => {
                let (role, room) = (self.actors[a].role, self.actors[a].room.clone());
                self.client_send(a, encode_hello(role, &room), None);
            }
            ToClient::Closed => {}
            ToClient::Text(text) => {
                let Some(m) = parse_server_message(&text) else {
                    return;
                };
                match m {
                    ServerMessage::Welcome { client_id, .. } => {
                        let raw = self.actors[a].raw.as_mut().expect("raw");
                        raw.client_id = Some(client_id);
                        if raw.mode == RawMode::ForgedOffer {
                            let after = raw.act_after_ms;
                            self.schedule_raw_act(a, after);
                        }
                    }
                    ServerMessage::Members { peers } => {
                        let raw = self.actors[a].raw.as_mut().expect("raw");
                        raw.members = peers.into_iter().map(|m| (m.client_id, m.role)).collect();
                        self.raw_greet(a);
                        self.raw_maybe_request(a);
                    }
                    ServerMessage::Signal { from, payload } => {
                        self.raw_on_payload(a, &from, &payload);
                    }
                    _ => {}
                }
            }
        }
    }

    /// One inbound relay `signal`: a PSK holder opens the envelope (and answers the confirmation
    /// handshake); the outsider only understands plaintext.
    fn raw_on_payload(&mut self, a: usize, from: &str, payload: &Value) {
        let now = self.now;
        let has_peer = self.actors[a]
            .raw
            .as_ref()
            .is_some_and(|r| r.peer.is_some());
        if !has_peer {
            if let Some(p) = parse_signal_payload(payload) {
                self.raw_on_signal(a, from, p);
            }
            return;
        }
        let room = self.actors[a].room.clone();
        let opened = {
            let raw = self.actors[a].raw.as_mut().expect("raw");
            raw.peer.as_mut().and_then(|p| p.accept(now, payload).ok())
        };
        let Some((opened, reply)) = opened else {
            return;
        };
        if let Some(reply) = reply {
            let text = encode_signal(&room, Some(from), reply);
            self.client_send(a, text, None);
        }
        if opened.class != MessageClass::Here
            && let Ok(v) = serde_json::from_slice::<Value>(&opened.message)
            && let Some(p) = parse_signal_payload(&v)
        {
            self.raw_on_signal(a, from, p);
        }
    }

    /// A PSK holder announces itself to the baby once per baby socket.
    fn raw_greet(&mut self, a: usize) {
        let Some(baby) = self.raw_baby(a) else {
            return;
        };
        let room = self.actors[a].room.clone();
        let raw = self.actors[a].raw.as_mut().expect("raw");
        if raw.greeted.as_deref() == Some(baby.as_str()) {
            return;
        }
        let Some(peer) = raw.peer.as_mut() else {
            return;
        };
        let here = peer.here();
        raw.greeted = Some(baby.clone());
        let text = encode_signal(&room, Some(&baby), here);
        self.client_send(a, text, None);
    }

    /// Sends `payload` to `to`: sealed for a PSK holder (dropped while the baby has not confirmed
    /// it yet), plaintext for the outsider.
    fn raw_send_signal(&mut self, a: usize, to: &str, payload: &SignalPayload) {
        let room = self.actors[a].room.clone();
        let json = signal_payload_json(payload);
        let wire = {
            let raw = self.actors[a].raw.as_mut().expect("raw");
            match raw.peer.as_mut() {
                Some(peer) => match peer.try_say(class_of(payload), &json) {
                    Some(sealed) => sealed,
                    None => return,
                },
                None => json,
            }
        };
        let text = encode_signal(&room, Some(to), wire);
        self.client_send(a, text, None);
    }

    fn raw_baby(&self, a: usize) -> Option<String> {
        let raw = self.actors[a].raw.as_ref()?;
        raw.members
            .iter()
            .find(|(id, r)| *r == Role::Baby && Some(id) != raw.client_id.as_ref())
            .map(|(id, _)| id.clone())
    }

    fn raw_maybe_request(&mut self, a: usize) {
        let raw = self.actors[a].raw.as_ref().expect("raw");
        if raw.mode == RawMode::ForgedOffer {
            return;
        }
        let active = raw
            .pc
            .and_then(|h| self.actors[a].pcs.get(&h))
            .is_some_and(|p| !p.closed && !p.failed);
        let due = raw
            .last_request
            .is_none_or(|t| self.now.saturating_sub(t) >= 5_000);
        let Some(baby) = self.raw_baby(a) else {
            return;
        };
        if active || !due {
            return;
        }
        let aid = self.rng.aid();
        let raw = self.actors[a].raw.as_mut().expect("raw");
        raw.my_aid = Some(aid.clone());
        raw.last_request = Some(self.now);
        self.raw_send_signal(a, &baby, &SignalPayload::RequestOffer { aid });
    }

    pub(crate) fn raw_tick(&mut self, a: usize) {
        if !self.actors[a].alive || self.actors[a].raw.is_none() {
            return;
        }
        if self.actors[a].sock.is_some() {
            self.raw_maybe_request(a);
        }
        self.schedule_raw_tick(a, 2_000);
    }

    fn raw_on_signal(&mut self, a: usize, from: &str, payload: SignalPayload) {
        let mode = self.actors[a].raw.as_ref().expect("raw").mode;
        let SignalPayload::Sdp {
            aid,
            sdp_type,
            sdp,
            re,
        } = payload
        else {
            return;
        };
        if mode == RawMode::ForgedOffer
            || sdp_type != "offer"
            || self.raw_baby(a).as_deref() != Some(from)
        {
            return;
        }
        if let Some(old) = self.actors[a].raw.as_ref().and_then(|r| r.pc) {
            self.pc_close(a, old);
        }
        let handle = self.actors[a].next_raw_handle;
        self.actors[a].next_raw_handle += 1;
        self.new_pc(a, handle);
        self.set_remote(a, handle, &sdp);
        let own = self.actors[a].raw.as_ref().and_then(|r| r.my_aid.clone());
        let answer_aid = if re.is_some() && re == own {
            own.unwrap_or_default()
        } else {
            self.rng.aid()
        };
        let _ = aid;
        let fp = self.actors[a].pcs[&handle].fp.clone();
        let answer_sdp = self.sdp_for(a, &fp);
        {
            let raw = self.actors[a].raw.as_mut().expect("raw");
            raw.pc = Some(handle);
            raw.baby_fp = extract_sha256_fingerprint(&sdp);
        }
        self.actors[a].pcs.get_mut(&handle).expect("pc").local = true;
        self.raw_send_signal(
            a,
            from,
            &SignalPayload::Sdp {
                aid: answer_aid,
                sdp_type: "answer".into(),
                sdp: answer_sdp,
                re: None,
            },
        );
        self.maybe_connect(a, handle);
    }

    pub(crate) fn raw_on_open(&mut self, a: usize, h: u32) {
        let raw = self.actors[a].raw.as_ref().expect("raw");
        if raw.mode != RawMode::WrongPsk {
            return;
        }
        let (Some(baby_fp), room, psk) = (raw.baby_fp.clone(), raw.room.clone(), raw.psk.clone())
        else {
            return;
        };
        let my_fp = self.actors[a].pcs[&h].fp.clone();
        if let Some(mac) = pair_proof_mac_v3(&psk, Role::Parent, &room, &my_fp, &baby_fp) {
            self.send_data(a, h, encode_data_message(&DataMessage::PairProof { mac }));
        }
    }

    pub(crate) fn raw_on_data(&mut self, a: usize, h: u32, text: &str) {
        let mode = self.actors[a].raw.as_ref().expect("raw").mode;
        if mode == RawMode::Reflect
            && matches!(
                parse_data_message(text),
                Some(DataMessage::PairProof { .. })
            )
        {
            self.send_data(a, h, text.to_owned());
        }
    }

    /// Forged traffic towards a verified pair: new-aid offer, candidate, `request_offer` and
    /// `stop_notice`, optionally with a `from` forged by a compromised relay.
    pub(crate) fn raw_act(&mut self, a: usize) {
        let (room, spoof, own) = {
            let raw = self.actors[a].raw.as_mut().expect("raw");
            if raw.acted {
                return;
            }
            raw.acted = true;
            (raw.room.clone(), raw.spoof, raw.client_id.clone())
        };
        let members = self.actors[a].raw.as_ref().expect("raw").members.clone();
        let find = |role: Role| {
            members
                .iter()
                .find(|(id, r)| *r == role && Some(id) != own.as_ref())
                .map(|(id, _)| id.clone())
        };
        let (Some(baby), Some(parent)) = (find(Role::Baby), find(Role::Parent)) else {
            return;
        };
        let fp = self.fresh_fp();
        let forged_offer = SignalPayload::Sdp {
            aid: self.rng.aid(),
            sdp_type: "offer".into(),
            sdp: format!(
                "v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=sendonly\r\na=fingerprint:sha-256 {}\r\n",
                colon_upper(&fp)
            ),
            re: None,
        };
        let forged_ice = SignalPayload::Ice {
            aid: self.rng.aid(),
            candidate: guardedsleep_core::protocol::IceCandidate {
                candidate: "forged".into(),
                sdp_mid: Some("0".into()),
                sdp_m_line_index: Some(0),
            },
        };
        let request = SignalPayload::RequestOffer {
            aid: self.rng.aid(),
        };
        // Without the PSK the stranger cannot seal anything: a bare stop is plaintext noise.
        let bare_stop = SignalPayload::StopNotice;
        let to_parent = [forged_offer, forged_ice, bare_stop.clone()];
        let to_baby = [request, bare_stop];
        for p in to_parent {
            let text = encode_signal(&room, Some(&parent), signal_payload_json(&p));
            self.client_send(a, text, spoof.then(|| baby.clone()));
        }
        for p in to_baby {
            let text = encode_signal(&room, Some(&baby), signal_payload_json(&p));
            self.client_send(a, text, spoof.then(|| parent.clone()));
        }
    }
}
