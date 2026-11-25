//! Rooms of the simulated relay: joining, membership broadcasts and ICE servers.

use super::*;

/// Why the relay turns a `hello` away, and whether it then closes the socket.
struct Refusal {
    code: &'static str,
    close: bool,
}

/// The TURN credentials a relay hands to the members of one room.
struct IceIssue {
    ttl_s: u64,
    expiry: u64,
    host: String,
}

fn hello_role(msg: &Value) -> Option<Role> {
    match msg["role"].as_str() {
        Some("baby") => Some(Role::Baby),
        Some("parent") => Some(Role::Parent),
        _ => None,
    }
}

impl World {
    pub(super) fn room_members(&self, key: &RoomKey) -> Vec<(SockIx, &Conn)> {
        let mut m: Vec<_> = self
            .server
            .conns
            .iter()
            .filter(|(_, c)| c.joined && c.key == key.as_str())
            .map(|(s, c)| (*s, c))
            .collect();
        m.sort_by_key(|(_, c)| c.join_seq);
        m
    }

    pub(super) fn broadcast_members(&mut self, key: &RoomKey) {
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
    pub(super) fn room_key(&self, requested: &str) -> RoomKey {
        let key = if self.relay.alias_rooms {
            "alias"
        } else {
            requested
        };
        RoomKey::new(key.to_owned())
    }

    fn role_counts(&self, key: &RoomKey) -> (usize, usize) {
        let members = self.room_members(key);
        let count = |role| members.iter().filter(|(_, c)| c.role == role).count();
        (count(Role::Baby), count(Role::Parent))
    }

    /// The error code when no seat is free for `role` in the room `key`.
    pub(super) fn seat_refusal(&self, role: Role, key: &RoomKey) -> Option<&'static str> {
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
    fn announce_peers(&mut self, sock: SockIx, key: &RoomKey, client_id: &str) {
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
    pub(super) fn issue_ice_servers(&mut self, key: &RoomKey) {
        let Some(ttl_s) = self.relay.ice.as_ref().map(|i| i.ttl_s) else {
            return;
        };
        let (babies, parents) = self.role_counts(key);
        if babies == 0 || parents == 0 {
            return;
        }
        let socks: Vec<SockIx> = self.room_members(key).iter().map(|(s, _)| *s).collect();
        let issue = IceIssue {
            ttl_s,
            expiry: self.wall_base + (self.now + ttl_s * 1_000) / 1_000,
            host: format!("relay-{}.invalid", self.cur_server),
        };
        for s in socks {
            self.issue_credential(s, &issue);
        }
        if self.server.ice_timers.insert(key.clone()) {
            let ev = RelayEv::IceRenew {
                srv: self.cur_server,
                key: key.clone(),
            };
            self.push(self.now + ttl_s * 500, 3, ev);
        }
    }

    /// One `ice_servers` frame with a fresh credential for `sock`.
    fn issue_credential(&mut self, sock: SockIx, issue: &IceIssue) {
        self.server.ice_seq += 1;
        let n = self.server.ice_seq;
        // Learned at issuance, before the frame is delivered and scanned. Not wire-exempt.
        if let Some(c) = self.canary.as_mut() {
            c.set.add_turn_password(&format!("simcred{n}"));
        }
        let host = &issue.host;
        self.server_send(
            sock,
            json!({
                "type": "ice_servers",
                "urls": [
                    format!("stun:{host}:3478"),
                    format!("turn:{host}:3478?transport=udp"),
                    format!("turn:{host}:3478?transport=tcp"),
                ],
                "username": format!("{}:sim{n}", issue.expiry),
                "credential": format!("simcred{n}"),
                "ttl": issue.ttl_s,
            }),
        );
    }

    pub(super) fn on_ice_renew(&mut self, key: RoomKey) {
        self.server.ice_timers.remove(&key);
        if self.server.up {
            self.issue_ice_servers(&key);
        }
    }
}
