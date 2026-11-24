//! The data of the simulated world: peer connections, actors, relay connections, faults, flows.

use std::ops::{Index, IndexMut};

use super::*;

#[derive(Debug, Clone)]
pub enum ToClient {
    Opened,
    Text(String),
    Closed,
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
    pub(super) pending_mailboxes: Option<Vec<InviteMailbox>>,
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
    pub(super) held_up: Vec<(SockIx, String)>,
    pub(super) held_down: Vec<(SockIx, ToClient)>,
    pub(super) chain: u32,
    pub(super) last_input_at: Time,
    pub last_rx_data: Time,
    pub(super) rec: Option<usize>,
    pub instance: u32,
    pub events: Vec<EvRec>,
    pub statuses: Vec<(Time, LinkStatus)>,
    pub welcomes: Vec<Time>,
    pub sent_signals: Vec<(Time, String)>,
    pub next_raw_handle: u32,
    /// Simulated wall-clock second last reported to the link.
    pub(super) last_wall_ms: Option<u64>,
    /// This device's clock is this many seconds ahead of true time (negative: behind).
    pub(super) wall_offset_s: i64,
}

impl Actor {
    pub(super) fn new(name: &str, role: Role, cfg: LinkConfig, room: String) -> Self {
        let (sdp_dir, talk_dir) = match role {
            Role::Baby => ("sendonly", "recvonly"),
            Role::Parent => ("recvonly", "sendonly"),
        };
        Actor {
            name: name.to_owned(),
            role,
            link: None,
            cfg,
            raw: None,
            pending_mailboxes: None,
            room,
            sdp_dir: sdp_dir.into(),
            talk_dir: talk_dir.into(),
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

    pub(super) fn pc(&self, handle: PcHandle) -> Option<&Pc> {
        self.pcs.get(&handle.raw())
    }

    pub(super) fn pc_mut(&mut self, handle: PcHandle) -> Option<&mut Pc> {
        self.pcs.get_mut(&handle.raw())
    }

    /// Whether `sock` is one of this actor's mailbox sockets.
    pub(super) fn owns(&self, sock: SockIx) -> bool {
        self.socks.values().any(|s| *s == sock.index())
    }

    /// Whether any peer connection of this actor is still open (not closed, failed or not).
    pub(super) fn has_open_pc(&self) -> bool {
        self.pcs.values().any(|p| !p.closed)
    }
}

impl Index<ActorIx> for Vec<Actor> {
    type Output = Actor;

    fn index(&self, a: ActorIx) -> &Actor {
        &self[a.index()]
    }
}

impl IndexMut<ActorIx> for Vec<Actor> {
    fn index_mut(&mut self, a: ActorIx) -> &mut Actor {
        &mut self[a.index()]
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

impl Conn {
    /// A freshly accepted connection of actor `a`, not yet in a room.
    pub(super) fn accepted(a: ActorIx, epoch: u32, role: Role) -> Self {
        Conn {
            actor: Some(a.index()),
            epoch,
            role,
            client_id: String::new(),
            requested: String::new(),
            key: String::new(),
            joined: false,
            alive: true,
            join_seq: 0,
        }
    }

    pub(super) fn actor_ix(&self) -> Option<ActorIx> {
        self.actor.map(ActorIx::from)
    }
}

/// The room a client is seated in on the relay (aliased rooms share one key).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RoomKey(String);

impl RoomKey {
    pub(super) fn new(key: String) -> Self {
        RoomKey(key)
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

/// A seat a client took in a room: what the relay learns from a `hello`.
pub(super) struct Seat {
    pub role: Role,
    pub client_id: String,
    pub requested: String,
    pub key: RoomKey,
    pub join_seq: u64,
}

impl Conn {
    pub(super) fn take_seat(&mut self, seat: Seat) {
        self.role = seat.role;
        self.client_id = seat.client_id;
        self.requested = seat.requested;
        self.key = seat.key.0;
        self.joined = true;
        self.join_seq = seat.join_seq;
    }

    /// A socket whose client died silently: in a room, owned by nobody.
    pub(super) fn ghost(seat: Seat) -> Self {
        let mut conn = Conn::accepted(ActorIx::from(0), 0, seat.role);
        conn.actor = None;
        conn.take_seat(seat);
        conn
    }
}

pub(super) struct Server {
    /// Accepts and acknowledges frames but delivers no `signal` (a relay that is up and mute).
    pub drop_signals: bool,
    pub up: bool,
    pub conns: BTreeMap<SockIx, Conn>,
    pub sweep_gen: u32,
    pub next_client: u32,
    pub next_join: u64,
    /// The latest `stop_notice` payload the relay saw and the client id it came from (a hostile
    /// relay records it for `replay_stop`).
    pub last_stop: Option<(String, Value)>,
    /// Every `hello` before this time is answered with `UNSUPPORTED_PROTOCOL`.
    pub reject_hello_until: Time,
    /// Rooms with a pending half-TTL renewal (`RelaySpec::ice`).
    pub ice_timers: BTreeSet<RoomKey>,
    pub ice_seq: u64,
}

impl Server {
    pub(super) fn new() -> Self {
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

    /// The next client number and join sequence (both advance together at every join).
    pub(super) fn next_ids(&mut self) -> (u32, u64) {
        self.next_client += 1;
        self.next_join += 1;
        (self.next_client, self.next_join)
    }
}

/// A client frame on its way to the relay; `spoof` makes the relay claim another sender.
#[derive(Debug, Clone)]
pub(super) struct Frame {
    pub text: String,
    pub spoof: Option<String>,
}

/// Which socket of whom, and on which network path it was opened.
#[derive(Clone, Copy)]
pub(super) struct SockOwner {
    pub actor: ActorIx,
    pub epoch: u32,
}

/// An ordered channel: traffic of one flow never overtakes earlier traffic of that flow.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Flow {
    /// Relay to client frames of a socket.
    Down(SockIx),
    /// Client to relay frames of a socket.
    Up(SockIx),
    /// DataChannel messages arriving at a peer connection.
    Data(PcRef),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Data,
    Signaling,
}

#[derive(Clone, Copy)]
pub(super) enum FaultKind {
    Drop,
    Delay,
    Duplicate,
    Reorder,
}

#[derive(Clone)]
pub(super) struct Fault {
    pub kind: FaultKind,
    pub from: Option<usize>,
    pub via: Via,
    /// Only messages whose text contains this.
    pub contains: Option<String>,
    pub start: Time,
    pub until: Time,
    pub p: f64,
    pub ms: u64,
    pub jitter: u64,
}

/// How one message copy travels: extra delay on top of the base latency, and whether it may
/// overtake earlier traffic of its flow.
#[derive(Clone, Copy)]
pub(super) struct Delivery {
    pub extra: u64,
    pub bypass: bool,
}

impl Delivery {
    /// Normal latency, in order.
    pub(super) const PLAIN: Delivery = Delivery {
        extra: 0,
        bypass: false,
    };
}
