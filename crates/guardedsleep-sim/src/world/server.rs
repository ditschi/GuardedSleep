//! The simulated signaling server (mirrors `crates/guardedsleep-relay`).

use super::*;

impl World {
    pub(super) fn signaling_down(&mut self) {
        self.server.up = false;
        self.server.sweep_gen += 1;
        let socks: Vec<usize> = self.server.conns.keys().copied().collect();
        for s in socks {
            self.server_terminate_silent(s);
        }
    }

    /// Server restart: connections vanish without member broadcasts (there is no server left).
    pub(super) fn server_terminate_silent(&mut self, sock: usize) {
        let Some(c) = self.server.conns.remove(&sock) else {
            return;
        };
        if let Some(a) = c.actor
            && self.reach(a, c.epoch)
        {
            let lat = self.rng.range(5, 40);
            let t = self.fifo_time(0, sock, 0, self.now + lat, false);
            self.push(
                t,
                2,
                Ev::SigToClient {
                    actor: a,
                    sock,
                    msg: ToClient::Closed,
                },
            );
        }
    }

    pub(super) fn on_accept(&mut self, sock: usize) {
        let Some(&(a, epoch)) = self.socks.get(&sock) else {
            return;
        };
        if !self.actors[a].socks.values().any(|s| *s == sock) {
            return;
        }
        if !self.server.up || !self.reach(a, epoch) {
            let d = 3_000;
            self.push(
                self.now + d,
                2,
                Ev::SigToClient {
                    actor: a,
                    sock,
                    msg: ToClient::Closed,
                },
            );
            return;
        }
        self.server.conns.insert(
            sock,
            Conn {
                actor: Some(a),
                epoch,
                role: self.actors[a].role,
                client_id: String::new(),
                requested: String::new(),
                key: String::new(),
                joined: false,
                alive: true,
                join_seq: 0,
            },
        );
        let lat = self.rng.range(5, 40);
        let t = self.fifo_time(0, sock, 0, self.now + lat, false);
        self.push(
            t,
            2,
            Ev::SigToClient {
                actor: a,
                sock,
                msg: ToClient::Opened,
            },
        );
    }

    pub(super) fn server_send(&mut self, sock: usize, v: Value) {
        let Some(c) = self.server.conns.get(&sock) else {
            return;
        };
        let (Some(a), epoch) = (c.actor, c.epoch) else {
            return;
        };
        let msg = ToClient::Text(v.to_string());
        if self.reach(a, epoch) {
            let lat = self.rng.range(5, 40);
            let t = self.fifo_time(0, sock, 0, self.now + lat, false);
            self.push(
                t,
                2,
                Ev::SigToClient {
                    actor: a,
                    sock,
                    msg,
                },
            );
        } else if self.viable(a, epoch) {
            self.actors[a].held_down.push((sock, msg));
        }
    }

    pub(super) fn server_error(&mut self, sock: usize, code: &str) {
        self.server_send(
            sock,
            json!({"type":"error","code":code,"message":"simulated"}),
        );
    }

    pub(super) fn room_members(&self, key: &str) -> Vec<(usize, &Conn)> {
        let mut m: Vec<_> = self
            .server
            .conns
            .iter()
            .filter(|(_, c)| c.joined && c.key == key)
            .map(|(s, c)| (*s, c))
            .collect();
        m.sort_by_key(|(_, c)| c.join_seq);
        m
    }

    pub(super) fn broadcast_members(&mut self, key: &str) {
        let members = self.room_members(key);
        if members.is_empty() {
            return;
        }
        let peers: Vec<Value> = members
            .iter()
            .map(|(_, c)| json!({"clientId": c.client_id, "role": c.role.as_str()}))
            .collect();
        let socks: Vec<usize> = members.iter().map(|(s, _)| *s).collect();
        for s in socks {
            self.server_send(s, json!({"type":"members","peers":peers}));
        }
    }

