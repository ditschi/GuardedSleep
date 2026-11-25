//! The client side of signaling: sockets, reachability and the frames that wait for the network.

use super::*;

/// The close notice a client gets for `sock`.
pub(super) fn closed_to(actor: ActorIx, sock: SockIx) -> RelayEv {
    RelayEv::SigToClient {
        actor,
        sock,
        msg: ToClient::Closed,
    }
}

/// Whether the message is the relay's `welcome` frame.
fn is_welcome(msg: &ToClient) -> bool {
    let ToClient::Text(text) = msg else {
        return false;
    };
    serde_json::from_str::<Value>(text)
        .ok()
        .is_some_and(|v| v["type"].as_str() == Some("welcome"))
}

impl World {
    pub(super) fn reach(&self, a: ActorIx, epoch: u32) -> bool {
        let ac = &self.actors[a];
        ac.alive && ac.net_up && ac.path_epoch == epoch
    }

    /// The path would work again once the network returns.
    pub(super) fn viable(&self, a: ActorIx, epoch: u32) -> bool {
        let ac = &self.actors[a];
        ac.alive && ac.path_epoch == epoch
    }

    pub(super) fn responsive(&self, c: &Conn) -> bool {
        c.actor_ix().is_some_and(|a| self.reach(a, c.epoch))
    }

    /// The mailbox (and relay) a socket belongs to.
    fn mailbox_of(&self, sock: SockIx) -> MailboxId {
        self.sock_server.get(&sock).copied().unwrap_or(0) as MailboxId
    }

    /// A frame for the client, after the usual jitter and behind earlier frames of the socket.
    pub(super) fn queue_to_client(&mut self, a: ActorIx, sock: SockIx, msg: ToClient) {
        let lat = self.rng.range(5, 40);
        let ev = RelayEv::SigToClient {
            actor: a,
            sock,
            msg,
        };
        self.push_flow(Flow::Down(sock), lat, ev);
    }

    /// A frame for the relay, delayed as `delivery` says.
    fn queue_to_server(&mut self, sock: SockIx, frame: Frame, delivery: Delivery) {
        let lat = self.rng.range(5, 40) + delivery.extra;
        let t = self.fifo_time(Flow::Up(sock), self.now + lat, delivery.bypass);
        self.push(t, 2, RelayEv::SigToServer { sock, frame });
    }

    /// The close of `sock` reaches the relay shortly after the client side let go of it.
    pub(super) fn queue_server_close(&mut self, sock: SockIx) {
        self.push_flow(Flow::Up(sock), 20, RelayEv::ServerClose { sock });
    }

    /// Tells the relay about a closed socket, if the client can still reach it.
    pub(super) fn close_toward_server(&mut self, a: ActorIx, sock: SockIx) {
        let reachable = self
            .sock_owner(sock)
            .is_some_and(|o| self.reach(a, o.epoch));
        if reachable {
            self.queue_server_close(sock);
        }
    }

    pub fn open_signaling(&mut self, a: impl Into<ActorIx>) {
        self.open_mailbox(a, 0);
    }

    /// Opens mailbox `id` of actor `a`: a socket to simulated relay `id`.
    pub fn open_mailbox(&mut self, a: impl Into<ActorIx>, id: MailboxId) {
        let a = a.into();
        let sock = SockIx::from(self.next_sock);
        self.next_sock += 1;
        let epoch = self.actors[a].path_epoch;
        self.socks.insert(sock, SockOwner { actor: a, epoch });
        self.sock_server.insert(sock, id as usize);
        self.actors[a].socks.insert(id, sock.index());
        self.select_server(id as usize);
        let (up, reach) = (self.server.up, self.reach(a, epoch));
        let (delay, ev) = if !reach {
            (3_000 + self.rng.range(0, 500), closed_to(a, sock))
        } else if !up {
            (self.rng.range(20, 80), closed_to(a, sock))
        } else {
            (self.rng.range(5, 40), RelayEv::Accept { sock })
        };
        self.push(self.now + delay, 2, ev);
    }

    pub fn client_send(&mut self, a: impl Into<ActorIx>, text: String, spoof: Option<String>) {
        self.client_send_on(a, 0, text, spoof);
    }

