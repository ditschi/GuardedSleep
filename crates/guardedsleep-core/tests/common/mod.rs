//! In-memory world for driving two or more `Link`s against a fake signaling server and a fake
//! WebRTC stack, with controllable network partitions, process restarts and time jumps.
#![allow(dead_code)]

use std::collections::{HashMap, VecDeque};

use guardedsleep_core::envelope::MessageClass;
use guardedsleep_core::invite::InviteMailbox;
use guardedsleep_core::link::{
    Link, LinkAction, LinkConfig, LinkEvent, LinkInput, LinkState, LinkStatus,
};
use guardedsleep_core::mailbox::MailboxKind;
use guardedsleep_core::pairing::{decode_psk, encode_psk};
use guardedsleep_core::protocol::{DeviceStatus, Role};
use serde_json::{Value, json};

/// `AB:CD:...` upper-case colon form of a plain hex fingerprint, as SDPs and WebRTC stats show it.
pub fn colon_fp(fp: &str) -> String {
    (0..fp.len())
        .step_by(2)
        .map(|i| fp[i..i + 2].to_uppercase())
        .collect::<Vec<_>>()
        .join(":")
}

pub const PSK_A: [u8; 32] = [7; 32];
pub const PSK_B: [u8; 32] = [9; 32];

pub const BABY: usize = 0;
pub const PARENT: usize = 1;

/// A distinct seed per role, so the two devices never draw the same session or nonce.
pub fn cfg(role: Role, psk: &[u8]) -> LinkConfig {
    let seed_byte = match role {
        Role::Baby => 5u8,
        Role::Parent => 6u8,
    };
    let mut c = LinkConfig::with_defaults(role, encode_psk(psk), &[seed_byte; 32]);
    c.mailboxes = vec![InviteMailbox {
        kind: MailboxKind::Relay,
        url: "wss://relay.test".into(),
    }];
    c.noise_threshold = 0.2;
    c.noise_hold_ms = 300;
    c
}

#[allow(unused_imports)]
pub use guardedsleep_core::testing::{
    HERE, SealedPeer, decode_message, handshake, signal_frame, signals_in,
};

/// What a node's fake WebRTC stack reports as the remote DTLS certificate fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dtls {
    /// The real remote certificate (what libwebrtc reads from `RTCCertificateStats`).
    Honest,
    /// A certificate that is not the one in the remote SDP (a man in the middle terminates DTLS).
    Mismatch,
    /// The platform never reports (stats unavailable).
    Silent,
}

struct Pc {
    fp: String,
    local: bool,
    remote: bool,
    peer: Option<(usize, u32)>,
    closed: bool,
    connected: bool,
    failed: bool,
}