    /// `ws.terminate()` / client close: cleanup, members broadcast, FIN to a reachable client.
    pub(super) fn server_terminate(&mut self, sock: usize) {
        let Some(c) = self.server.conns.remove(&sock) else {
            return;
        };
        if c.joined {
            self.broadcast_members(&c.key);
        }
        if let Some(a) = c.actor
            && self.reach(a, c.epoch)
            && self.actors[a].socks.values().any(|s| *s == sock)
        {
            let lat = self.rng.range(5, 40);
            let t = self.fifo_time(0, sock, 0, self.now + lat, false);
            self.push(
                t,
                2,
                Ev::SigToClient {
                    actor: a,
                    sock,
                    msg: ToClient::Closed,
                },
            );
        }
    }

    pub(super) fn on_server_rx(&mut self, sock: usize, text: &str, spoof: Option<String>) {
        if !self.server.up {
            return;
        }
        if !self.server.conns.contains_key(&sock) {
            // TCP reset: the server has no such connection (evicted or restarted).
            if let Some(&(a, epoch)) = self.socks.get(&sock)
                && self.reach(a, epoch)
                && self.actors[a].socks.values().any(|s| *s == sock)
            {
                let lat = self.rng.range(5, 40);
                self.push(
                    self.now + lat,
                    2,
                    Ev::SigToClient {
                        actor: a,
                        sock,
                        msg: ToClient::Closed,
                    },
                );
            }
            return;
        }
        let Ok(msg) = serde_json::from_str::<Value>(text) else {
            self.server_error(sock, "BAD_JSON");
            return;
        };
        // What a socket says before (or without) joining a room, per actor: the reachability
        // probes of a verified link. The capture assertion `preauth` reads it.
        if !self.server.conns[&sock].joined
            && msg["type"] != "hello"
            && let Some(a) = self.server.conns[&sock].actor
        {
            self.preauth.push((a, text.to_owned()));
        }
        match msg["type"].as_str().unwrap_or("") {
            "ping" => self.server_send(sock, json!({"type":"pong"})),
            "hello" => self.server_hello(sock, &msg),
            "signal" => self.server_signal(sock, &msg, spoof),
            _ => self.server_error(sock, "BAD_JSON"),
        }
    }

    pub(super) fn server_hello(&mut self, sock: usize, msg: &Value) {
        let joined = self.server.conns[&sock].joined;
        if joined {
            self.server_error(sock, "INVALID_MESSAGE");
            return;
        }
        let role = match msg["role"].as_str() {
            Some("baby") => Role::Baby,
            Some("parent") => Role::Parent,
            _ => {
                self.server_error(sock, "INVALID_MESSAGE");
                return;
            }
        };
        let requested = msg["roomId"].as_str().unwrap_or("").to_owned();
        if msg["protocolVersion"].as_u64() != Some(3) || self.now < self.server.reject_hello_until {
            self.server_error(sock, "UNSUPPORTED_PROTOCOL");
            self.close_after_error(sock);
            return;
        }
        if !valid_v3_room(&requested) {
            self.server_error(sock, "INVALID_MESSAGE");
            self.close_after_error(sock);
            return;
        }
        let key = if self.relay.alias_rooms {
            "alias".to_owned()
        } else {
            requested.clone()
        };
        let (babies, parents) = {
            let m = self.room_members(&key);
            (
                m.iter().filter(|(_, c)| c.role == Role::Baby).count(),
                m.iter().filter(|(_, c)| c.role == Role::Parent).count(),
            )
        };
        let refusal = match role {
            Role::Baby if babies > 0 => Some("BABY_TAKEN"),
            Role::Parent if parents >= self.relay.parent_cap => Some("PARENT_CAP"),
            _ => None,
        };
        if let Some(code) = refusal {
            self.server_error(sock, code);
            self.close_after_error(sock);
            return;
        }
        self.server.next_client += 1;
        self.server.next_join += 1;
        let client_id = if self.relay.reuse_client_ids {
            format!("c-{}", role.as_str())
        } else {
            format!("c{}", self.server.next_client)
        };
        let join_seq = self.server.next_join;
        let c = self.server.conns.get_mut(&sock).expect("exists");
        c.role = role;
        c.client_id = client_id.clone();
        c.requested = requested.clone();
        c.key = key.clone();
        c.joined = true;
        c.join_seq = join_seq;
        self.server_send(
            sock,
            json!({"type":"welcome","clientId":client_id,"roomId":requested}),
        );
        // Before the `members` update that completes the room (design section 7).
        self.issue_ice_servers(&key);
        self.broadcast_members(&key);
        let others: Vec<(usize, String)> = self
            .room_members(&key)
            .iter()
            .filter(|(s, _)| *s != sock)
            .map(|(s, c)| (*s, c.client_id.clone()))
            .collect();
        for (s, _) in &others {
            self.server_send(*s, json!({"type":"peer","peerId":client_id}));
        }
        let first = others.first().map(|(_, id)| Value::from(id.clone()));
        self.server_send(
            sock,
            json!({"type":"peer","peerId": first.unwrap_or(Value::Null)}),
        );
    }

