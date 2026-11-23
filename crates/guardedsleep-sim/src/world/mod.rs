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

mod actors;
mod canary;
mod peer;
mod server;
mod steps;

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
