//! Hostile relays and strangers: SDP rewriting, a man in the middle, stop replay, ghost sockets.

use super::*;

/// Rewrites the SDP inside `m` the way a fingerprint-confusing relay forwards it.
fn confuse_in_place(m: &mut Value, mitm_fp: &str) {
    if let Some(sdp) = m["sdp"]["sdp"].as_str() {
        m["sdp"]["sdp"] = Value::from(confuse_fingerprint(sdp, mitm_fp));
    }
}

impl World {
    /// Whether the relay currently rewrites SDPs in transit.
    pub(super) fn relay_rewrites_sdp(&self) -> bool {
        matches!(
            self.relay.sdp_rewrite.as_deref(),
            Some("fingerprint_confusion" | "mitm_recompute")
        )
    }

    /// Puts the relay's own fingerprint into the SDP of `payload`, sealed or plain.
    pub(super) fn rewrite_sdp(&mut self, psk: Option<&[u8]>, mut payload: Value) -> Value {
        let mitm = self.fresh_fp();
        match psk {
            Some(psk) if payload.get("env").is_some() => {
                self.rewrite_sealed(psk, payload, |m| confuse_in_place(m, &mitm))
            }
            _ => {
                confuse_in_place(&mut payload, &mitm);
                payload
            }
        }
    }

    fn rewrite_sealed(
        &mut self,
        psk: &[u8],
        payload: Value,
        confuse: impl FnOnce(&mut Value),
    ) -> Value {
        let mut nonce = [0u8; 24];
        for chunk in nonce.chunks_mut(8) {
            chunk.copy_from_slice(&self.rng.next_u64().to_le_bytes()[..chunk.len()]);
        }
        // The man in the middle reuses the original nonce; it cannot compute the pin tag, which needs
        // the parent-secret subkey and covers the message, so a rewritten envelope is refused before
        // any attempt starts (the DataChannel proofs, bound to the fingerprints, are the second layer).
        let recompute = self.relay.sdp_rewrite.as_deref() == Some("mitm_recompute");
        if let Some(original) = nonce_of(&payload).filter(|_| recompute) {
            nonce = original;
        }
        let rewritten = rewrite_message(psk, &payload, nonce, confuse);
        rewritten.unwrap_or(payload)
    }

    /// A man in the middle that holds the PSK and sits between two DTLS sessions
    /// (`sdp_rewrite: "mitm_recompute"`): the SDPs it relays carry its own certificate per leg, and
    /// it recomputes the pairing proof for each leg, so the PSK-level verification succeeds. It does
    /// not know the parent secret, so every other DataChannel message (the parent proof included)
    /// passes unchanged, computed over fingerprints the other leg never saw.
    pub(super) fn mitm_adjust(&self, to: PcRef, text: String) -> String {
        if self.relay.sdp_rewrite.as_deref() != Some("mitm_recompute") {
            return text;
        }
        let is_proof = matches!(
            parse_data_message(&text),
            Some(DataMessage::PairProof { .. })
        );
        match self.recomputed_proof(to).filter(|_| is_proof) {
            Some(proof) => encode_data_message(&proof),
            None => text,
        }
    }

    /// The pairing proof as `to`'s side expects it from the other end, over the fingerprints of
    /// the leg that ends here.
    fn recomputed_proof(&self, to: PcRef) -> Option<DataMessage> {
        let ac = &self.actors[to.actor];
        let psk = self.actor_psk(to.actor)?;
        let pc = ac.pc(to.handle)?;
        let remote = pc.dtls_fp.clone()?;
        let sender = match ac.role {
            Role::Baby => Role::Parent,
            Role::Parent => Role::Baby,
        };
        let mac = pair_proof_mac_v3(&psk, sender, &ac.room, &remote, &pc.fp)?;
        Some(DataMessage::PairProof { mac })
    }

    /// A hostile relay re-delivers the last recorded `stop_notice` to `peer`, as the original
    /// sender, on whatever connection `peer` has now.
    pub(super) fn replay_stop(&mut self, st: &Step) {
        let Some(a) = self.peer_ix(st) else {
            return;
        };
        self.select_server(st.relay.unwrap_or(0));
        let Some((from, payload)) = self.server.last_stop.clone() else {
            self.violations
                .push("replay_stop: no stop notice was recorded".into());
            return;
        };
        let sock = self
            .server
            .conns
            .iter()
            .find(|(_, c)| c.actor_ix() == Some(a) && c.joined)
            .map(|(s, _)| *s);
        if let Some(sock) = sock {
            self.server_send(sock, json!({"type":"signal","from":from,"payload":payload}));
        }
    }

    /// A socket whose client died silently: holds its room slot until the liveness sweep.
    pub(super) fn ghost_join(&mut self, st: &Step) {
        let role = st.role.unwrap_or(Role::Parent);
        let requested = self.target_room();
        let key = self.room_key(&requested);
        if self.seat_refusal(role, &key).is_some() {
            return;
        }
        self.select_server(st.relay.unwrap_or(0));
        let sock = SockIx::from(self.next_sock);
        self.next_sock += 1;
        self.sock_server.insert(sock, self.cur_server);
        let (number, join_seq) = self.server.next_ids();
        let seat = Seat {
            role,
            client_id: format!("c{number}"),
            requested,
            key: key.clone(),
            join_seq,
        };
        self.server.conns.insert(sock, Conn::ghost(seat));
        self.broadcast_members(&key);
    }

    pub(crate) fn fresh_fp(&mut self) -> String {
        self.fp_seq += 1;
        synthetic_fp(self.fp_seq)
    }

    /// The relay turns hostile (or honest again) at run time: `mode` is an `sdp_rewrite` value,
    /// absent for honest.
    pub(super) fn verb_relay_rewrite(&mut self, st: &Step) {
        self.relay.sdp_rewrite = st.mode.clone();
    }
}
