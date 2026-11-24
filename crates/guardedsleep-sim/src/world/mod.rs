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
mod client;
mod driver;
mod events;
mod faults;
mod hooks;
mod hostile;
mod ids;
mod media;
mod model;
mod peer;
mod room;
mod server;
mod steps;
mod synth;

use self::events::{ActorEv, Ev, PeerEv, RelayEv};
use self::ids::{ActorIx, PcHandle, PcRef, SockIx};
pub use self::model::{Actor, EvRec, Pc, ToClient, Via};
use self::model::{Conn, Delivery, Fault, FaultKind, Flow, Frame, Seat, Server, SockOwner};
pub use self::synth::{
    SYNTH_FP_PREFIX_HEX, SYNTH_PSK_PREFIX, SYNTH_SEED_PREFIX, colon_upper, synthetic_psk,
};
use self::synth::{confuse_fingerprint, link_seed, synthetic_fp, valid_v3_room};

pub type Time = u64;

/// Server liveness sweep period (`SIGNALING_PING_INTERVAL`, ADR-017 section 10).
const SWEEP_MS: u64 = 10_000;
/// Packets of a connection that stays broken this long make the peer connection `failed`
/// (ICE consent freshness is about 30 s).
const FAIL_AFTER_MIN: u64 = 25_000;
const FAIL_AFTER_MAX: u64 = 35_000;
const CONNECT_GIVE_UP_MS: u64 = 30_000;
/// The simulated wall clock starts at this Unix second (a synthetic value, not a date).
const SIM_WALL_BASE_S: u64 = 1_000_000;
/// Drivers report the wall clock this often (and after a time change).
const WALL_CLOCK_EVERY_MS: u64 = 60_000;

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
    sock_server: BTreeMap<SockIx, usize>,
    fp_seq: u64,
    next_sock: usize,
    socks: BTreeMap<SockIx, SockOwner>,
    faults: Vec<Fault>,
    fifo: BTreeMap<Flow, Time>,
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
    canary: Option<canary::CanaryRun>,
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
        w.push(1_000, 4, PeerEv::Watch);
        w.push(SWEEP_MS, 3, RelayEv::Sweep { r#gen: 0, srv: 0 });
        Ok(w)
    }

    /// The `Link` configuration a scenario peer asks for.
    fn link_config(&self, spec: &PeerSpec) -> LinkConfig {
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
        cfg
    }

    fn add_link_actor(&mut self, spec: &PeerSpec) -> Result<(), String> {
        let cfg = self.link_config(spec);
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

    fn push(&mut self, t: Time, prio: u8, ev: impl Into<Ev>) {
        self.seq += 1;
        self.q.insert((t.max(self.now), prio, self.seq), ev.into());
    }

    /// Queues `ev` `delay` ms from now behind earlier traffic of `flow`.
    fn push_flow(&mut self, flow: Flow, delay: u64, ev: impl Into<Ev>) {
        let t = self.fifo_time(flow, self.now + delay, false);
        self.push(t, 2, ev);
    }

    /// `t`, pushed back as far as `flow` needs to stay in order, unless `bypass` lets it overtake.
    fn fifo_time(&mut self, flow: Flow, t: Time, bypass: bool) -> Time {
        if bypass {
            return t;
        }
        let e = self.fifo.entry(flow).or_insert(0);
        let t = t.max(*e);
        *e = t;
        t
    }

    /// The next event that is due before the end of the run.
    fn pop_due(&mut self) -> Option<((Time, u8, u64), Ev)> {
        let entry = self.q.first_entry()?;
        if entry.key().0 > self.end {
            return None;
        }
        Some(entry.remove_entry())
    }

    pub fn run(&mut self) {
        while let Some((key, ev)) = self.pop_due() {
            self.now = self.now.max(key.0);
            self.dispatch(ev);
        }
        self.now = self.now.max(self.end);
        for a in 0..self.actors.len() {
            self.finish_rec(ActorIx::from(a));
        }
        self.flag_extra_attempts();
    }

    /// A violation for every running actor that ends with more than one live attempt.
    fn flag_extra_attempts(&mut self) {
        let overfull: Vec<String> = self
            .actors
            .iter()
            .filter(|a| a.link.is_some())
            .map(|a| (&a.name, a.pcs.values().filter(|p| !p.closed).count()))
            .filter(|(_, n)| *n > 1)
            .map(|(name, n)| format!("{name} ends with {n} live attempts"))
            .collect();
        self.violations.extend(overfull);
    }

    fn dispatch(&mut self, ev: Ev) {
        match ev {
            Ev::Timeline(i) => self.run_step(i),
            Ev::Actor(e) => self.dispatch_actor(e),
            Ev::Relay(e) => self.dispatch_relay(e),
            Ev::Peer(e) => self.dispatch_peer(e),
        }
    }

    // ---- lookups ---------------------------------------------------------------------------

    fn peer_ix(&mut self, st: &Step) -> Option<ActorIx> {
        let name = st.peer.clone().unwrap_or_default();
        let ix = self.actor_ix(&name);
        if ix.is_none() {
            self.violations
                .push(format!("step `{}`: unknown peer `{name}`", st.verb));
        }
        ix.map(ActorIx::from)
    }

    fn link_actors(&self) -> Vec<ActorIx> {
        (0..self.actors.len())
            .filter(|i| self.actors[*i].raw.is_none())
            .map(ActorIx::from)
            .collect()
    }

    fn pc(&self, r: PcRef) -> Option<&Pc> {
        self.actors[r.actor].pc(r.handle)
    }

    fn pc_mut(&mut self, r: PcRef) -> Option<&mut Pc> {
        self.actors[r.actor].pc_mut(r.handle)
    }

    fn sock_owner(&self, sock: SockIx) -> Option<SockOwner> {
        self.socks.get(&sock).copied()
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
    fn select_sock(&mut self, sock: SockIx) {
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
