//! The simulated world: virtual time, a relay that mirrors `crates/guardedsleep-relay`, a fake WebRTC
//! media layer, and the `Link` instances (plus raw protocol clients) living in it.
//!
//! Everything is driven by one event queue ordered by `(time, priority, sequence)`. The only
//! source of randomness is the seeded [`Rng`], so `(scenario, seed)` fully determines a run.

use std::collections::{BTreeMap, BTreeSet};

use guardedsleep_core::ice::IceServer;
use guardedsleep_core::invite::InviteMailbox;
use guardedsleep_core::link::{
    Link, LinkAction, LinkConfig, LinkEvent, LinkInput, LinkState, LinkStatus, carries_parent_pin,
};
use guardedsleep_core::mailbox::{MailboxId, MailboxKind};
use guardedsleep_core::pairing::{
    decode_psk, derive_room_id, encode_psk, extract_sha256_fingerprint,
    extract_sha256_fingerprint_v2_legacy, pair_proof_mac_v3,
};
use guardedsleep_core::protocol::{DataMessage, Role, encode_data_message, parse_data_message};
use guardedsleep_core::testing::{decode_message, nonce_of, rewrite_message};
use serde_json::{Value, json};

use crate::canary::CanarySet;
use crate::raw::RawState;
use crate::rng::Rng;
use crate::scenario::{PeerSpec, RelaySpec, Scenario, Step};
use crate::trace::TraceRec;

pub type Time = u64;

/// Server liveness sweep period (`SIGNALING_PING_INTERVAL`, ADR-017 section 10).
const SWEEP_MS: u64 = 10_000;
/// Packets of a connection that stays broken this long make the peer connection `failed`
/// (ICE consent freshness is about 30 s).
const FAIL_AFTER_MIN: u64 = 25_000;
const FAIL_AFTER_MAX: u64 = 35_000;
const CONNECT_GIVE_UP_MS: u64 = 30_000;

/// Synthetic PSK material: recognisable by the lint, never a real secret.
pub const SYNTH_PSK_PREFIX: &[u8; 16] = b"GS-SYNTHETIC-PSK";
/// First four bytes of every synthetic DTLS fingerprint ("SYNT"), lowercase hex.
pub const SYNTH_FP_PREFIX_HEX: &str = "53594e54";
/// The simulated wall clock starts at this Unix second (a synthetic value, not a date).
const SIM_WALL_BASE_S: u64 = 1_000_000;
/// Drivers report the wall clock this often (and after a time change).
const WALL_CLOCK_EVERY_MS: u64 = 60_000;

/// Every simulation `Link` seed starts with these 16 bytes, so the lint can tell a test seed from
/// a real one.
pub const SYNTH_SEED_PREFIX: &[u8; 16] = b"GS-SYNTH-SEED-v1";

pub fn synthetic_psk(name: &str) -> Vec<u8> {
    let fill = match name {
        "B" => 0xB2,
        _ => 0xA1,
    };
    let mut k = SYNTH_PSK_PREFIX.to_vec();
    k.extend_from_slice(&[fill; 16]);
    k
}

/// The SDP as a fingerprint-confusing relay forwards it: the real value moves into an `s=` line
/// (the first occurrence a substring search finds) and the relay's own fingerprint takes the real
/// `a=fingerprint` line, so DTLS ends at the relay.
fn confuse_fingerprint(sdp: &str, mitm_fp: &str) -> String {
    let mut hidden = None;
    let mut body = String::new();
    for line in sdp.split_inclusive("\r\n") {
        if let Some(v) = line.strip_prefix("a=fingerprint:sha-256 ") {
            hidden = Some(v.trim_end().to_owned());
            body.push_str(&format!(
                "a=fingerprint:sha-256 {}\r\n",
                colon_upper(mitm_fp)
            ));
        } else {
            body.push_str(line);
        }
    }
    match (hidden, body.strip_prefix("v=0\r\n")) {
        (Some(h), Some(rest)) => format!("v=0\r\ns=a=fingerprint:sha-256 {h}\r\n{rest}"),
        _ => body,
    }
}