pub struct Node {
    pub link: Link,
    pub cfg: LinkConfig,
    pub role: Role,
    pub alive: bool,
    pub net_up: bool,
    pub sock: Option<usize>,
    pcs: HashMap<u32, Pc>,
    pub events: Vec<LinkEvent>,
    pub created: Vec<u32>,
    pub open_signaling_calls: u32,
    pub mic_calls: Vec<bool>,
    pub apply_quiet_calls: u32,
    pub probe_tones: u32,
    pub sent_signal_types: Vec<String>,
    /// Every signal this node sent: the decrypted message and the wire payload.
    pub sent: Vec<(Value, Value)>,
    pub restarts: u32,
    /// Extra bytes of SDP, to size the offers and answers like a real browser's.
    pub sdp_pad: usize,
    pub max_level: u8,
    pub baby_direction: &'static str,
    pub dtls: Dtls,
    pub down_since: Option<u64>,
    pub create_times: Vec<u64>,
    /// What this node does to its own `parent_proof` DataChannel messages (a tampered or withheld
    /// proof); the other messages are untouched.
    pub parent_proof: ProofFault,
    /// The same for this node's `baby_proof` messages.
    pub baby_proof: ProofFault,
    /// This node never sends its `pin_ack` (a lost acknowledgement).
    pub drop_pin_ack: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofFault {
    None,
    /// One hex digit of the MAC is flipped.
    Corrupt,
    /// The message never leaves.
    Withhold,
}

struct Conn {
    node: usize,
    role: Role,
    client_id: String,
}

enum Item {
    Input(usize, LinkInput),
    Sig(usize, usize, LinkInput),
}

pub struct World {
    pub now: u64,
    pub nodes: Vec<Node>,
    conns: Vec<Option<Conn>>,
    queue: VecDeque<Item>,
    delayed: Vec<(u64, usize, usize, LinkInput)>,
    /// TCP semantics: signaling bytes wait out a short partition instead of vanishing.
    held_to_node: Vec<(usize, usize, Value)>,
    held_to_server: Vec<(usize, String)>,
    kill_timers: Vec<(u64, usize, usize)>,
    pub parent_cap: usize,
    pub dead_after_ms: u64,
    pub pc_fail_after_ms: u64,
    pub block_media: bool,
    /// The relay answers every `hello` with `UNSUPPORTED_PROTOCOL`.
    pub reject_hello: bool,
    next_client: u32,
    fp_seq: u32,
    pub server_signal_log: Vec<(String, Value)>,
    /// Every frame any node sent to the server, in order (node, text).
    pub frames: Vec<(usize, String)>,
    /// Executed actions that matter for ordering: `pinned:<node>` for a `ParentPinned` event and
    /// `pin:<node>` for the DataChannel message that carries the secret, in execution order.
    pub order: Vec<String>,
    pub impostors: Vec<Impostor>,
}

/// A room member that holds the PSK but is no `Link`: scripted by the test.
pub struct Impostor {
    pub id: String,
    pub peer: SealedPeer,
    sock: usize,
    inbox: Vec<Value>,
}

impl World {
    pub fn empty() -> Self {
        World {
            now: 0,
            nodes: Vec::new(),
            conns: Vec::new(),
            queue: VecDeque::new(),
            delayed: Vec::new(),
            held_to_node: Vec::new(),
            held_to_server: Vec::new(),
            kill_timers: Vec::new(),
            parent_cap: 1,
            dead_after_ms: 20_000,
            pc_fail_after_ms: 30_000,
            block_media: false,
            reject_hello: false,
            next_client: 0,
            fp_seq: 0,
            server_signal_log: Vec::new(),
            frames: Vec::new(),
            order: Vec::new(),
            impostors: Vec::new(),
        }
    }

    /// Baby is node 0, parent node 1; neither is started.
    pub fn pair(baby_psk: &[u8], parent_psk: &[u8]) -> Self {
        let mut w = Self::empty();
        w.add_node(cfg(Role::Baby, baby_psk));
        w.add_node(cfg(Role::Parent, parent_psk));
        w
    }

    pub fn add_node(&mut self, mut cfg: LinkConfig) -> usize {
        // Every node draws its own session and nonces.
        cfg.seed = vec![0x40 + self.nodes.len() as u8; 32];
        let role = cfg.role;
        self.nodes.push(Node {
            link: Link::new(cfg.clone()).expect("valid config"),
            cfg,
            role,
            alive: true,
            net_up: true,
            sock: None,
            pcs: HashMap::new(),
            events: Vec::new(),
            created: Vec::new(),
            open_signaling_calls: 0,
            mic_calls: Vec::new(),
            apply_quiet_calls: 0,
            probe_tones: 0,
            sent_signal_types: Vec::new(),
            sent: Vec::new(),
            restarts: 0,
            sdp_pad: 0,
            max_level: 0,
            baby_direction: "sendonly",
            dtls: Dtls::Honest,
            down_since: None,
            create_times: Vec::new(),
            parent_proof: ProofFault::None,
            baby_proof: ProofFault::None,
            drop_pin_ack: false,
        });
        self.nodes.len() - 1
    }

    // ---- driving ---------------------------------------------------------------------------

    pub fn start(&mut self, n: usize, entropy: u64) {
        self.input(n, LinkInput::Start { entropy });
    }

    pub fn input(&mut self, n: usize, input: LinkInput) {
        self.queue.push_back(Item::Input(n, input));
        self.drain();
    }

    pub fn stop(&mut self, n: usize) {
        self.input(n, LinkInput::Stop);
    }

    pub fn advance(&mut self, total_ms: u64) {
        self.advance_by(total_ms, 500);
    }

    pub fn advance_by(&mut self, total_ms: u64, step_ms: u64) {
        let end = self.now + total_ms;
        while self.now < end {
            let step = step_ms.min(end - self.now);
            self.now += step;
            self.step();
        }
    }

