//! The simulated signaling server (mirrors `crates/guardedsleep-relay`): connections, liveness and
//! the frames it takes.

use super::client::closed_to;
use super::*;

impl World {
    pub(super) fn dispatch_relay(&mut self, ev: RelayEv) {
        match ev {
            RelayEv::Accept { sock } => {
                self.select_sock(sock);
                self.on_accept(sock);
            }
            RelayEv::SigToServer { sock, frame } => {
                self.select_sock(sock);
                self.on_server_rx(sock, frame);
            }
            RelayEv::SigToClient { actor, sock, msg } => self.on_client_rx(actor, sock, msg),
            RelayEv::Pong { sock } => {
                self.select_sock(sock);
                self.on_pong(sock);
            }
            RelayEv::Sweep { r#gen, srv } => {
                self.select_server(srv);
                self.on_sweep(r#gen);
            }
            RelayEv::ServerClose { sock } => {
                self.select_sock(sock);
                self.server_terminate(sock);
            }
            RelayEv::IceRenew { srv, key } => {
                self.select_server(srv);
                self.on_ice_renew(key);
            }
        }
    }

    /// A reachable client answered the liveness ping.
    fn on_pong(&mut self, sock: SockIx) {
        let responsive = self
            .server
            .conns
            .get(&sock)
            .is_some_and(|c| self.responsive(c));
        if !responsive {
            return;
        }
        if let Some(c) = self.server.conns.get_mut(&sock) {
            c.alive = true;
        }
    }

    pub(super) fn signaling_down(&mut self) {
        self.server.up = false;
        self.server.sweep_gen += 1;
        let socks: Vec<SockIx> = self.server.conns.keys().copied().collect();
        for s in socks {
            self.server_terminate_silent(s);
        }
    }

    /// The actor behind a connection, when its path to the relay works.
    fn reachable_actor(&self, c: &Conn) -> Option<ActorIx> {
        c.actor_ix().filter(|a| self.reach(*a, c.epoch))
    }

    /// Like [`World::reachable_actor`], and `sock` is still one of the actor's sockets.
    fn attached_actor(&self, c: &Conn, sock: SockIx) -> Option<ActorIx> {
        self.reachable_actor(c)
            .filter(|a| self.actors[*a].owns(sock))
    }

    /// Server restart: connections vanish without member broadcasts (there is no server left).
    fn server_terminate_silent(&mut self, sock: SockIx) {
        let Some(c) = self.server.conns.remove(&sock) else {
            return;
        };
        if let Some(a) = self.reachable_actor(&c) {
            self.queue_to_client(a, sock, ToClient::Closed);
        }
    }

    pub(super) fn on_accept(&mut self, sock: SockIx) {
        let Some(SockOwner { actor: a, epoch }) = self.sock_owner(sock) else {
            return;
        };
        if !self.actors[a].owns(sock) {
            return;
        }
        if !self.server.up || !self.reach(a, epoch) {
            self.push(self.now + 3_000, 2, closed_to(a, sock));
            return;
        }
        let role = self.actors[a].role;
        self.server
            .conns
            .insert(sock, Conn::accepted(a, epoch, role));
        self.queue_to_client(a, sock, ToClient::Opened);
    }

    pub(super) fn server_send(&mut self, sock: SockIx, v: Value) {
        let Some(c) = self.server.conns.get(&sock) else {
            return;
        };
        let (Some(a), epoch) = (c.actor_ix(), c.epoch) else {
            return;
        };
        let msg = ToClient::Text(v.to_string());
        if self.reach(a, epoch) {
            self.queue_to_client(a, sock, msg);
        } else if self.viable(a, epoch) {
            self.actors[a].held_down.push((sock, msg));
        }
    }

    pub(super) fn server_error(&mut self, sock: SockIx, code: &str) {
        self.server_send(
            sock,
            json!({"type":"error","code":code,"message":"simulated"}),
        );
    }

    /// `ws.terminate()` / client close: cleanup, members broadcast, FIN to a reachable client.
    pub(super) fn server_terminate(&mut self, sock: SockIx) {
        let Some(c) = self.server.conns.remove(&sock) else {
            return;
        };
        if c.joined {
            self.broadcast_members(&c.key);
        }
        if let Some(a) = self.attached_actor(&c, sock) {
            self.queue_to_client(a, sock, ToClient::Closed);
        }
    }

    pub(super) fn on_server_rx(&mut self, sock: SockIx, frame: Frame) {
        if !self.server.up {
            return;
        }
        if !self.server.conns.contains_key(&sock) {
            self.reset_orphan(sock);
            return;
        }
        let Ok(msg) = serde_json::from_str::<Value>(&frame.text) else {
            self.server_error(sock, "BAD_JSON");
            return;
        };
        self.note_preauth(sock, &msg, &frame.text);
        match msg["type"].as_str().unwrap_or("") {
            "ping" => self.server_send(sock, json!({"type":"pong"})),
            "hello" => self.server_hello(sock, &msg),
            "signal" => self.server_signal(sock, &msg, frame.spoof),
            _ => self.server_error(sock, "BAD_JSON"),
        }
    }

    /// TCP reset: the server has no such connection (evicted or restarted).
    fn reset_orphan(&mut self, sock: SockIx) {
        let Some(SockOwner { actor: a, epoch }) = self.sock_owner(sock) else {
            return;
        };
        if self.reach(a, epoch) && self.actors[a].owns(sock) {
            let lat = self.rng.range(5, 40);
            self.push(self.now + lat, 2, closed_to(a, sock));
        }
    }

    /// What a socket says before (or without) joining a room, per actor: the reachability probes
    /// of a verified link. The capture assertion `preauth` reads it.
    fn note_preauth(&mut self, sock: SockIx, msg: &Value, text: &str) {
        let conn = &self.server.conns[&sock];
        if conn.joined || msg["type"] == "hello" {
            return;
        }
        if let Some(a) = conn.actor {
            self.preauth.push((a, text.to_owned()));
        }
    }

    pub(super) fn close_after_error(&mut self, sock: SockIx) {
        self.push_flow(Flow::Down(sock), 20, RelayEv::ServerClose { sock });
    }

    pub(super) fn on_sweep(&mut self, r#gen: u32) {
        if r#gen != self.server.sweep_gen || !self.server.up {
            return;
        }
        let socks: Vec<SockIx> = self.server.conns.keys().copied().collect();
        for s in socks {
            self.sweep_conn(s);
        }
        let srv = self.cur_server;
        self.push(self.now + SWEEP_MS, 3, RelayEv::Sweep { r#gen, srv });
    }

    /// One liveness round for a connection: evict it if it missed the last ping, otherwise ping.
    fn sweep_conn(&mut self, s: SockIx) {
        let Some(c) = self.server.conns.get(&s) else {
            return;
        };
        if !c.alive {
            self.server_terminate(s);
            return;
        }
        let responsive = self.responsive(c);
        if let Some(c) = self.server.conns.get_mut(&s) {
            c.alive = false;
        }
        if responsive {
            let rtt = self.rng.range(10, 80);
            self.push(self.now + rtt, 2, RelayEv::Pong { sock: s });
        }
    }
}