fn synthetic_fp(n: u64) -> String {
    let mut bytes = [0u8; 32];
    bytes[..4].copy_from_slice(b"SYNT");
    bytes[24..].copy_from_slice(&n.to_be_bytes());
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn colon_upper(fp: &str) -> String {
    (0..fp.len())
        .step_by(2)
        .map(|i| fp[i..i + 2].to_uppercase())
        .collect::<Vec<_>>()
        .join(":")
}

#[derive(Debug, Clone)]
pub enum ToClient {
    Opened,
    Text(String),
    Closed,
}

#[derive(Debug, Clone)]
enum Ev {
    Timeline(usize),
    Tick {
        actor: usize,
        chain: u32,
    },
    Accept {
        sock: usize,
    },
    SigToServer {
        sock: usize,
        text: String,
        spoof: Option<String>,
    },
    SigToClient {
        actor: usize,
        sock: usize,
        msg: ToClient,
    },
    Pong {
        sock: usize,
    },
    Sweep {
        r#gen: u32,
        srv: usize,
    },
    LocalDesc {
        actor: usize,
        handle: u32,
        sdp_type: &'static str,
    },
    PcConnect {
        a: usize,
        ha: u32,
        b: usize,
        hb: u32,
        since: Time,
    },
    Data {
        to: usize,
        handle: u32,
        text: String,
    },
    DataClosed {
        to: usize,
        handle: u32,
    },
    Input {
        actor: usize,
        input: LinkInput,
    },
    ServerClose {
        sock: usize,
    },
    PathBack {
        actor: usize,
    },
    Restart {
        actor: usize,
    },
    Watch,
    /// Half-TTL renewal of the ICE servers of the room `key` on relay `srv`.
    IceRenew {
        srv: usize,
        key: String,
    },
    RawTick {
        actor: usize,
    },
    RawAct {
        actor: usize,
    },
}

pub struct Pc {
    pub fp: String,
    /// The certificate fingerprint DTLS authenticates the remote against: what the real
    /// `a=fingerprint` line of the applied remote SDP says. The platform reports it to the core.
    pub dtls_fp: Option<String>,
    pub local: bool,
    pub remote: bool,
    pub peer: Option<(usize, u32)>,
    pub closed: bool,
    pub connected: bool,
    pub failed: bool,
    pub connecting: bool,
    pub epoch: u32,
    pub unhealthy_since: Option<Time>,
    pub fail_after: u64,
}

pub struct EvRec {
    pub t: Time,
    pub ev: Value,
    /// Milliseconds since the peer last received a DataChannel message (the core's `last_alive`).
    pub since_contact: u64,
}

pub struct Actor {
    pub name: String,
    pub role: Role,
    pub link: Option<Link>,
    pub cfg: LinkConfig,
    pub raw: Option<RawState>,
    /// A mailbox set the next `restart_process` configures the new run with.
    pending_mailboxes: Option<Vec<InviteMailbox>>,
    /// Room id this client asks the relay for.
    pub room: String,
    /// Direction of the monitoring line (first audio m-line) and of the talk line (second) in this
    /// device's local descriptions; the defaults follow the direction rule of `link-supervision`.
    pub sdp_dir: String,
    pub talk_dir: String,
    pub net_up: bool,
    pub path_epoch: u32,
    pub alive: bool,
    /// The socket of each mailbox (a mailbox id is also the index of the simulated relay).
    pub socks: BTreeMap<MailboxId, usize>,
    pub pcs: BTreeMap<u32, Pc>,
    held_up: Vec<(usize, String)>,
    held_down: Vec<(usize, ToClient)>,
    chain: u32,
    last_input_at: Time,
    pub last_rx_data: Time,
    rec: Option<usize>,
    pub instance: u32,
    pub events: Vec<EvRec>,
    pub statuses: Vec<(Time, LinkStatus)>,
    pub welcomes: Vec<Time>,
    pub sent_signals: Vec<(Time, String)>,
    pub next_raw_handle: u32,
    /// Simulated wall-clock second last reported to the link.
    last_wall_ms: Option<u64>,
    /// This device's clock is this many seconds ahead of true time (negative: behind).
    wall_offset_s: i64,
}

impl Actor {
    fn new(name: &str, role: Role, cfg: LinkConfig, room: String) -> Self {
        Actor {
            name: name.to_owned(),
            role,
            link: None,
            cfg,
            raw: None,
            pending_mailboxes: None,
            room,
            sdp_dir: match role {
                Role::Baby => "sendonly",
                Role::Parent => "recvonly",
            }
            .into(),
            talk_dir: match role {
                Role::Baby => "recvonly",
                Role::Parent => "sendonly",
            }
            .into(),
            net_up: true,
            path_epoch: 0,
            alive: true,
            socks: BTreeMap::new(),
            pcs: BTreeMap::new(),
            held_up: Vec::new(),
            held_down: Vec::new(),
            chain: 0,
            last_input_at: 0,
            last_rx_data: 0,
            rec: None,
            instance: 0,
            events: Vec::new(),
            statuses: Vec::new(),
            welcomes: Vec::new(),
            sent_signals: Vec::new(),
            next_raw_handle: 1,
            last_wall_ms: None,
            wall_offset_s: 0,
        }
    }
}

pub struct Conn {
    pub actor: Option<usize>,
    pub epoch: u32,
    pub role: Role,
    pub client_id: String,
    pub requested: String,
    pub key: String,
    pub joined: bool,
    pub alive: bool,
    pub join_seq: u64,
}

impl Server {
    fn new() -> Self {
        Server {
            drop_signals: false,
            up: true,
            conns: BTreeMap::new(),
            sweep_gen: 0,
            next_client: 0,
            next_join: 0,
            last_stop: None,
            reject_hello_until: 0,
            ice_timers: BTreeSet::new(),
            ice_seq: 0,
        }
    }
}

struct Server {
    /// Accepts and acknowledges frames but delivers no `signal` (a relay that is up and mute).
    drop_signals: bool,
    up: bool,
    conns: BTreeMap<usize, Conn>,
    sweep_gen: u32,
    next_client: u32,
    next_join: u64,
    /// The latest `stop_notice` payload the relay saw and the client id it came from (a hostile
    /// relay records it for `replay_stop`).
    last_stop: Option<(String, Value)>,
    /// Every `hello` before this time is answered with `UNSUPPORTED_PROTOCOL`.
    reject_hello_until: Time,
    /// Rooms with a pending half-TTL renewal (`RelaySpec::ice`).
    ice_timers: BTreeSet<String>,
    ice_seq: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Data,
    Signaling,
}

#[derive(Clone, Copy)]
enum FaultKind {
    Drop,
    Delay,
    Duplicate,
    Reorder,
}

#[derive(Clone)]
struct Fault {
    kind: FaultKind,
    from: Option<usize>,
    via: Via,
    /// Only messages whose text contains this.
    contains: Option<String>,
    start: Time,
    until: Time,
    p: f64,
    ms: u64,
    jitter: u64,
}

pub struct World {
    pub now: Time,
    pub end: Time,
    q: BTreeMap<(Time, u8, u64), Ev>,
    seq: u64,
    pub rng: Rng,
    pub relay: RelaySpec,
    pub actors: Vec<Actor>,
    /// The simulated relay that is selected: `servers` are swapped in and out of this slot by
    /// [`World::select_server`], so the relay code below handles one relay at a time.
    server: Server,
    cur_server: usize,
    /// How many relays the scenario's default mailbox set names.
    relay_count: usize,
    /// The other relays (the selected one's slot is empty), indexed by mailbox id.
    stash: Vec<Option<Server>>,
    /// The relay a socket belongs to.
    sock_server: BTreeMap<usize, usize>,
    fp_seq: u64,
    next_sock: usize,
    socks: BTreeMap<usize, (usize, u32)>,
    faults: Vec<Fault>,
    fifo: BTreeMap<(u8, usize, u32), Time>,
    timeline: Vec<Step>,
    pub violations: Vec<String>,
    /// Frames received on sockets that had not said `hello`, with the actor that sent them.
    pub preauth: Vec<(usize, String)>,
    /// `(actor, time)` of every `create_attempt` the cores asked for (assertion `attempts`).
    pub attempts: Vec<(usize, Time)>,
    /// `(actor, time, ice servers)` of every `create_attempt` (assertion `attempt_turn_valid`).
    pub attempt_ice: Vec<(usize, Time, Vec<IceServer>)>,
    pub recs: Vec<TraceRec>,
    record: bool,
    pub seed: u64,
    /// True wall-clock time (Unix seconds) at virtual time 0.
    pub wall_base: u64,
    /// The log canary, when a run asks for it (see [`World::enable_log_canary`]).
    canary: Option<CanaryRun>,
}

/// State of the log canary of one run: the known values plus the ones the run draws.
struct CanaryRun {
    set: CanarySet,
    /// `(canary, encoding, kind)` already reported, so one leak is one violation.
    reported: BTreeSet<(String, String, String)>,
    /// The parent secret of the first simulated pairing (the core draws it at run time).
    parent_secret: Option<Vec<u8>>,
    lines: u64,
}

impl World {
    pub fn new(sc: &Scenario, seed: u64, record: bool) -> Result<World, String> {
        let mut w = World {
            now: 0,
            end: sc.end_ms(),
            q: BTreeMap::new(),
            seq: 0,
            rng: Rng::new(seed),
            relay: sc.relay.clone(),
            actors: Vec::new(),
            server: Server::new(),
            cur_server: 0,
            relay_count: sc.relay_count.max(1),
            stash: vec![None],
            sock_server: BTreeMap::new(),
            fp_seq: 0,
            next_sock: 0,
            socks: BTreeMap::new(),
            faults: Vec::new(),
            fifo: BTreeMap::new(),
            timeline: sc.timeline.clone(),
            violations: Vec::new(),
            preauth: Vec::new(),
            attempts: Vec::new(),
            attempt_ice: Vec::new(),
            recs: Vec::new(),
            record,
            seed,
            wall_base: sc.wall_start_s.unwrap_or(SIM_WALL_BASE_S),
            canary: None,
        };
        for spec in &sc.peers {
            w.add_link_actor(spec)?;
        }
        for (i, st) in sc.timeline.iter().enumerate() {
            w.push(st.at, 1, Ev::Timeline(i));
        }
        w.push(1_000, 4, Ev::Watch);
        w.push(SWEEP_MS, 3, Ev::Sweep { r#gen: 0, srv: 0 });
        Ok(w)
    }

    fn add_link_actor(&mut self, spec: &PeerSpec) -> Result<(), String> {
        let psk = synthetic_psk(&spec.psk);
        let seed = link_seed(self.seed, &spec.id, 0);
        let mut cfg = LinkConfig::with_defaults(spec.role, encode_psk(&psk), &seed);
        cfg.paired_before = spec.paired_before;
        cfg.mailboxes = if spec.mailboxes.is_empty() {
            // The URLs are labels: mailbox `i` talks to simulated relay `i`.
            (0..self.relay_count)
                .map(|i| InviteMailbox {
                    kind: MailboxKind::Relay,
                    url: format!("wss://relay-{i}.invalid"),
                })
                .collect()
        } else {
            spec.mailboxes.clone()
        };
        cfg.pin_window_until_s = spec.pin_window_s.map(|s| self.wall_base + s);
        cfg.only_local = spec.only_local;
        if let Some(n) = &spec.device_name {
            cfg.device_name = n.clone();
        }
        if let Some(h) = spec.heartbeat_interval_ms {
            cfg.heartbeat_interval_ms = h;
        }
        if let Some(a) = spec.alarm_after_ms {
            cfg.alarm_after_ms = a;
        }
        let link = Link::new(cfg.clone()).map_err(|e| format!("peer {}: {e:?}", spec.id))?;
        let mut a = Actor::new(&spec.id, spec.role, cfg, link.room_id().to_owned());
        a.wall_offset_s = spec.wall_offset_s;
        a.link = Some(link);
        if let Some(d) = &spec.sdp_direction {
            a.sdp_dir = d.clone();
        }
        if let Some(d) = &spec.sdp_talk_direction {
            a.talk_dir = d.clone();
        }
        self.actors.push(a);
        Ok(())
    }

    pub fn actor_ix(&self, name: &str) -> Option<usize> {
        self.actors.iter().position(|a| a.name == name)
    }

    /// The room the first peer derives from its PSK (what a stranger "knows").
    pub fn target_room(&self) -> String {
        self.actors
            .first()
            .map(|a| a.room.clone())
            .unwrap_or_else(|| derive_room_id(&synthetic_psk("A")))
    }

    // ---- event loop ------------------------------------------------------------------------

    fn push(&mut self, t: Time, prio: u8, ev: Ev) {
        self.seq += 1;
        self.q.insert((t.max(self.now), prio, self.seq), ev);
    }

    fn fifo_time(&mut self, kind: u8, a: usize, b: u32, t: Time, bypass: bool) -> Time {
        if bypass {
            return t;
        }
        let e = self.fifo.entry((kind, a, b)).or_insert(0);
        let t = t.max(*e);
        *e = t;
        t
    }

    pub fn run(&mut self) {
        while let Some((&key, _)) = self.q.iter().next() {
            if key.0 > self.end {
                break;
            }
            let ev = self.q.remove(&key).expect("key exists");
            self.now = self.now.max(key.0);
            self.dispatch(ev);
        }
        self.now = self.now.max(self.end);
        for a in 0..self.actors.len() {
            self.finish_rec(a);
        }
        let live: Vec<_> = self
            .actors
            .iter()
            .filter(|a| a.link.is_some())
            .map(|a| (a.name.clone(), a.pcs.values().filter(|p| !p.closed).count()))
            .collect();
        for (name, n) in live {
            if n > 1 {
                self.violations
                    .push(format!("{name} ends with {n} live attempts"));
            }
        }
    }

    fn dispatch(&mut self, ev: Ev) {
        match ev {
            Ev::Timeline(i) => self.run_step(i),
            Ev::Tick { actor, chain } => self.on_tick(actor, chain),
            Ev::Accept { sock } => {
                self.select_sock(sock);
                self.on_accept(sock)
            }
            Ev::SigToServer { sock, text, spoof } => {
                self.select_sock(sock);
                self.on_server_rx(sock, &text, spoof)
            }
            Ev::SigToClient { actor, sock, msg } => self.on_client_rx(actor, sock, msg),
            Ev::Pong { sock } => {
                self.select_sock(sock);
                if let Some(c) = self.server.conns.get(&sock)
                    && self.responsive(c)
                {
                    self.server.conns.get_mut(&sock).expect("exists").alive = true;
                }
            }
            Ev::Sweep { r#gen, srv } => {
                self.select_server(srv);
                self.on_sweep(r#gen)
            }
            Ev::IceRenew { srv, key } => {
                self.select_server(srv);
                self.on_ice_renew(key);
            }
            Ev::LocalDesc {
                actor,
                handle,
                sdp_type,
            } => self.on_local_desc(actor, handle, sdp_type),
            Ev::PcConnect {
                a,
                ha,
                b,
                hb,
                since,
            } => self.on_pc_connect(a, ha, b, hb, since),
            Ev::Data { to, handle, text } => {
                // Messages already in flight arrive even if the sender closed meanwhile.
                let rx_ok = self.actors[to].pcs.get(&handle).is_some_and(|p| {
                    p.connected && !p.closed && !p.failed && self.reach(to, p.epoch)
                });
                if !rx_ok {
                    return;
                }
                if self.actors[to].raw.is_some() {
                    self.raw_on_data(to, handle, &text);
                } else {
                    let text = self.mitm_adjust(to, handle, text);
                    self.actors[to].last_rx_data = self.now;
                    self.deliver(
                        to,
                        LinkInput::AttemptDataMessage {
                            attempt: handle,
                            text,
                        },
                    );
                }
            }
            Ev::DataClosed { to, handle } => {
                let open = self.actors[to]
                    .pcs
                    .get(&handle)
                    .is_some_and(|p| !p.closed && !p.failed);
                if open && self.actors[to].raw.is_none() {
                    self.deliver(to, LinkInput::AttemptDataClosed { attempt: handle });
                }
            }
            Ev::Input { actor, input } => self.deliver(actor, input),
            Ev::ServerClose { sock } => {
                self.select_sock(sock);
                self.server_terminate(sock)
            }
            Ev::PathBack { actor } => {
                self.actors[actor].net_up = true;
                self.flush_after_up(actor, true);
            }
            Ev::Restart { actor } => self.restart_actor(actor),
            Ev::Watch => {
                self.on_watch();
                self.push(self.now + 1_000, 4, Ev::Watch);
            }
            Ev::RawTick { actor } => self.raw_tick(actor),
            Ev::RawAct { actor } => self.raw_act(actor),
        }
    }

    // ---- timeline verbs --------------------------------------------------------------------

    fn peer_ix(&mut self, st: &Step) -> Option<usize> {
        let name = st.peer.clone().unwrap_or_default();
        let ix = self.actor_ix(&name);
        if ix.is_none() {
            self.violations
                .push(format!("step `{}`: unknown peer `{name}`", st.verb));
        }
        ix
    }

    fn link_actors(&self) -> Vec<usize> {
        (0..self.actors.len())
            .filter(|i| self.actors[*i].raw.is_none())
            .collect()
    }

    fn run_step(&mut self, i: usize) {
        let st = self.timeline[i].clone();
        match st.verb.as_str() {
            "end" => {}
            "start" => {
                let targets = match &st.peer {
                    Some(_) => self.peer_ix(&st).into_iter().collect(),
                    None => self.link_actors(),
                };
                for a in targets {
                    self.start_actor(a);
                }
            }
            "stop" => {
                if let Some(a) = self.peer_ix(&st) {
                    self.deliver(a, LinkInput::Stop);
                }
            }
            "acknowledge" => {
                if let Some(a) = self.peer_ix(&st) {
                    self.deliver(a, LinkInput::AcknowledgeAlarm);
                }
            }
            "input" => {
                if let Some(a) = self.peer_ix(&st) {
                    match serde_json::from_value::<LinkInput>(st.input.clone().unwrap_or_default())
                    {
                        Ok(input) => self.deliver(a, input),
                        Err(e) => self.violations.push(format!("bad input: {e}")),
                    }
                }
            }
            // What the baby's microphone hears (`levels`) and the level of the incoming talk audio
            // (`talk_levels`): one sample every 100 ms for `for_ms`, like the platform's ~10 Hz.
            "levels" | "talk_levels" => {
                if let Some(a) = self.peer_ix(&st) {
                    let rms = st.rms.unwrap_or(0.0);
                    for i in 0..st.for_ms.unwrap_or(100) / 100 {
                        let input = if st.verb == "levels" {
                            LinkInput::AudioLevel { rms }
                        } else {
                            LinkInput::TalkLevel { rms }
                        };
                        self.push(self.now + i * 100, 2, Ev::Input { actor: a, input });
                    }
                }
            }
            "network_down" => {
                if let Some(a) = self.peer_ix(&st) {
                    self.actors[a].net_up = false;
                }
            }
            "network_up" => {
                if let Some(a) = self.peer_ix(&st) {
                    self.actors[a].net_up = true;
                    self.flush_after_up(a, st.emit_available.unwrap_or(true));
                }
            }
            "path_switch" => {
                if let Some(a) = self.peer_ix(&st) {
                    let gap = st.gap_ms.unwrap_or(1_500);
                    let ac = &mut self.actors[a];
                    ac.path_epoch += 1;
                    ac.net_up = false;
                    ac.held_up.clear();
                    ac.held_down.clear();
                    self.push(self.now + gap, 3, Ev::PathBack { actor: a });
                }
            }
            "signaling_down" => {
                for srv in self.targeted_servers(&st) {
                    self.select_server(srv);
                    self.signaling_down();
                }
            }
            "signaling_up" => {
                for srv in self.targeted_servers(&st) {
                    self.select_server(srv);
                    self.server.up = true;
                    self.server.sweep_gen += 1;
                    let r#gen = self.server.sweep_gen;
                    self.push(self.now + SWEEP_MS, 3, Ev::Sweep { r#gen, srv });
                }
            }
            // A relay that stays up, answers pings and takes frames, but delivers no `signal`.
            "relay_drop_signals" => {
                let on = st.mode.as_deref() != Some("off");
                for srv in self.targeted_servers(&st) {
                    self.select_server(srv);
                    self.server.drop_signals = on;
                }
            }
            "kill_process" => {
                if let Some(a) = self.peer_ix(&st) {
                    self.kill_actor(a);
                }
            }
            "restart_process" => {
                if let Some(a) = self.peer_ix(&st) {
                    self.kill_actor(a);
                    self.actors[a].pending_mailboxes = st.mailboxes.clone();
                    let after = st.after_ms.unwrap_or(0);
                    self.push(self.now + after, 2, Ev::Restart { actor: a });
                }
            }
            "drop_data" | "delay" | "duplicate" | "reorder" => self.add_fault(&st),
            "clear_faults" => self.faults.clear(),
            "time_jump" => self.time_jump(&st),
            "stranger_join" => self.stranger_join(&st),
            "stranger_leave" => {
                if let Some(a) = self.peer_ix(&st) {
                    self.kill_actor(a);
                }
            }
            "ghost_join" => self.ghost_join(&st),
            "replay_stop" => self.replay_stop(&st),
            // The relay turns hostile (or honest again) at run time: `mode` is an `sdp_rewrite`
            // value, absent for honest.
            "relay_rewrite" => self.relay.sdp_rewrite = st.mode.clone(),
            "reject_hello" => {
                let for_ms = st.for_ms.unwrap_or(u64::MAX - self.now);
                for srv in self.targeted_servers(&st) {
                    self.select_server(srv);
                    self.server.reject_hello_until = self.now.saturating_add(for_ms);
                }
            }
            other => self
                .violations
                .push(format!("unknown timeline verb `{other}`")),
        }
    }

    /// Makes relay `idx` the selected one (creating it on first use).
    fn select_server(&mut self, idx: usize) {
        if idx == self.cur_server {
            return;
        }
        while self.stash.len() <= idx {
            self.stash.push(Some(Server::new()));
        }
        let next = self.stash[idx].take().unwrap_or_else(Server::new);
        let prev = std::mem::replace(&mut self.server, next);
        self.stash[self.cur_server] = Some(prev);
        self.cur_server = idx;
    }

    /// Selects the relay a socket is connected to.
    fn select_sock(&mut self, sock: usize) {
        if let Some(&srv) = self.sock_server.get(&sock) {
            self.select_server(srv);
        }
    }

    /// The relays a timeline step addresses: its `relay` index, otherwise every relay.
    fn targeted_servers(&self, st: &Step) -> Vec<usize> {
        match st.relay {
            Some(i) => vec![i],
            None => (0..self.stash.len().max(self.relay_count)).collect(),
        }
    }

    fn add_fault(&mut self, st: &Step) {
        let from = st
            .from
            .as_deref()
            .and_then(|n| if n == "all" { None } else { self.actor_ix(n) });
        let kind = match st.verb.as_str() {
            "drop_data" => FaultKind::Drop,
            "delay" => FaultKind::Delay,
            "duplicate" => FaultKind::Duplicate,
            _ => FaultKind::Reorder,
        };
        let via = if st.via.as_deref() == Some("signaling") {
            Via::Signaling
        } else {
            Via::Data
        };
        self.faults.push(Fault {
            kind,
            from,
            via,
            contains: st.message.clone(),
            start: self.now,
            until: st.for_ms.map_or(u64::MAX, |d| self.now + d),
            p: st.p.unwrap_or(1.0),
            ms: st.ms.unwrap_or(0),
            jitter: st.jitter_ms.unwrap_or(0),
        });
    }

    fn plan(&mut self, from: usize, via: Via, text: &str) -> Vec<(u64, bool)> {
        let now = self.now;
        let faults: Vec<Fault> = self
            .faults
            .iter()
            .filter(|f| {
                f.via == via
                    && f.start <= now
                    && now < f.until
                    && f.from.is_none_or(|x| x == from)
                    && f.contains.as_deref().is_none_or(|c| text.contains(c))
            })
            .cloned()
            .collect();
        let (mut extra, mut copies, mut bypass) = (0u64, 1usize, false);
        for f in faults {
            match f.kind {
                FaultKind::Drop => {
                    if self.rng.chance(f.p) {
                        return Vec::new();
                    }
                }
                FaultKind::Delay => extra += f.ms + self.rng.range(0, f.jitter),
                FaultKind::Duplicate => {
                    if self.rng.chance(f.p) {
                        copies = 2;
                    }
                }
                FaultKind::Reorder => {
                    if self.rng.chance(f.p) {
                        extra += self.rng.range(0, f.ms);
                        bypass = true;
                    }
                }
            }
        }
        (0..copies)
            .map(|i| {
                (
                    extra + if i > 0 { self.rng.range(1, 30) } else { 0 },
                    bypass,
                )
            })
            .collect()
    }

    fn time_jump(&mut self, st: &Step) {
        let ms = st.ms.unwrap_or(0);
        let old = std::mem::take(&mut self.q);
        for ((t, p, s), ev) in old {
            match ev {
                Ev::Timeline(_) => {
                    self.q.insert((t, p, s), ev);
                }
                Ev::PcConnect {
                    a,
                    ha,
                    b,
                    hb,
                    since,
                } => {
                    let ev = Ev::PcConnect {
                        a,
                        ha,
                        b,
                        hb,
                        since: since + ms,
                    };
                    self.q.insert((t + ms, p, s), ev);
                }
                other => {
                    self.q.insert((t + ms, p, s), other);
                }
            }
        }
        for v in self.fifo.values_mut() {
            *v += ms;
        }
        for a in &mut self.actors {
            for pc in a.pcs.values_mut() {
                if let Some(s) = pc.unhealthy_since.as_mut() {
                    *s += ms;
                }
            }
        }
        self.now += ms;
        if st.peer_traffic.as_deref() == Some("kept") {
            let mut deliveries = Vec::new();
            for (i, a) in self.actors.iter().enumerate() {
                if a.link.is_none() {
                    continue;
                }
                for (h, pc) in &a.pcs {
                    if pc.connected && !pc.closed && !pc.failed {
                        deliveries.push((i, *h));
                    }
                }
            }
            for (i, h) in deliveries {
                let text = json!({"type":"heartbeat","v":3,"t":0,"seq":1_000_000}).to_string();
                self.push(
                    self.now,
                    0,
                    Ev::Data {
                        to: i,
                        handle: h,
                        text,
                    },
                );
            }
        }
    }

    // ---- actors: start, kill, restart, tick ------------------------------------------------

    fn start_actor(&mut self, a: usize) {
        let entropy = self.rng.next_u64() & 0xFFFF_FFFF;
        self.actors[a].chain += 1;
        let chain = self.actors[a].chain;
        self.actors[a].last_wall_ms = None;
        self.deliver(a, LinkInput::Start { entropy });
        self.report_wall_clock(a);
        self.push(self.now + 250, 3, Ev::Tick { actor: a, chain });
    }

    /// Reports the simulated wall clock at start and then every 60 s of virtual time, like a driver
    /// does. Synthetic (no real date) and not an "input" for the tick pacing.
    fn report_wall_clock(&mut self, a: usize) {
        let due = self.actors[a]
            .last_wall_ms
            .is_none_or(|t| self.now.saturating_sub(t) >= WALL_CLOCK_EVERY_MS);
        if due {
            self.actors[a].last_wall_ms = Some(self.now);
            let true_s = i64::try_from(self.wall_base + self.now / 1_000).unwrap_or(i64::MAX);
            let unix_s = u64::try_from(true_s + self.actors[a].wall_offset_s).unwrap_or(0);
            self.deliver(a, LinkInput::WallClock { unix_s });
        }
    }

    fn on_tick(&mut self, a: usize, chain: u32) {
        let ac = &self.actors[a];
        if ac.chain != chain || !ac.alive || ac.link.is_none() {
            return;
        }
        self.deliver(a, LinkInput::Tick);
        // After the tick: the first input of a moment takes the timers that are due.
        self.report_wall_clock(a);
        let ac = &self.actors[a];
        let state = ac.link.as_ref().and_then(|l| l.status()).map(|s| s.state);
        if matches!(state, Some(LinkState::Stopped | LinkState::FailedNeedsUser)) {
            return;
        }
        // Tick often while something happened recently, otherwise at the 500 ms contract rate.
        let period = if self.now.saturating_sub(ac.last_input_at) < 2_000 {
            250
        } else {
            500
        };
        self.push(self.now + period, 3, Ev::Tick { actor: a, chain });
    }

    fn finish_rec(&mut self, a: usize) {
        if let Some(r) = self.actors[a].rec.take() {
            self.recs[r].final_state = self.actors[a]
                .link
                .as_ref()
                .and_then(|l| l.status())
                .map(|s| s.state);
        }
    }

    /// Process death: sockets and peer connections vanish, nobody is told.
    fn kill_actor(&mut self, a: usize) {
        self.finish_rec(a);
        let reachable = {
            let ac = &self.actors[a];
            ac.net_up
        };
        let socks = std::mem::take(&mut self.actors[a].socks);
        for s in socks.into_values() {
            if reachable {
                let t = self.fifo_time(1, s, 0, self.now + 20, false);
                self.push(t, 2, Ev::ServerClose { sock: s });
            }
        }
        let ac = &mut self.actors[a];
        ac.alive = false;
        ac.link = None;
        ac.chain += 1;
        ac.pcs.clear();
        ac.held_up.clear();
        ac.held_down.clear();
    }

    fn restart_actor(&mut self, a: usize) {
        if self.actors[a].raw.is_some() {
            return;
        }
        let mut cfg = self.actors[a].cfg.clone();
        cfg.paired_before = true;
        // A restart is a new run: a new seed, as the platforms draw one per run.
        cfg.seed = link_seed(self.seed, &self.actors[a].name, self.actors[a].instance + 1);
        if let Some(c) = self.canary.as_mut() {
            c.set.add_seed(
                &format!(
                    "seed_{}_run{}",
                    self.actors[a].name,
                    self.actors[a].instance + 1
                ),
                &cfg.seed,
            );
        }
        if let Some(m) = self.actors[a].pending_mailboxes.take() {
            cfg.mailboxes = m;
        }
        let Ok(link) = Link::new(cfg.clone()) else {
            return;
        };
        let ac = &mut self.actors[a];
        ac.cfg = cfg;
        ac.link = Some(link);
        ac.alive = true;
        ac.instance += 1;
        ac.rec = None;
        self.start_actor(a);
    }

    // ---- log canary ------------------------------------------------------------------------

    /// Turns the log canary on (AGENTS.md section 7.6): from now on everything a platform could
    /// log is scanned for the PSKs of the run (known), the seeds, the keys derived from them, the
    /// room ids, plaintext SDP and, once the first pairing has drawn it, the parent secret.
    ///
    /// "Log" here is the `{:?}` text of every input the core receives, of every action and event
    /// it returns and of every config: the worst a careless driver could print. Wire payloads (a
    /// mailbox frame, an SDP in transit) may carry the room id and SDP by design, so for those
    /// lines only the secrets are checked. Call before [`World::run`].
    pub fn enable_log_canary(&mut self) {
        let mut set = CanarySet::new();
        set.add_pairing("", &synthetic_psk("A"));
        set.add_pairing("_other", &synthetic_psk("B"));
        for ac in &self.actors {
            // The two synthetic PSKs are in already; a scenario may name another one.
            if let Some(psk) = decode_psk(&ac.cfg.psk_b64url)
                && psk != synthetic_psk("A")
                && psk != synthetic_psk("B")
            {
                set.add_pairing(&format!("_{}", ac.name), &psk);
            }
            set.add_seed(&format!("seed_{}_run0", ac.name), &ac.cfg.seed);
        }
        set.add_sdp_markers();
        self.canary = Some(CanaryRun {
            set,
            reported: BTreeSet::new(),
            parent_secret: None,
            lines: 0,
        });
        let configs: Vec<(usize, String)> = self
            .actors
            .iter()
            .enumerate()
            .map(|(i, ac)| (i, format!("{:?}", ac.cfg)))
            .collect();
        for (i, text) in configs {
            self.log_line(i, "config", &text, false);
        }
    }

    /// The canary list of the run (names only matter to callers and tests).
    pub fn canary_set(&self) -> Option<&CanarySet> {
        self.canary.as_ref().map(|c| &c.set)
    }

    /// The parent secret of the first pairing, once the run has drawn it.
    pub fn canary_parent_secret(&self) -> Option<&[u8]> {
        self.canary
            .as_ref()
            .and_then(|c| c.parent_secret.as_deref())
    }

    /// How many log lines the canary has scanned.
    pub fn canary_lines(&self) -> u64 {
        self.canary.as_ref().map_or(0, |c| c.lines)
    }

    /// One log line of actor `a`; a leak becomes a violation (the canary name and encoding, never
    /// the value). Public so a test can plant a leak through the very same path.
    pub fn log_line(&mut self, a: usize, kind: &str, text: &str, wire: bool) {
        let name = self
            .actors
            .get(a)
            .map_or("?", |ac| ac.name.as_str())
            .to_owned();
        let now = self.now;
        let Some(c) = self.canary.as_mut() else {
            return;
        };
        c.lines += 1;
        let place = format!("sim log of {name} at t={now} ({kind})");
        let hits = c.set.scan(&place, text.as_bytes(), wire);
        for h in hits {
            let key = (h.canary.clone(), h.encoding.clone(), kind.to_owned());
            if c.reported.insert(key) {
                self.violations.push(format!(
                    "log canary: {} ({}) in {place}",
                    h.canary, h.encoding
                ));
            }
        }
    }

    fn log_actions(&mut self, a: usize, actions: &[LinkAction]) {
        // Learn the run-time values first: the parent secret comes out in `ParentPinned`, before
        // the action that sends it, in the very same list.
        if let Some(c) = self.canary.as_mut() {
            for act in actions {
                match act {
                    LinkAction::Emit {
                        event: LinkEvent::ParentPinned { secret, .. },
                    } if c.parent_secret.is_none() => {
                        c.parent_secret = Some(secret.bytes.clone());
                        c.set.add_parent_secret(&secret.bytes);
                    }
                    LinkAction::SendData { text, .. } if carries_parent_pin(text) => {
                        c.set.add_pin_message(text);
                    }
                    LinkAction::CreateAttempt { ice_servers, .. } => {
                        for cred in ice_servers.iter().filter_map(|s| s.credential.as_deref()) {
                            c.set.add_turn_password(cred);
                        }
                    }
                    _ => {}
                }
            }
        }
        for act in actions {
            let wire = matches!(
                act,
                LinkAction::MailboxSend { .. }
                    | LinkAction::SendData { .. }
                    | LinkAction::SetRemoteDescription { .. }
                    | LinkAction::AddIceCandidate { .. }
            );
            self.log_line(a, "action", &format!("{act:?}"), wire);
        }
    }

    // ---- driving a Link --------------------------------------------------------------------

    fn deliver(&mut self, a: usize, input: LinkInput) {
        if !self.actors[a].alive {
            return;
        }
        let now = self.now;
        // The wall clock, like a tick, says nothing about activity.
        let is_tick = matches!(input, LinkInput::Tick | LinkInput::WallClock { .. });
        if !is_tick {
            self.actors[a].last_input_at = now;
        }
        if self.record && self.actors[a].rec.is_none() && self.actors[a].link.is_some() {
            let ac = &self.actors[a];
            self.recs
                .push(TraceRec::new(&ac.name, ac.instance, ac.cfg.clone()));
            let ix = self.recs.len() - 1;
            self.actors[a].rec = Some(ix);
        }
        let recorded = self.actors[a].rec.map(|_| input.clone());
        if self.canary.is_some() && !is_tick {
            let wire = matches!(
                input,
                LinkInput::MailboxFrame { .. }
                    | LinkInput::AttemptLocalDescription { .. }
                    | LinkInput::AttemptLocalIce { .. }
                    | LinkInput::AttemptDataMessage { .. }
            );
            self.log_line(a, "input", &format!("{input:?}"), wire);
        }
        let Some(link) = self.actors[a].link.as_mut() else {
            return;
        };
        let actions = link.handle(now, input);
        if self.canary.is_some() {
            self.log_actions(a, &actions);
        }
        if let (Some(r), Some(inp)) = (self.actors[a].rec, recorded) {
            self.recs[r].push(now, inp, &actions);
        }
        self.exec(a, actions);
    }

    fn exec(&mut self, a: usize, actions: Vec<LinkAction>) {
        for act in actions {
            match act {
                LinkAction::OpenMailbox { id, .. } => self.open_mailbox(a, id),
                LinkAction::CloseMailbox { id } => {
                    if let Some(s) = self.actors[a].socks.remove(&id) {
                        let epoch = self.socks[&s].1;
                        if self.reach(a, epoch) {
                            let t = self.fifo_time(1, s, 0, self.now + 20, false);
                            self.push(t, 2, Ev::ServerClose { sock: s });
                        }
                    }
                }
                LinkAction::MailboxSend { id, frame } => self.client_send_on(a, id, frame, None),
                LinkAction::CreateAttempt {
                    attempt,
                    ice_servers,
                    ..
                } => {
                    self.attempts.push((a, self.now));
                    self.attempt_ice.push((a, self.now, ice_servers));
                    if self.actors[a].pcs.values().any(|p| !p.closed) {
                        self.violations.push(format!(
                            "{}: create_attempt {attempt} while another attempt is live",
                            self.actors[a].name
                        ));
                    }
                    self.new_pc(a, attempt);
                }
                LinkAction::CloseAttempt { attempt } => self.close_attempt(a, attempt),
                LinkAction::CreateOffer { attempt } => {
                    let d = self.rng.range(5, 30);
                    self.push(
                        self.now + d,
                        2,
                        Ev::LocalDesc {
                            actor: a,
                            handle: attempt,
                            sdp_type: "offer",
                        },
                    );
                }
                LinkAction::CreateAnswer { attempt } => {
                    let d = self.rng.range(5, 30);
                    self.push(
                        self.now + d,
                        2,
                        Ev::LocalDesc {
                            actor: a,
                            handle: attempt,
                            sdp_type: "answer",
                        },
                    );
                }
                LinkAction::SetRemoteDescription { attempt, sdp, .. } => {
                    self.set_remote(a, attempt, &sdp);
                }
                LinkAction::SendData { attempt, text } => self.send_data(a, attempt, text),
                // The fake platform of talk-back: a healthy one. The chime takes 600 ms, the
                // microphone attaches 30 ms after it was asked to; what the baby's microphone
                // hears (`levels`, `talk_levels` verbs) is up to the scenario.
                LinkAction::PlayTalkChime { attempt, .. } => self.push(
                    self.now + 600,
                    2,
                    Ev::Input {
                        actor: a,
                        input: LinkInput::TalkChimeDone { attempt },
                    },
                ),
                LinkAction::SetTalkMic { enabled: true, .. } => self.push(
                    self.now + 30,
                    2,
                    Ev::Input {
                        actor: a,
                        input: LinkInput::TalkMicAttached,
                    },
                ),
                LinkAction::SetTalkMic { .. }
                | LinkAction::SetPlaybackDucked { .. }
                | LinkAction::SetTalkPlayback { .. }
                | LinkAction::AddIceCandidate { .. }
                | LinkAction::SetMicEnabled { .. }
                | LinkAction::ApplyQuiet
                | LinkAction::SendProbeTone { .. } => {}
                LinkAction::Emit { event } => self.record_event(a, event),
            }
        }
    }

    fn record_event(&mut self, a: usize, event: LinkEvent) {
        let now = self.now;
        let ac = &mut self.actors[a];
        if let LinkEvent::Status { status } = &event {
            ac.statuses.push((now, status.clone()));
        }
        // Like a platform: persist the pinned parent secret, so that a restarted process
        // (`restart_process`) is configured with it.
        if let LinkEvent::ParentPinned {
            secret,
            window_until_s,
        } = &event
        {
            ac.cfg.pinned_parent = Some(secret.clone());
            ac.cfg.pin_window_until_s = *window_until_s;
        }
        let since = if ac.last_rx_data == 0 {
            now
        } else {
            now.saturating_sub(ac.last_rx_data)
        };
        ac.events.push(EvRec {
            t: now,
            ev: serde_json::to_value(&event).unwrap_or(Value::Null),
            since_contact: since,
        });
    }

    // ---- reachability ----------------------------------------------------------------------

    fn reach(&self, a: usize, epoch: u32) -> bool {
        let ac = &self.actors[a];
        ac.alive && ac.net_up && ac.path_epoch == epoch
    }

    /// The path would work again once the network returns.
    fn viable(&self, a: usize, epoch: u32) -> bool {
        let ac = &self.actors[a];
        ac.alive && ac.path_epoch == epoch
    }

    fn responsive(&self, c: &Conn) -> bool {
        c.actor.is_some_and(|a| self.reach(a, c.epoch))
    }

    fn flush_after_up(&mut self, a: usize, emit_available: bool) {
        let ups = std::mem::take(&mut self.actors[a].held_up);
        for (sock, text) in ups {
            let Some(&(_, epoch)) = self.socks.get(&sock) else {
                continue;
            };
            if self.viable(a, epoch) {
                let lat = self.rng.range(5, 40);
                let t = self.fifo_time(1, sock, 0, self.now + lat, false);
                self.push(
                    t,
                    2,
                    Ev::SigToServer {
                        sock,
                        text,
                        spoof: None,
                    },
                );
            }
        }
        let downs = std::mem::take(&mut self.actors[a].held_down);
        for (sock, msg) in downs {
            self.select_sock(sock);
            if self.server.conns.contains_key(&sock) {
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
            }
        }
        if emit_available && self.actors[a].link.is_some() {
            let d = self.rng.range(50, 400);
            self.push(
                self.now + d,
                2,
                Ev::Input {
                    actor: a,
                    input: LinkInput::NetworkAvailable,
                },
            );
        }
    }

    // ---- signaling: client side ------------------------------------------------------------

    pub fn open_signaling(&mut self, a: usize) {
        self.open_mailbox(a, 0);
    }

    /// Opens mailbox `id` of actor `a`: a socket to simulated relay `id`.
    pub fn open_mailbox(&mut self, a: usize, id: MailboxId) {
        let sock = self.next_sock;
        self.next_sock += 1;
        let epoch = self.actors[a].path_epoch;
        self.socks.insert(sock, (a, epoch));
        self.sock_server.insert(sock, id as usize);
        self.actors[a].socks.insert(id, sock);
        self.select_server(id as usize);
        let (up, reach) = (self.server.up, self.reach(a, epoch));
        if !reach {
            let d = 3_000 + self.rng.range(0, 500);
            self.push(
                self.now + d,
                2,
                Ev::SigToClient {
                    actor: a,
                    sock,
                    msg: ToClient::Closed,
                },
            );
        } else if !up {
            let d = self.rng.range(20, 80);
            self.push(
                self.now + d,
                2,
                Ev::SigToClient {
                    actor: a,
                    sock,
                    msg: ToClient::Closed,
                },
            );
        } else {
            let d = self.rng.range(5, 40);
            self.push(self.now + d, 2, Ev::Accept { sock });
        }
    }

    pub fn client_send(&mut self, a: usize, text: String, spoof: Option<String>) {
        self.client_send_on(a, 0, text, spoof);
    }

    pub fn client_send_on(&mut self, a: usize, id: MailboxId, text: String, spoof: Option<String>) {
        let Some(sock) = self.actors[a].socks.get(&id).copied() else {
            return;
        };
        if let Some(t) = self.signal_type_for(a, &text) {
            let now = self.now;
            self.actors[a].sent_signals.push((now, t));
        }
        let epoch = self.socks[&sock].1;
        for (extra, bypass) in self.plan(a, Via::Signaling, &text) {
            if self.reach(a, epoch) {
                let lat = self.rng.range(5, 40) + extra;
                let t = self.fifo_time(1, sock, 0, self.now + lat, bypass);
                self.push(
                    t,
                    2,
                    Ev::SigToServer {
                        sock,
                        text: text.clone(),
                        spoof: spoof.clone(),
                    },
                );
            } else if self.viable(a, epoch) {
                self.actors[a].held_up.push((sock, text.clone()));
            }
        }
    }

    fn on_client_rx(&mut self, a: usize, sock: usize, msg: ToClient) {
        let id = self.sock_server.get(&sock).copied().unwrap_or(0) as MailboxId;
        if self.actors[a].socks.get(&id) != Some(&sock) || !self.actors[a].alive {
            return;
        }
        let epoch = self.socks[&sock].1;
        if !matches!(msg, ToClient::Closed) && !self.reach(a, epoch) {
            if self.viable(a, epoch) {
                self.actors[a].held_down.push((sock, msg));
            }
            return;
        }
        if let ToClient::Text(t) = &msg
            && serde_json::from_str::<Value>(t)
                .ok()
                .and_then(|v| v["type"].as_str().map(|s| s == "welcome"))
                == Some(true)
        {
            let now = self.now;
            self.actors[a].welcomes.push(now);
        }
        if matches!(msg, ToClient::Closed) {
            self.actors[a].socks.remove(&id);
        }
        if self.actors[a].raw.is_some() {
            self.raw_on_signaling(a, msg);
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

    // ---- signaling: server side (mirrors crates/guardedsleep-relay) --------------

    fn signaling_down(&mut self) {
        self.server.up = false;
        self.server.sweep_gen += 1;
        let socks: Vec<usize> = self.server.conns.keys().copied().collect();
        for s in socks {
            self.server_terminate_silent(s);
        }
    }

    /// Server restart: connections vanish without member broadcasts (there is no server left).
    fn server_terminate_silent(&mut self, sock: usize) {
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

    fn on_accept(&mut self, sock: usize) {
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

    fn server_send(&mut self, sock: usize, v: Value) {
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

    fn server_error(&mut self, sock: usize, code: &str) {
        self.server_send(
            sock,
            json!({"type":"error","code":code,"message":"simulated"}),
        );
    }

    fn room_members(&self, key: &str) -> Vec<(usize, &Conn)> {
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

    fn broadcast_members(&mut self, key: &str) {
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
    fn server_terminate(&mut self, sock: usize) {
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

    fn on_server_rx(&mut self, sock: usize, text: &str, spoof: Option<String>) {
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

    fn server_hello(&mut self, sock: usize, msg: &Value) {
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
    fn issue_ice_servers(&mut self, key: &str) {
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

    fn on_ice_renew(&mut self, key: String) {
        self.server.ice_timers.remove(&key);
        if self.server.up {
            self.issue_ice_servers(&key);
        }
    }

    fn close_after_error(&mut self, sock: usize) {
        let t = self.fifo_time(0, sock, 0, self.now + 20, false);
        self.push(t, 2, Ev::ServerClose { sock });
    }

    fn server_signal(&mut self, sock: usize, msg: &Value, spoof: Option<String>) {
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

    fn on_sweep(&mut self, r#gen: u32) {
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
    fn mitm_adjust(&self, to: usize, handle: u32, text: String) -> String {
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

    /// A hostile relay re-delivers the last recorded `stop_notice` to `peer`, as the original
    /// sender, on whatever connection `peer` has now.
    fn replay_stop(&mut self, st: &Step) {
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
            .find(|(_, c)| c.actor == Some(a) && c.joined)
            .map(|(s, _)| *s);
        if let Some(sock) = sock {
            self.server_send(sock, json!({"type":"signal","from":from,"payload":payload}));
        }
    }

    /// A socket whose client died silently: holds its room slot until the liveness sweep.
    fn ghost_join(&mut self, st: &Step) {
        let role = st.role.unwrap_or(Role::Parent);
        let requested = self.target_room();
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
        if (role == Role::Baby && babies > 0)
            || (role == Role::Parent && parents >= self.relay.parent_cap)
        {
            return;
        }
        self.select_server(st.relay.unwrap_or(0));
        let sock = self.next_sock;
        self.next_sock += 1;
        self.sock_server.insert(sock, self.cur_server);
        self.server.next_client += 1;
        self.server.next_join += 1;
        let client_id = format!("c{}", self.server.next_client);
        let join_seq = self.server.next_join;
        self.server.conns.insert(
            sock,
            Conn {
                actor: None,
                epoch: 0,
                role,
                client_id,
                requested,
                key: key.clone(),
                joined: true,
                alive: true,
                join_seq,
            },
        );
        self.broadcast_members(&key);
    }

    // ---- media: peer connections and the DataChannel --------------------------------------

    pub fn new_pc(&mut self, a: usize, handle: u32) {
        self.fp_seq += 1;
        let fp = synthetic_fp(self.fp_seq);
        let fail_after = self.rng.range(FAIL_AFTER_MIN, FAIL_AFTER_MAX);
        let epoch = self.actors[a].path_epoch;
        self.actors[a].pcs.insert(
            handle,
            Pc {
                fp,
                dtls_fp: None,
                local: false,
                remote: false,
                peer: None,
                closed: false,
                connected: false,
                failed: false,
                connecting: false,
                epoch,
                unhealthy_since: None,
                fail_after,
            },
        );
    }

    pub fn sdp_for(&self, a: usize, fp: &str) -> String {
        let ac = &self.actors[a];
        let (monitor, talk) = (ac.sdp_dir.as_str(), ac.talk_dir.as_str());
        // The fingerprint line stays last: the core's extractor reads hex until the first
        // non-hex character.
        format!(
            "v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na={monitor}\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na={talk}\r\na=fingerprint:sha-256 {}\r\n",
            colon_upper(fp)
        )
    }

    fn on_local_desc(&mut self, a: usize, handle: u32, sdp_type: &'static str) {
        let Some(pc) = self.actors[a].pcs.get_mut(&handle) else {
            return;
        };
        if pc.closed {
            return;
        }
        pc.local = true;
        let fp = pc.fp.clone();
        let sdp = self.sdp_for(a, &fp);
        self.deliver(
            a,
            LinkInput::AttemptLocalDescription {
                attempt: handle,
                sdp_type: sdp_type.into(),
                sdp,
            },
        );
        self.maybe_connect(a, handle);
    }

    pub fn set_remote(&mut self, a: usize, handle: u32, sdp: &str) {
        // Routing follows the lenient v2 reading: a relay that hides the real fingerprint in `s=`
        // still forwards the media path to the real peer, while DTLS itself is authenticated
        // against the real line (`dtls_fp`).
        let fp = extract_sha256_fingerprint_v2_legacy(sdp).unwrap_or_default();
        let mut peer = None;
        for (m, other) in self.actors.iter().enumerate() {
            if m == a {
                continue;
            }
            for (h, pc) in &other.pcs {
                if pc.fp == fp && !pc.closed {
                    peer = Some((m, *h));
                }
            }
        }
        if let Some(pc) = self.actors[a].pcs.get_mut(&handle) {
            pc.remote = true;
            pc.peer = peer;
            pc.dtls_fp = extract_sha256_fingerprint(sdp);
        }
        self.maybe_connect(a, handle);
    }

    pub fn maybe_connect(&mut self, a: usize, h: u32) {
        let ready =
            |p: &Pc| p.local && p.remote && !p.closed && !p.failed && !p.connecting && !p.connected;
        let Some(pc) = self.actors[a].pcs.get(&h) else {
            return;
        };
        if !ready(pc) {
            return;
        }
        let Some((b, hb)) = pc.peer else {
            return;
        };
        let Some(other) = self.actors[b].pcs.get(&hb) else {
            return;
        };
        if !ready(other) || other.peer != Some((a, h)) {
            return;
        }
        self.actors[a].pcs.get_mut(&h).expect("pc").connecting = true;
        self.actors[b].pcs.get_mut(&hb).expect("pc").connecting = true;
        let d = self.rng.range(250, 1_200);
        let since = self.now;
        self.push(
            self.now + d,
            2,
            Ev::PcConnect {
                a,
                ha: h,
                b,
                hb,
                since,
            },
        );
    }

    fn pair_reachable(&self, a: usize, h: u32, pa: &Pc) -> bool {
        let Some((b, hb)) = pa.peer else {
            return false;
        };
        let Some(pb) = self.actors[b].pcs.get(&hb) else {
            return false;
        };
        !pb.closed
            && !pb.failed
            && self.reach(a, pa.epoch)
            && self.reach(b, pb.epoch)
            && pb.peer == Some((a, h))
    }

    /// The pc exists, is open and both ends can reach each other right now.
    pub fn pc_ok(&self, a: usize, h: u32) -> bool {
        let Some(pc) = self.actors[a].pcs.get(&h) else {
            return false;
        };
        pc.connected && !pc.closed && !pc.failed && self.pair_reachable(a, h, pc)
    }

    fn on_pc_connect(&mut self, a: usize, ha: u32, b: usize, hb: u32, since: Time) {
        let alive = |w: &World, x: usize, h: u32| {
            w.actors[x]
                .pcs
                .get(&h)
                .is_some_and(|p| !p.closed && !p.failed)
        };
        if !alive(self, a, ha) || !alive(self, b, hb) {
            return;
        }
        let reachable = {
            let pa = &self.actors[a].pcs[&ha];
            let pb = &self.actors[b].pcs[&hb];
            self.reach(a, pa.epoch) && self.reach(b, pb.epoch)
        };
        if reachable {
            for (x, hx) in [(a, ha), (b, hb)] {
                self.actors[x].pcs.get_mut(&hx).expect("pc").connected = true;
            }
            for (x, hx) in [(a, ha), (b, hb)] {
                if self.actors[x].raw.is_some() {
                    self.raw_on_open(x, hx);
                } else {
                    self.deliver(
                        x,
                        LinkInput::AttemptPcState {
                            attempt: hx,
                            state: "connected".into(),
                        },
                    );
                    self.deliver(x, LinkInput::AttemptDataOpen { attempt: hx });
                    if let Some(fingerprint) = self.actors[x].pcs[&hx].dtls_fp.clone() {
                        self.deliver(
                            x,
                            LinkInput::AttemptDtlsFingerprint {
                                attempt: hx,
                                fingerprint,
                            },
                        );
                    }
                }
            }
        } else if self.now.saturating_sub(since) < CONNECT_GIVE_UP_MS {
            self.push(
                self.now + 500,
                2,
                Ev::PcConnect {
                    a,
                    ha,
                    b,
                    hb,
                    since,
                },
            );
        } else {
            for (x, hx) in [(a, ha), (b, hb)] {
                self.fail_pc(x, hx);
            }
        }
    }

    fn fail_pc(&mut self, a: usize, h: u32) {
        let Some(pc) = self.actors[a].pcs.get_mut(&h) else {
            return;
        };
        if pc.failed || pc.closed {
            return;
        }
        pc.failed = true;
        if self.actors[a].raw.is_none() {
            self.deliver(
                a,
                LinkInput::AttemptPcState {
                    attempt: h,
                    state: "failed".into(),
                },
            );
        }
    }

    /// ICE consent: a connected pair that stays unreachable for ~30 s becomes `failed`.
    fn on_watch(&mut self) {
        let now = self.now;
        let mut to_fail = Vec::new();
        for a in 0..self.actors.len() {
            let handles: Vec<u32> = self.actors[a]
                .pcs
                .iter()
                .filter(|(_, p)| p.connected && !p.closed && !p.failed)
                .map(|(h, _)| *h)
                .collect();
            for h in handles {
                let ok = self.pc_ok(a, h);
                let pc = self.actors[a].pcs.get_mut(&h).expect("pc");
                if ok {
                    pc.unhealthy_since = None;
                } else {
                    let since = *pc.unhealthy_since.get_or_insert(now);
                    if now.saturating_sub(since) >= pc.fail_after {
                        to_fail.push((a, h));
                    }
                }
            }
        }
        for (a, h) in to_fail {
            self.fail_pc(a, h);
        }
    }

    fn close_attempt(&mut self, a: usize, h: u32) {
        let Some(pc) = self.actors[a].pcs.get(&h) else {
            return;
        };
        if pc.closed {
            return;
        }
        let notify = pc.connected && self.pair_reachable(a, h, pc);
        let peer = pc.peer;
        self.actors[a].pcs.get_mut(&h).expect("pc").closed = true;
        if let (true, Some((b, hb))) = (notify, peer) {
            let lat = self.rng.range(10, 60);
            let t = self.fifo_time(2, b, hb, self.now + lat, false);
            self.push(t, 2, Ev::DataClosed { to: b, handle: hb });
        }
    }

    pub fn send_data(&mut self, a: usize, h: u32, text: String) {
        if !self.pc_ok(a, h) {
            return;
        }
        let Some((b, hb)) = self.actors[a].pcs[&h].peer else {
            return;
        };
        for (extra, bypass) in self.plan(a, Via::Data, &text) {
            let lat = self.rng.range(10, 60) + extra;
            let t = self.fifo_time(2, b, hb, self.now + lat, bypass);
            self.push(
                t,
                2,
                Ev::Data {
                    to: b,
                    handle: hb,
                    text: text.clone(),
                },
            );
        }
    }

    // ---- hooks used by raw.rs --------------------------------------------------------------

    pub(crate) fn schedule_raw_tick(&mut self, actor: usize, delay: u64) {
        self.push(self.now + delay, 3, Ev::RawTick { actor });
    }

    pub(crate) fn schedule_raw_act(&mut self, actor: usize, delay: u64) {
        self.push(self.now + delay, 3, Ev::RawAct { actor });
    }

    pub(crate) fn fresh_fp(&mut self) -> String {
        self.fp_seq += 1;
        synthetic_fp(self.fp_seq)
    }

    pub(crate) fn add_raw_actor(&mut self, name: &str, role: Role, raw: RawState) -> usize {
        let seed = link_seed(self.seed, name, 0);
        let cfg = LinkConfig::with_defaults(role, encode_psk(&synthetic_psk("A")), &seed);
        let room = raw.room.clone();
        let mut a = Actor::new(name, role, cfg, room);
        a.raw = Some(raw);
        self.actors.push(a);
        self.actors.len() - 1
    }

    fn actor_psk(&self, a: usize) -> Option<Vec<u8>> {
        decode_psk(&self.actors[a].cfg.psk_b64url)
    }

    /// What a frame `a` sent is, for the "sent_signal" assertions: the frame type, or for a relay
    /// `signal` the type of the message inside (read with the sender's PSK when it is sealed).
    fn signal_type_for(&self, a: usize, text: &str) -> Option<String> {
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

    pub(crate) fn pc_close(&mut self, a: usize, h: u32) {
        self.close_attempt(a, h);
    }
}

/// Test seed of one `Link` run, derived from the scenario seed, the actor and the run number.
/// It uses its own generator, so the world's random stream (latencies, faults) is untouched.
/// Simulation traces record it; real seeds are never recorded.
fn link_seed(world_seed: u64, actor: &str, run: u32) -> Vec<u8> {
    let name_hash = actor.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    let mut rng = Rng::new(world_seed ^ name_hash ^ (u64::from(run) << 48));
    let mut seed = SYNTH_SEED_PREFIX.to_vec();
    seed.extend((0..2).flat_map(|_| rng.next_u64().to_le_bytes()));
    seed
}

fn valid_v3_room(room: &str) -> bool {
    room.len() == 22
        && room
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