    fn step(&mut self) {
        let now = self.now;
        let due: Vec<_> = self
            .delayed
            .iter()
            .filter(|d| d.0 <= now)
            .cloned()
            .collect();
        self.delayed.retain(|d| d.0 > now);
        for (_, n, s, input) in due {
            self.queue.push_back(Item::Sig(n, s, input));
        }
        let kills: Vec<_> = self
            .kill_timers
            .iter()
            .filter(|k| k.0 <= now)
            .cloned()
            .collect();
        self.kill_timers.retain(|k| k.0 > now);
        for (_, n, s) in kills {
            if !self.nodes[n].net_up && self.conns.get(s).is_some_and(Option::is_some) {
                self.remove_conn(s);
            }
        }
        self.pc_failures();
        self.connect_all();
        for n in 0..self.nodes.len() {
            if self.nodes[n].alive {
                self.queue.push_back(Item::Input(n, LinkInput::Tick));
            }
        }
        self.drain();
        self.check_invariants();
    }

    /// Phones sleeping: the clock jumps without any tick in between.
    pub fn jump(&mut self, ms: u64) {
        self.now += ms;
    }

    fn drain(&mut self) {
        while let Some(item) = self.queue.pop_front() {
            match item {
                Item::Input(n, input) => self.deliver(n, input),
                Item::Sig(n, s, input) => {
                    if self.nodes[n].sock == Some(s) && self.nodes[n].alive {
                        self.deliver(n, input);
                    }
                }
            }
        }
    }

    fn deliver(&mut self, n: usize, input: LinkInput) {
        if !self.nodes[n].alive {
            return;
        }
        if std::env::var("GS_TRACE").is_ok() {
            println!("{:>7} n{n} <- {input:?}", self.now);
        }
        let acts = self.nodes[n].link.handle(self.now, input);
        if std::env::var("GS_TRACE").is_ok() {
            for a in &acts {
                println!("        n{n} -> {a:?}");
            }
        }
        self.exec(n, acts);
    }

    fn check_invariants(&self) {
        for (i, node) in self.nodes.iter().enumerate() {
            let live = node.pcs.values().filter(|p| !p.closed).count();
            assert!(live <= 1, "node {i} has {live} live attempts");
        }
    }

    // ---- network and process control -------------------------------------------------------

    pub fn set_net(&mut self, n: usize, up: bool) {
        let was = self.nodes[n].net_up;
        self.nodes[n].net_up = up;
        if was && !up {
            self.nodes[n].down_since = Some(self.now);
            if let Some(s) = self.nodes[n].sock {
                self.kill_timers.push((self.now + self.dead_after_ms, n, s));
            }
        }
        if !was && up {
            self.nodes[n].down_since = None;
            self.queue
                .push_back(Item::Input(n, LinkInput::NetworkAvailable));
            let to_server: Vec<_> = self
                .held_to_server
                .iter()
                .filter(|(m, _)| *m == n)
                .cloned()
                .collect();
            self.held_to_server.retain(|(m, _)| *m != n);
            for (_, text) in to_server {
                self.node_to_server(n, &text);
            }
            let to_node: Vec<_> = self
                .held_to_node
                .iter()
                .filter(|(m, _, _)| *m == n)
                .cloned()
                .collect();
            self.held_to_node.retain(|(m, _, _)| *m != n);
            for (_, sock, msg) in to_node {
                if self.conns.get(sock).is_some_and(Option::is_some) {
                    self.server_to_node(sock, msg);
                }
            }
            self.drain();
        }
    }

    /// Process death and restart: sockets die, peers are not told, state is gone.
    pub fn restart(&mut self, n: usize, entropy: u64) {
        if let Some(s) = self.nodes[n].sock.take()
            && self.nodes[n].net_up
        {
            self.remove_conn(s);
        }
        let mut cfg = self.nodes[n].cfg.clone();
        cfg.paired_before = true;
        // A restart is a new run: a new seed, as the platforms draw one per run.
        self.nodes[n].restarts += 1;
        cfg.seed = vec![0x80 + (n as u8) * 16 + self.nodes[n].restarts as u8; 32];
        let node = &mut self.nodes[n];
        node.cfg = cfg.clone();
        node.link = Link::new(cfg).expect("valid config");
        node.pcs.clear();
        node.alive = true;
        self.start(n, entropy);
    }

    pub fn server_restart(&mut self) {
        let socks: Vec<usize> = (0..self.conns.len())
            .filter(|s| self.conns[*s].is_some())
            .collect();
        for s in socks {
            let n = self.conns[s].as_ref().map(|c| c.node);
            self.conns[s] = None;
            if let Some(n) = n.filter(|n| *n < self.nodes.len())
                && self.nodes[n].sock == Some(s)
                && self.nodes[n].net_up
            {
                self.queue.push_back(Item::Sig(
                    n,
                    s,
                    LinkInput::MailboxClosed {
                        id: 0,
                        reason: "closed".into(),
                    },
                ));
            }
        }
        self.drain();
    }

