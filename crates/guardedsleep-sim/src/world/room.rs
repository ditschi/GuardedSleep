//! Rooms of the simulated relay: joining, membership broadcasts, ICE servers and signal routing.

use super::*;

/// Why the relay turns a `hello` away, and whether it then closes the socket.
struct Refusal {
    code: &'static str,
    close: bool,
}

/// Where a `signal` goes: the target socket and the sender's client id.
struct Route {
    target: SockIx,
    from: String,
}

fn hello_role(msg: &Value) -> Option<Role> {
    match msg["role"].as_str() {
        Some("baby") => Some(Role::Baby),
        Some("parent") => Some(Role::Parent),
        _ => None,
    }
}

impl World {
    pub(super) fn room_members(&self, key: &str) -> Vec<(SockIx, &Conn)> {
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
        let socks: Vec<SockIx> = members.iter().map(|(s, _)| *s).collect();
        for s in socks {
            self.server_send(s, json!({"type":"members","peers":peers}));
        }
    }

    /// The room a request lands in (every request in one room, when the relay aliases rooms).
    pub(super) fn room_key(&self, requested: &str) -> String {
        if self.relay.alias_rooms {
            "alias".to_owned()
        } else {
            requested.to_owned()
        }
    }

    fn role_counts(&self, key: &str) -> (usize, usize) {
        let members = self.room_members(key);
        let count = |role| members.iter().filter(|(_, c)| c.role == role).count();
        (count(Role::Baby), count(Role::Parent))
    }

