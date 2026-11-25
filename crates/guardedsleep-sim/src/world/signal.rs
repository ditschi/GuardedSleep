//! Routing of `signal` frames in the simulated relay, and what a hostile relay does to them.

use super::*;

/// Where a `signal` goes: the target socket and the sender's client id.
struct Route {
    target: SockIx,
    from: String,
}

impl World {
    pub(super) fn server_signal(&mut self, sock: SockIx, msg: &Value, spoof: Option<String>) {
        let Some(route) = self.route_signal(sock, msg) else {
            return;
        };
        if self.server.drop_signals {
            return;
        }
        let payload = self.relay_payload(sock, &route, msg["payload"].clone());
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
        let (key, from) = (RoomKey::new(c.key.clone()), c.client_id.clone());
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
        key: &RoomKey,
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
    fn relay_payload(&mut self, sock: SockIx, route: &Route, payload: Value) -> Value {
        let sender_psk = self.server.conns[&sock]
            .actor_ix()
            .and_then(|sa| self.actor_psk(sa));
        self.record_stop_notice(route, &payload, sender_psk.as_deref());
        if self.relay_rewrites_sdp() {
            self.rewrite_sdp(sender_psk.as_deref(), payload)
        } else {
            payload
        }
    }

    /// A hostile relay keeps the last `stop_notice` for `replay_stop`.
    fn record_stop_notice(&mut self, route: &Route, payload: &Value, psk: Option<&[u8]>) {
        let message = match psk {
            Some(psk) if payload.get("env").is_some() => decode_message(psk, payload),
            _ => Some(payload.clone()),
        };
        let is_stop = message.as_ref().and_then(|m| m["type"].as_str()) == Some("stop_notice");
        if is_stop {
            self.server.last_stop = Some((route.from.clone(), payload.clone()));
        }
    }
}