    /// A stale baby socket the server has not noticed yet.
    pub fn add_ghost_baby(&mut self) -> usize {
        let s = self.conns.len();
        self.next_client += 1;
        self.conns.push(Some(Conn {
            node: usize::MAX,
            role: Role::Baby,
            client_id: format!("ghost{}", self.next_client),
        }));
        s
    }

    pub fn remove_ghost(&mut self, sock: usize) {
        self.remove_conn(sock);
    }

    pub fn client_id(&self, n: usize) -> Option<String> {
        let s = self.nodes[n].sock?;
        self.conns.get(s)?.as_ref().map(|c| c.client_id.clone())
    }

    // ---- observation -----------------------------------------------------------------------

    pub fn status(&self, n: usize) -> LinkStatus {
        self.nodes[n]
            .link
            .status()
            .cloned()
            .unwrap_or_else(|| panic!("node {n} has no status"))
    }

    pub fn state(&self, n: usize) -> LinkState {
        self.status(n).state
    }

    pub fn mark(&self, n: usize) -> usize {
        self.nodes[n].events.len()
    }

    pub fn events_since(&self, n: usize, mark: usize) -> &[LinkEvent] {
        &self.nodes[n].events[mark..]
    }

    pub fn count_events(&self, n: usize, mark: usize, pred: impl Fn(&LinkEvent) -> bool) -> usize {
        self.events_since(n, mark)
            .iter()
            .filter(|e| pred(e))
            .count()
    }

    pub fn live_attempts(&self, n: usize) -> usize {
        self.nodes[n].pcs.values().filter(|p| !p.closed).count()
    }

    pub fn reset_max_level(&mut self, n: usize) {
        self.nodes[n].max_level = 0;
    }

    pub fn connect_pair(&mut self) {
        self.start(BABY, 1);
        self.start(PARENT, 2);
        self.advance(5_000);
        assert_eq!(self.state(BABY), LinkState::Connected);
        assert_eq!(self.state(PARENT), LinkState::Connected);
    }

    // ---- action execution ------------------------------------------------------------------

    fn exec(&mut self, n: usize, actions: Vec<LinkAction>) {
        for a in actions {
            match a {
                // Only the relay has a fake server here.
                LinkAction::OpenMailbox { id: 0, .. } => self.open_signaling(n),
                LinkAction::OpenMailbox { .. } => {}
                LinkAction::CloseMailbox { id: 0 } => self.close_signaling(n),
                LinkAction::CloseMailbox { .. } => {}
                LinkAction::MailboxSend { id: 0, frame } => self.node_to_server(n, &frame),
                LinkAction::MailboxSend { .. } => {}
                LinkAction::CreateAttempt { attempt, .. } => {
                    assert!(
                        self.nodes[n].pcs.values().all(|p| p.closed),
                        "node {n}: create_attempt {attempt} while another attempt is live"
                    );
                    self.fp_seq += 1;
                    let fp = format!("{:064x}", u64::from(self.fp_seq) * 7919 + 17);
                    let node = &mut self.nodes[n];
                    node.created.push(attempt);
                    node.create_times.push(self.now);
                    node.pcs.insert(
                        attempt,
                        Pc {
                            fp,
                            local: false,
                            remote: false,
                            peer: None,
                            closed: false,
                            connected: false,
                            failed: false,
                        },
                    );
                }
                LinkAction::CloseAttempt { attempt } => self.close_attempt(n, attempt),
                LinkAction::CreateOffer { attempt } => self.local_description(n, attempt, "offer"),
                LinkAction::CreateAnswer { attempt } => {
                    self.local_description(n, attempt, "answer")
                }
                LinkAction::SetRemoteDescription { attempt, sdp, .. } => {
                    self.set_remote(n, attempt, &sdp);
                }
                LinkAction::AddIceCandidate { .. } => {}
                LinkAction::SendData { attempt, text } => {
                    if text.contains("\"parent_pin\"") {
                        self.order.push(format!("pin:{n}"));
                    }
                    self.send_data(n, attempt, &text)
                }
                LinkAction::SetMicEnabled { enabled } => self.nodes[n].mic_calls.push(enabled),
                LinkAction::ApplyQuiet => self.nodes[n].apply_quiet_calls += 1,
                LinkAction::SendProbeTone { .. } => self.nodes[n].probe_tones += 1,
                LinkAction::Emit { event } => {
                    if let LinkEvent::Status { status } = &event {
                        let node = &mut self.nodes[n];
                        node.max_level = node.max_level.max(status.level);
                    }
                    // Like a platform: persist the pinned parent secret, so that a restarted run
                    // (`restart`) is configured with it.
                    if let LinkEvent::ParentPinned {
                        secret,
                        window_until_s,
                    } = &event
                    {
                        self.order.push(format!("pinned:{n}"));
                        self.nodes[n].cfg.pinned_parent = Some(secret.clone());
                        self.nodes[n].cfg.pin_window_until_s = *window_until_s;
                    }
                    self.nodes[n].events.push(event);
                }
            }
        }
    }