    /// The error code when no seat is free for `role` in the room `key`.
    pub(super) fn seat_refusal(&self, role: Role, key: &str) -> Option<&'static str> {
        let (babies, parents) = self.role_counts(key);
        match role {
            Role::Baby if babies > 0 => Some("BABY_TAKEN"),
            Role::Parent if parents >= self.relay.parent_cap => Some("PARENT_CAP"),
            _ => None,
        }
    }

    pub(super) fn server_hello(&mut self, sock: SockIx, msg: &Value) {
        match self.vet_hello(sock, msg) {
            Ok((role, requested)) => self.join_room(sock, role, requested),
            Err(refusal) => {
                self.server_error(sock, refusal.code);
                if refusal.close {
                    self.close_after_error(sock);
                }
            }
        }
    }

    /// The role and room of an acceptable `hello`, or the reason for refusing it.
    fn vet_hello(&self, sock: SockIx, msg: &Value) -> Result<(Role, String), Refusal> {
        let keep = |code| Refusal { code, close: false };
        let close = |code| Refusal { code, close: true };
        if self.server.conns[&sock].joined {
            return Err(keep("INVALID_MESSAGE"));
        }
        let role = hello_role(msg).ok_or_else(|| keep("INVALID_MESSAGE"))?;
        let requested = msg["roomId"].as_str().unwrap_or("").to_owned();
        if msg["protocolVersion"].as_u64() != Some(3) || self.now < self.server.reject_hello_until {
            return Err(close("UNSUPPORTED_PROTOCOL"));
        }
        if !valid_v3_room(&requested) {
            return Err(close("INVALID_MESSAGE"));
        }
        match self.seat_refusal(role, &self.room_key(&requested)) {
            Some(code) => Err(close(code)),
            None => Ok((role, requested)),
        }
    }

    fn join_room(&mut self, sock: SockIx, role: Role, requested: String) {
        let key = self.room_key(&requested);
        let (number, join_seq) = self.server.next_ids();
        let client_id = if self.relay.reuse_client_ids {
            format!("c-{}", role.as_str())
        } else {
            format!("c{number}")
        };
        let seat = Seat {
            role,
            client_id: client_id.clone(),
            requested: requested.clone(),
            key: key.clone(),
            join_seq,
        };
        if let Some(c) = self.server.conns.get_mut(&sock) {
            c.take_seat(seat);
        }
        self.server_send(
            sock,
            json!({"type":"welcome","clientId":client_id,"roomId":requested}),
        );
        // Before the `members` update that completes the room (design section 7).
        self.issue_ice_servers(&key);
        self.broadcast_members(&key);
        self.announce_peers(sock, &key, &client_id);
    }

    /// Tells the other members about the newcomer and the newcomer about the first other member.
    fn announce_peers(&mut self, sock: SockIx, key: &str, client_id: &str) {
        let others: Vec<(SockIx, String)> = self
            .room_members(key)
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
        let (babies, parents) = self.role_counts(key);
        if babies == 0 || parents == 0 {
            return;
        }
        let socks: Vec<SockIx> = self.room_members(key).iter().map(|(s, _)| *s).collect();
        let host = format!("relay-{}.invalid", self.cur_server);
        let expiry = self.wall_base + (self.now + ttl_s * 1_000) / 1_000;
        for s in socks {
            self.issue_credential(s, ttl_s, &format!("{expiry}:sim"), &host);
        }
        if self.server.ice_timers.insert(key.to_owned()) {
            let ev = RelayEv::IceRenew {
                srv: self.cur_server,
                key: key.to_owned(),
            };
            self.push(self.now + ttl_s * 500, 3, ev);
        }
    }

    /// One `ice_servers` frame with a fresh credential for `sock`.
    fn issue_credential(&mut self, sock: SockIx, ttl_s: u64, user_prefix: &str, host: &str) {
        self.server.ice_seq += 1;
        let n = self.server.ice_seq;
        // Learned at issuance, before the frame is delivered and scanned. Not wire-exempt.
        if let Some(c) = self.canary.as_mut() {
            c.set.add_turn_password(&format!("simcred{n}"));
        }
        self.server_send(
            sock,
            json!({
                "type": "ice_servers",
                "urls": [
                    format!("stun:{host}:3478"),
                    format!("turn:{host}:3478?transport=udp"),
                    format!("turn:{host}:3478?transport=tcp"),
                ],
                "username": format!("{user_prefix}{n}"),
                "credential": format!("simcred{n}"),
                "ttl": ttl_s,
            }),
        );
    }

    pub(super) fn on_ice_renew(&mut self, key: String) {
        self.server.ice_timers.remove(&key);
        if self.server.up {
            self.issue_ice_servers(&key);
        }
    }

    pub(super) fn server_signal(&mut self, sock: SockIx, msg: &Value, spoof: Option<String>) {
        let Some(route) = self.route_signal(sock, msg) else {
            return;
        };
        if self.server.drop_signals {
            return;
        }
        let payload = self.relay_payload(sock, &route.from, msg["payload"].clone());
        let from = spoof.unwrap_or(route.from);
        self.server_send(
            route.target,
            json!({"type":"signal","from": from,"payload": payload}),
        );
    }

    /// Checks a `signal` and finds its target; answers the sender with an error otherwise.
    fn route_signal(&mut self, sock: SockIx, msg: &Value) -> Option<Route> {
        let c = &self.server.conns[&sock];
        if !c.joined {
            self.server_error(sock, "NOT_JOINED");
            return None;
        }
        if msg["roomId"].as_str() != Some(c.requested.as_str()) {
            self.server_error(sock, "INVALID_MESSAGE");
            return None;
        }
        let (key, from) = (c.key.clone(), c.client_id.clone());
        let to = msg["to"].as_str().unwrap_or("");
        match self.signal_target(sock, &key, to) {
            Ok(target) => target.map(|target| Route { target, from }),
            Err(code) => {
                self.server_error(sock, code);
                None
            }
        }
    }

    /// The socket a `signal` of `sock` goes to: the named one, or the only other member. `None`
    /// is a signal to an empty room, which is dropped without an error.
    fn signal_target(
        &self,
        sock: SockIx,
        key: &str,
        to: &str,
    ) -> Result<Option<SockIx>, &'static str> {
        let members: Vec<(SockIx, String)> = self
            .room_members(key)
            .iter()
            .map(|(s, c)| (*s, c.client_id.clone()))
            .collect();
        let other = |(s, _): &&(SockIx, String)| *s != sock;
        if !to.is_empty() {
            let named = members.iter().find(|m| m.1 == to && other(m));
            return named.map(|(s, _)| Some(*s)).ok_or("INVALID_MESSAGE");
        }
        if members.len() <= 2 {
            return Ok(members.iter().find(other).map(|(s, _)| *s));
        }
        Err("SIGNAL_TARGET_REQUIRED")
    }

    /// What the relay forwards: the payload, noted if it is a stop notice, and rewritten when the
    /// relay is hostile. The recording and rewriting relays read sealed payloads: the simulation
    /// stands in for a relay that holds the PSK (a compromised one that was handed the invite).
    fn relay_payload(&mut self, sock: SockIx, from: &str, payload: Value) -> Value {
        let sender_psk = self.server.conns[&sock]
            .actor_ix()
            .and_then(|sa| self.actor_psk(sa));
        self.record_stop_notice(from, &payload, sender_psk.as_deref());
        if self.relay_rewrites_sdp() {
            self.rewrite_sdp(sender_psk.as_deref(), payload)
        } else {
            payload
        }
    }

    /// A hostile relay keeps the last `stop_notice` for `replay_stop`.
    fn record_stop_notice(&mut self, from: &str, payload: &Value, psk: Option<&[u8]>) {
        let message = match psk {
            Some(psk) if payload.get("env").is_some() => decode_message(psk, payload),
            _ => Some(payload.clone()),
        };
        let is_stop = message.as_ref().and_then(|m| m["type"].as_str()) == Some("stop_notice");
        if is_stop {
            self.server.last_stop = Some((from.to_owned(), payload.clone()));
        }
    }
}