    /// A relay with `RelaySpec::ice`: a credential of its own to every member of a room that holds
    /// a baby and a parent, and the half-TTL renewal timer for that room.
    pub(super) fn issue_ice_servers(&mut self, key: &str) {
        let Some(ttl_s) = self.relay.ice.as_ref().map(|i| i.ttl_s) else {
            return;
        };
        let members = self.room_members(key);
        let complete = members.iter().any(|(_, c)| c.role == Role::Baby)
            && members.iter().any(|(_, c)| c.role == Role::Parent);
        if !complete {
            return;
        }
        let socks: Vec<usize> = members.iter().map(|(s, _)| *s).collect();
        let host = format!("relay-{}.invalid", self.cur_server);
        let expiry = self.wall_base + (self.now + ttl_s * 1_000) / 1_000;
        for s in socks {
            self.server.ice_seq += 1;
            let n = self.server.ice_seq;
            // Learned at issuance, before the frame is delivered and scanned. Not wire-exempt.
            if let Some(c) = self.canary.as_mut() {
                c.set.add_turn_password(&format!("simcred{n}"));
            }
            self.server_send(
                s,
                json!({
                    "type": "ice_servers",
                    "urls": [
                        format!("stun:{host}:3478"),
                        format!("turn:{host}:3478?transport=udp"),
                        format!("turn:{host}:3478?transport=tcp"),
                    ],
                    "username": format!("{expiry}:sim{n}"),
                    "credential": format!("simcred{n}"),
                    "ttl": ttl_s,
                }),
            );
        }
        if self.server.ice_timers.insert(key.to_owned()) {
            self.push(
                self.now + ttl_s * 500,
                3,
                Ev::IceRenew {
                    srv: self.cur_server,
                    key: key.to_owned(),
                },
            );
        }
    }

    pub(super) fn on_ice_renew(&mut self, key: String) {
        self.server.ice_timers.remove(&key);
        if self.server.up {
            self.issue_ice_servers(&key);
        }
    }

    pub(super) fn close_after_error(&mut self, sock: usize) {
        let t = self.fifo_time(0, sock, 0, self.now + 20, false);
        self.push(t, 2, Ev::ServerClose { sock });
    }