    fn open_signaling(&mut self, n: usize) {
        self.nodes[n].open_signaling_calls += 1;
        let s = self.conns.len();
        self.conns.push(None);
        self.nodes[n].sock = Some(s);
        if self.nodes[n].net_up {
            self.queue
                .push_back(Item::Sig(n, s, LinkInput::MailboxOpened { id: 0 }));
        } else {
            self.delayed.push((
                self.now + 3_000,
                n,
                s,
                LinkInput::MailboxClosed {
                    id: 0,
                    reason: "closed".into(),
                },
            ));
        }
    }

    fn close_signaling(&mut self, n: usize) {
        if let Some(s) = self.nodes[n].sock.take()
            && self.conns.get(s).is_some_and(Option::is_some)
        {
            if self.nodes[n].net_up {
                self.remove_conn(s);
            } else {
                self.kill_timers.push((self.now + self.dead_after_ms, n, s));
            }
        }
    }

    fn server_to_node(&mut self, sock: usize, msg: Value) {
        let Some(Some(conn)) = self.conns.get(sock) else {
            return;
        };
        let n = conn.node;
        if n >= self.nodes.len() {
            return;
        }
        if !self.nodes[n].net_up {
            self.held_to_node.push((n, sock, msg));
            return;
        }
        self.queue.push_back(Item::Sig(
            n,
            sock,
            LinkInput::MailboxFrame {
                id: 0,
                bytes: msg.to_string(),
            },
        ));
    }

    fn broadcast_members(&mut self) {
        let peers: Vec<Value> = self
            .conns
            .iter()
            .flatten()
            .map(|c| json!({ "clientId": c.client_id, "role": c.role.as_str() }))
            .collect();
        let socks: Vec<usize> = (0..self.conns.len())
            .filter(|s| self.conns[*s].is_some())
            .collect();
        for s in socks {
            self.server_to_node(s, json!({ "type": "members", "peers": peers }));
        }
    }

    fn remove_conn(&mut self, sock: usize) {
        if self.conns.get(sock).is_some_and(Option::is_some) {
            self.conns[sock] = None;
            self.broadcast_members();
        }
    }

    fn node_to_server(&mut self, n: usize, text: &str) {
        let Some(sock) = self.nodes[n].sock else {
            return;
        };
        if !self.nodes[n].net_up {
            self.held_to_server.push((n, text.to_owned()));
            return;
        }
        let server_has = self.conns.get(sock).is_some_and(Option::is_some);
        let msg: Value = serde_json::from_str(text).expect("link sends valid JSON");
        let kind = msg["type"].as_str().unwrap_or("").to_owned();
        self.frames.push((n, text.to_owned()));
        // A reachability check pings a socket that never said hello; the server just answers.
        if kind == "ping" && !server_has {
            return;
        }
        if kind == "hello" {
            self.hello(n, sock, &msg);
            return;
        }
        if !server_has {
            // TCP reset on the first send after the server dropped the socket.
            self.queue.push_back(Item::Sig(
                n,
                sock,
                LinkInput::MailboxClosed {
                    id: 0,
                    reason: "closed".into(),
                },
            ));
            return;
        }
        match kind.as_str() {
            "ping" => self.server_to_node(sock, json!({ "type": "pong" })),
            "signal" => {
                let from = self.conns[sock]
                    .as_ref()
                    .expect("checked")
                    .client_id
                    .clone();
                let payload = msg["payload"].clone();
                self.server_signal_log.push((from.clone(), payload.clone()));
                let psk = decode_psk(&self.nodes[n].cfg.psk_b64url).expect("psk");
                if let Some(message) = decode_message(&psk, &payload) {
                    if let Some(t) = message["type"].as_str() {
                        self.nodes[n].sent_signal_types.push(t.to_owned());
                    }
                    self.nodes[n].sent.push((message, payload.clone()));
                }
                let to = msg["to"].as_str().unwrap_or("").to_owned();
                if let Some(i) = self.impostors.iter().position(|im| im.id == to) {
                    self.impostors[i].inbox.push(payload);
                    return;
                }
                let target = (0..self.conns.len()).find(|s| {
                    self.conns[*s]
                        .as_ref()
                        .is_some_and(|c| c.client_id == to && *s != sock)
                });
                if let Some(t) = target {
                    self.server_to_node(
                        t,
                        json!({ "type": "signal", "from": from, "payload": payload }),
                    );
                }
            }
            other => panic!("unexpected client message {other}"),
        }
    }