    pub fn client_send_on(
        &mut self,
        a: impl Into<ActorIx>,
        id: MailboxId,
        text: String,
        spoof: Option<String>,
    ) {
        let a = a.into();
        let Some(sock) = self.actors[a].socks.get(&id).copied().map(SockIx::from) else {
            return;
        };
        self.note_sent_signal(a, &text);
        let frame = Frame { text, spoof };
        for delivery in self.plan(a, Via::Signaling, &frame.text) {
            self.send_up(a, sock, &frame, delivery);
        }
    }

    /// Records what a frame is, for the `sent_signal` assertions.
    fn note_sent_signal(&mut self, a: ActorIx, text: &str) {
        if let Some(t) = self.signal_type_for(a, text) {
            let now = self.now;
            self.actors[a].sent_signals.push((now, t));
        }
    }

    /// Sends now when the path works, holds the frame for the network's return when it may.
    fn send_up(&mut self, a: ActorIx, sock: SockIx, frame: &Frame, delivery: Delivery) {
        let epoch = self.socks[&sock].epoch;
        if self.reach(a, epoch) {
            self.queue_to_server(sock, frame.clone(), delivery);
        } else if self.viable(a, epoch) {
            self.actors[a].held_up.push((sock, frame.text.clone()));
        }
    }

    pub(super) fn on_client_rx(&mut self, a: ActorIx, sock: SockIx, msg: ToClient) {
        let id = self.mailbox_of(sock);
        let current = self.actors[a].socks.get(&id) == Some(&sock.index());
        if !current || !self.actors[a].alive {
            return;
        }
        let epoch = self.socks[&sock].epoch;
        if !matches!(msg, ToClient::Closed) && !self.reach(a, epoch) {
            if self.viable(a, epoch) {
                self.actors[a].held_down.push((sock, msg));
            }
            return;
        }
        self.receive_down(a, id, msg);
    }

    fn receive_down(&mut self, a: ActorIx, id: MailboxId, msg: ToClient) {
        if is_welcome(&msg) {
            let now = self.now;
            self.actors[a].welcomes.push(now);
        }
        if matches!(msg, ToClient::Closed) {
            self.actors[a].socks.remove(&id);
        }
        if self.actors[a].raw.is_some() {
            self.raw_on_signaling(a.index(), msg);
            return;
        }
        let input = match msg {
            ToClient::Opened => LinkInput::MailboxOpened { id },
            ToClient::Text(bytes) => LinkInput::MailboxFrame { id, bytes },
            ToClient::Closed => LinkInput::MailboxClosed {
                id,
                reason: "closed".into(),
            },
        };
        self.deliver(a, input);
    }

    pub(super) fn flush_after_up(&mut self, a: ActorIx, emit_available: bool) {
        self.flush_held_up(a);
        self.flush_held_down(a);
        if emit_available && self.actors[a].link.is_some() {
            let d = self.rng.range(50, 400);
            self.push_input(a, d, LinkInput::NetworkAvailable);
        }
    }

    fn flush_held_up(&mut self, a: ActorIx) {
        for (sock, text) in std::mem::take(&mut self.actors[a].held_up) {
            let Some(owner) = self.sock_owner(sock) else {
                continue;
            };
            if self.viable(a, owner.epoch) {
                let frame = Frame { text, spoof: None };
                self.queue_to_server(sock, frame, Delivery::PLAIN);
            }
        }
    }

    fn flush_held_down(&mut self, a: ActorIx) {
        for (sock, msg) in std::mem::take(&mut self.actors[a].held_down) {
            self.select_sock(sock);
            if self.server.conns.contains_key(&sock) {
                self.queue_to_client(a, sock, msg);
            }
        }
    }

    pub(super) fn actor_psk(&self, a: ActorIx) -> Option<Vec<u8>> {
        decode_psk(&self.actors[a].cfg.psk_b64url)
    }

    /// What a frame `a` sent is, for the "sent_signal" assertions: the frame type, or for a relay
    /// `signal` the type of the message inside (read with the sender's PSK when it is sealed).
    pub(super) fn signal_type_for(&self, a: ActorIx, text: &str) -> Option<String> {
        let v: Value = serde_json::from_str(text).ok()?;
        let t = v["type"].as_str()?;
        if t != "signal" {
            return Some(t.to_owned());
        }
        let payload = &v["payload"];
        if payload.get("env").is_some() {
            let psk = self.actor_psk(a)?;
            return decode_message(&psk, payload)?["type"]
                .as_str()
                .map(str::to_owned);
        }
        payload["type"].as_str().map(str::to_owned)
    }
}