    pub(super) fn server_signal(&mut self, sock: usize, msg: &Value, spoof: Option<String>) {
        let c = &self.server.conns[&sock];
        if !c.joined {
            self.server_error(sock, "NOT_JOINED");
            return;
        }
        if msg["roomId"].as_str() != Some(c.requested.as_str()) {
            self.server_error(sock, "INVALID_MESSAGE");
            return;
        }
        let (key, from) = (c.key.clone(), c.client_id.clone());
        let to = msg["to"].as_str().unwrap_or("").to_owned();
        let members: Vec<(usize, String)> = self
            .room_members(&key)
            .iter()
            .map(|(s, c)| (*s, c.client_id.clone()))
            .collect();
        let target = if !to.is_empty() {
            match members.iter().find(|(s, id)| *id == to && *s != sock) {
                Some((s, _)) => *s,
                None => {
                    self.server_error(sock, "INVALID_MESSAGE");
                    return;
                }
            }
        } else if members.len() <= 2 {
            match members.iter().find(|(s, _)| *s != sock) {
                Some((s, _)) => *s,
                None => return,
            }
        } else {
            self.server_error(sock, "SIGNAL_TARGET_REQUIRED");
            return;
        };
        if self.server.drop_signals {
            return;
        }
        let mut payload = msg["payload"].clone();
        // The recording and rewriting relays below read sealed payloads: the simulation stands in
        // for a relay that holds the PSK (a compromised one that was handed the invite).
        let sender_psk = self.server.conns[&sock]
            .actor
            .and_then(|sa| self.actor_psk(sa));
        let message = match &sender_psk {
            Some(psk) if payload.get("env").is_some() => decode_message(psk, &payload),
            _ => Some(payload.clone()),
        };
        if message.as_ref().and_then(|m| m["type"].as_str()) == Some("stop_notice") {
            self.server.last_stop = Some((from.clone(), payload.clone()));
        }
        if matches!(
            self.relay.sdp_rewrite.as_deref(),
            Some("fingerprint_confusion" | "mitm_recompute")
        ) {
            let mitm = self.fresh_fp();
            if let Some(psk) = &sender_psk
                && payload.get("env").is_some()
            {
                let mut nonce = [0u8; 24];
                for chunk in nonce.chunks_mut(8) {
                    chunk.copy_from_slice(&self.rng.next_u64().to_le_bytes()[..chunk.len()]);
                }
                // The man in the middle reuses the original nonce; it cannot compute the pin tag, which needs
                // the parent-secret subkey and covers the message, so a rewritten envelope is refused before
                // any attempt starts (the DataChannel proofs, bound to the fingerprints, are the second layer).
                if self.relay.sdp_rewrite.as_deref() == Some("mitm_recompute")
                    && let Some(original) = nonce_of(&payload)
                {
                    nonce = original;
                }
                let rewritten = rewrite_message(psk, &payload, nonce, |m| {
                    if let Some(sdp) = m["sdp"]["sdp"].as_str() {
                        m["sdp"]["sdp"] = Value::from(confuse_fingerprint(sdp, &mitm));
                    }
                });
                if let Some(p) = rewritten {
                    payload = p;
                }
            } else if let Some(sdp) = payload["sdp"]["sdp"].as_str() {
                payload["sdp"]["sdp"] = Value::from(confuse_fingerprint(sdp, &mitm));
            }
        }
        self.server_send(
            target,
            json!({"type":"signal","from": spoof.unwrap_or(from),"payload": payload}),
        );
    }

    pub(super) fn on_sweep(&mut self, r#gen: u32) {
        if r#gen != self.server.sweep_gen || !self.server.up {
            return;
        }
        let socks: Vec<usize> = self.server.conns.keys().copied().collect();
        for s in socks {
            let Some(c) = self.server.conns.get(&s) else {
                continue;
            };
            if !c.alive {
                self.server_terminate(s);
                continue;
            }
            let responsive = self.responsive(c);
            self.server.conns.get_mut(&s).expect("exists").alive = false;
            if responsive {
                let rtt = self.rng.range(10, 80);
                self.push(self.now + rtt, 2, Ev::Pong { sock: s });
            }
        }
        self.push(
            self.now + SWEEP_MS,
            3,
            Ev::Sweep {
                r#gen,
                srv: self.cur_server,
            },
        );
    }

    /// A man in the middle that holds the PSK and sits between two DTLS sessions
    /// (`sdp_rewrite: "mitm_recompute"`): the SDPs it relays carry its own certificate per leg, and
    /// it recomputes the pairing proof for each leg, so the PSK-level verification succeeds. It does
    /// not know the parent secret, so every other DataChannel message (the parent proof included)
    /// passes unchanged, computed over fingerprints the other leg never saw.
    pub(super) fn mitm_adjust(&self, to: usize, handle: u32, text: String) -> String {
        if self.relay.sdp_rewrite.as_deref() != Some("mitm_recompute") {
            return text;
        }
        if !matches!(
            parse_data_message(&text),
            Some(DataMessage::PairProof { .. })
        ) {
            return text;
        }
        let ac = &self.actors[to];
        let (Some(psk), Some(pc)) = (self.actor_psk(to), ac.pcs.get(&handle)) else {
            return text;
        };
        let Some(remote) = pc.dtls_fp.clone() else {
            return text;
        };
        let sender = match ac.role {
            Role::Baby => Role::Parent,
            Role::Parent => Role::Baby,
        };
        match pair_proof_mac_v3(&psk, sender, &ac.room, &remote, &pc.fp) {
            Some(mac) => encode_data_message(&DataMessage::PairProof { mac }),
            None => text,
        }
    }
}