    fn hello(&mut self, n: usize, sock: usize, msg: &Value) {
        let role = if msg["role"] == "baby" {
            Role::Baby
        } else {
            Role::Parent
        };
        let err = |code: &str| json!({ "type": "error", "code": code, "message": "x" });
        if msg["protocolVersion"] != 3 || self.reject_hello {
            self.conns[sock] = Some(Conn {
                node: n,
                role,
                client_id: String::new(),
            });
            self.server_to_node(sock, err("UNSUPPORTED_PROTOCOL"));
            self.conns[sock] = None;
            return;
        }
        let babies = self
            .conns
            .iter()
            .flatten()
            .filter(|c| c.role == Role::Baby)
            .count();
        let parents = self
            .conns
            .iter()
            .flatten()
            .filter(|c| c.role == Role::Parent)
            .count();
        let refusal = match role {
            Role::Baby if babies > 0 => Some("BABY_TAKEN"),
            Role::Parent if parents >= self.parent_cap => Some("PARENT_CAP"),
            _ => None,
        };
        self.next_client += 1;
        let client_id = format!("c{}", self.next_client);
        self.conns[sock] = Some(Conn {
            node: n,
            role,
            client_id: client_id.clone(),
        });
        if let Some(code) = refusal {
            self.server_to_node(sock, err(code));
            self.conns[sock] = None;
            return;
        }
        // The harness aliases rooms: it echoes the room id the client asked for, which is how an
        // outsider who only knows the room id (not the PSK) ends up next to a real peer.
        self.server_to_node(
            sock,
            json!({ "type": "welcome", "clientId": client_id, "roomId": msg["roomId"] }),
        );
        self.broadcast_members();
    }

    fn sdp(&self, n: usize, fp: &str) -> String {
        let dir = match self.nodes[n].role {
            Role::Baby => self.nodes[n].baby_direction,
            Role::Parent => "recvonly",
        };
        let pad = match self.nodes[n].sdp_pad {
            0 => String::new(),
            k => format!("a=x-pad:{}\r\n", "x".repeat(k)),
        };
        format!(
            "v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na={dir}\r\n{pad}a=fingerprint:sha-256 {}\r\n",
            colon_fp(fp)
        )
    }

    fn local_description(&mut self, n: usize, attempt: u32, sdp_type: &str) {
        let Some(pc) = self.nodes[n].pcs.get_mut(&attempt) else {
            return;
        };
        pc.local = true;
        let fp = pc.fp.clone();
        let sdp = self.sdp(n, &fp);
        self.queue.push_back(Item::Input(
            n,
            LinkInput::AttemptLocalDescription {
                attempt,
                sdp_type: sdp_type.into(),
                sdp,
            },
        ));
    }

    fn set_remote(&mut self, n: usize, attempt: u32, sdp: &str) {
        let fp: String = sdp
            .lines()
            .find_map(|l| l.strip_prefix("a=fingerprint:sha-256 "))
            .map(|v| v.replace(':', "").to_lowercase())
            .unwrap_or_default();
        let mut peer = None;
        for (m, node) in self.nodes.iter().enumerate() {
            if m == n {
                continue;
            }
            for (h, pc) in &node.pcs {
                if pc.fp == fp {
                    peer = Some((m, *h));
                }
            }
        }
        if let Some(pc) = self.nodes[n].pcs.get_mut(&attempt) {
            pc.remote = true;
            pc.peer = peer;
        }
    }

    fn connect_all(&mut self) {
        if self.block_media {
            return;
        }
        let mut ready = Vec::new();
        for (n, node) in self.nodes.iter().enumerate() {
            for (h, pc) in &node.pcs {
                let Some((m, hp)) = pc.peer else { continue };
                let Some(other) = self.nodes[m].pcs.get(&hp) else {
                    continue;
                };
                if pc.local
                    && pc.remote
                    && other.local
                    && other.remote
                    && !pc.closed
                    && !other.closed
                    && !pc.connected
                    && !pc.failed
                    && !other.failed
                    && node.net_up
                    && self.nodes[m].net_up
                    && n < m
                {
                    ready.push((n, *h, m, hp));
                }
            }
        }
        for (n, h, m, hp) in ready {
            for (x, hx) in [(n, h), (m, hp)] {
                self.nodes[x].pcs.get_mut(&hx).expect("pc").connected = true;
                let remote = if x == n { (m, hp) } else { (n, h) };
                let real = colon_fp(&self.nodes[remote.0].pcs[&remote.1].fp);
                self.queue.push_back(Item::Input(
                    x,
                    LinkInput::AttemptPcState {
                        attempt: hx,
                        state: "connected".into(),
                    },
                ));
                self.queue
                    .push_back(Item::Input(x, LinkInput::AttemptDataOpen { attempt: hx }));
                let fingerprint = match self.nodes[x].dtls {
                    Dtls::Honest => Some(real),
                    Dtls::Mismatch => Some(colon_fp(&"ee".repeat(32))),
                    Dtls::Silent => None,
                };
                if let Some(fingerprint) = fingerprint {
                    self.queue.push_back(Item::Input(
                        x,
                        LinkInput::AttemptDtlsFingerprint {
                            attempt: hx,
                            fingerprint,
                        },
                    ));
                }
            }
        }
    }

    fn pc_failures(&mut self) {
        for n in 0..self.nodes.len() {
            let Some(since) = self.nodes[n].down_since else {
                continue;
            };
            if self.now.saturating_sub(since) < self.pc_fail_after_ms {
                continue;
            }
            let handles: Vec<u32> = self.nodes[n]
                .pcs
                .iter()
                .filter(|(_, p)| p.connected && !p.closed && !p.failed)
                .map(|(h, _)| *h)
                .collect();
            for h in handles {
                let peer = self.nodes[n].pcs[&h].peer;
                self.nodes[n].pcs.get_mut(&h).expect("pc").failed = true;
                self.queue.push_back(Item::Input(
                    n,
                    LinkInput::AttemptPcState {
                        attempt: h,
                        state: "failed".into(),
                    },
                ));
                if let Some((m, hp)) = peer
                    && let Some(p) = self.nodes[m].pcs.get_mut(&hp)
                    && !p.failed
                    && !p.closed
                {
                    p.failed = true;
                    self.queue.push_back(Item::Input(
                        m,
                        LinkInput::AttemptPcState {
                            attempt: hp,
                            state: "failed".into(),
                        },
                    ));
                }
            }
        }
    }

    fn close_attempt(&mut self, n: usize, attempt: u32) {
        let Some(pc) = self.nodes[n].pcs.get_mut(&attempt) else {
            return;
        };
        pc.closed = true;
        let (connected, peer) = (pc.connected, pc.peer);
        if let Some((m, hp)) = peer {
            let open = self.nodes[m].pcs.get(&hp).is_some_and(|p| !p.closed);
            if connected && open && self.nodes[n].net_up && self.nodes[m].net_up {
                self.queue
                    .push_back(Item::Input(m, LinkInput::AttemptDataClosed { attempt: hp }));
            }
        }
    }

    fn send_data(&mut self, n: usize, attempt: u32, text: &str) {
        let Some(pc) = self.nodes[n].pcs.get(&attempt) else {
            return;
        };
        if !pc.connected || pc.closed || pc.failed || !self.nodes[n].net_up {
            return;
        }
        let Some((m, hp)) = pc.peer else { return };
        let Some(other) = self.nodes[m].pcs.get(&hp) else {
            return;
        };
        if other.closed || other.failed || !other.connected || !self.nodes[m].net_up {
            return;
        }
        let mut text = text.to_owned();
        if text.contains("\"pin_ack\"") && self.nodes[n].drop_pin_ack {
            return;
        }
        for (marker, fault) in [
            ("\"parent_proof\"", self.nodes[n].parent_proof),
            ("\"baby_proof\"", self.nodes[n].baby_proof),
        ] {
            if !text.contains(marker) {
                continue;
            }
            match fault {
                ProofFault::None => {}
                ProofFault::Withhold => return,
                ProofFault::Corrupt => {
                    let mut v: Value = serde_json::from_str(&text).expect("json");
                    let mac = v["mac"].as_str().unwrap_or("").to_owned();
                    let flipped = if mac.starts_with('0') { "1" } else { "0" };
                    v["mac"] = json!(format!("{flipped}{}", &mac[1..]));
                    text = v.to_string();
                }
            }
        }
        self.queue.push_back(Item::Input(
            m,
            LinkInput::AttemptDataMessage { attempt: hp, text },
        ));
    }

    /// Delivers a DataChannel text to the live attempt of `n`, as if the peer had sent it.
    pub fn inject_data(&mut self, n: usize, text: &str) {
        let h = self.nodes[n]
            .pcs
            .iter()
            .find(|(_, p)| !p.closed)
            .map(|(h, _)| *h)
            .expect("live attempt");
        self.input(
            n,
            LinkInput::AttemptDataMessage {
                attempt: h,
                text: text.to_owned(),
            },
        );
    }

    /// Adds a PSK-holding room member that the test scripts.
    pub fn add_impostor(&mut self, id: &str, role: Role, psk: &[u8], seed: u8) -> usize {
        let sock = self.conns.len();
        self.conns.push(Some(Conn {
            node: usize::MAX,
            role,
            client_id: id.to_owned(),
        }));
        self.impostors.push(Impostor {
            id: id.to_owned(),
            peer: SealedPeer::new(role, psk, seed),
            sock,
            inbox: Vec::new(),
        });
        self.broadcast_members();
        self.drain();
        self.impostors.len() - 1
    }

    pub fn remove_impostor(&mut self, i: usize) {
        let sock = self.impostors[i].sock;
        self.remove_conn(sock);
        self.drain();
    }

    /// Sends the impostor's `here` to node `n` and settles the confirmation handshake.
    pub fn impostor_greets(&mut self, i: usize, n: usize) {
        let id = self.impostors[i].id.clone();
        let here = self.impostors[i].peer.here();
        self.inject_signal(n, &id, here);
        self.impostor_settle(i, n);
    }

    /// Answers everything node `n` sent to the impostor until both sides are quiet.
    pub fn impostor_settle(&mut self, i: usize, n: usize) {
        for _ in 0..10 {
            let inbox = std::mem::take(&mut self.impostors[i].inbox);
            if inbox.is_empty() {
                return;
            }
            let id = self.impostors[i].id.clone();
            for payload in inbox {
                let now = self.now;
                if let Ok((_, Some(reply))) = self.impostors[i].peer.accept(now, &payload) {
                    self.inject_signal(n, &id, reply);
                }
            }
        }
    }

    /// A message the impostor seals to node `n`'s confirmed session and delivers.
    pub fn impostor_says(&mut self, i: usize, n: usize, class: MessageClass, message: &Value) {
        let id = self.impostors[i].id.clone();
        let payload = self.impostors[i].peer.say(class, message);
        self.inject_signal(n, &id, payload);
    }

    /// The decrypted messages of type `ty` that node `n` sent, oldest first.
    pub fn sent_of(&self, n: usize, ty: &str) -> Vec<(Value, Value)> {
        self.nodes[n]
            .sent
            .iter()
            .filter(|(m, _)| m["type"] == ty)
            .cloned()
            .collect()
    }

    /// Delivers a signaling `signal` message to `n` from an arbitrary client id.
    pub fn inject_signal(&mut self, n: usize, from: &str, payload: Value) {
        let text = json!({ "type": "signal", "from": from, "payload": payload }).to_string();
        self.input(n, LinkInput::MailboxFrame { id: 0, bytes: text });
    }

    /// Chooses what node `n` reports as the remote DTLS fingerprint from now on.
    pub fn set_dtls(&mut self, n: usize, dtls: Dtls) {
        self.nodes[n].dtls = dtls;
    }

    /// The SDP fingerprint (plain hex) of the live attempt of `n`; `None` without one.
    pub fn live_fp(&self, n: usize) -> Option<String> {
        self.nodes[n]
            .pcs
            .values()
            .find(|p| !p.closed)
            .map(|p| p.fp.clone())
    }

    pub fn status_input(&mut self, n: usize, status: DeviceStatus) {
        self.input(n, LinkInput::DeviceStatus { status });
    }
}

pub fn ok_status() -> DeviceStatus {
    DeviceStatus::default()
}
