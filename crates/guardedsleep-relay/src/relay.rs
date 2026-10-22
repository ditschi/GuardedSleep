//! Rooms, admission, limits and liveness as a pure state machine.

use std::collections::HashMap;
use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use guardedsleep_core::pairing::{AdmissionKey, verify_admission};
use guardedsleep_core::protocol::{
    ClientMessage, LEGACY_PROTOCOL_VERSION, PROTOCOL_VERSION, Role, encode_challenge,
    parse_client_message,
};
use serde_json::{Value, json};

use crate::limiter::RateLimiter;

/// Monotonic milliseconds supplied by the caller.
pub type Millis = u64;

/// Handle for one socket, chosen by the platform. Never reuse an id while the relay may still know
/// it (the relay forgets it on `Close` or `on_close`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnId(pub u64);

/// Opaque rate-limit key of a client, e.g. a hash of its IP address. The relay only compares it
/// and never logs it (AGENTS.md section 7: no IPs in logs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientKey(pub u128);

/// Source of unpredictable bytes, injected so that the crate stays free of OS randomness.
pub trait Entropy {
    fn fill(&mut self, buf: &mut [u8]);
}

/// Splitmix64 stream for tests and simulation. Predictable: never use it on a live listener.
#[derive(Debug, Clone)]
pub struct DeterministicEntropy(u64);

impl DeterministicEntropy {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

impl Entropy for DeterministicEntropy {
    fn fill(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(8) {
            let bytes = self.next().to_le_bytes();
            for (dst, src) in chunk.iter_mut().zip(bytes) {
                *dst = src;
            }
        }
    }
}

/// Who may join.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// Public server: it does not know any PSK, so joiners say `hello` and the end-to-end pairing
    /// proof plus DTLS protect the call, as before.
    Open,
    /// LAN relay (phone listener, PC local mode): every joiner must answer a nonce with the
    /// room's admission key (ADR-021 section 1). A room without a registered key admits nobody.
    Required,
}

#[derive(Debug, Clone)]
pub struct RelayConfig {
    pub admission: Admission,
    /// Parents per room; the MVP is one baby plus one parent.
    pub max_parents_per_room: usize,
    /// Milliseconds after connect by which `hello` must have completed.
    pub hello_timeout_ms: Millis,
    /// Milliseconds after connect by which a joiner must have been admitted (Required mode).
    pub admission_timeout_ms: Millis,
    /// A connection that did not answer the previous ping within one interval is dropped.
    pub ping_interval_ms: Millis,
    /// Join attempts per client key and minute; 0 disables.
    pub hello_rate_per_min: u32,
    /// New rooms per client key and minute; 0 disables.
    pub new_room_rate_per_min: u32,
    pub max_frame_bytes: usize,
    pub max_connections: usize,
    /// Open connections per client key, so one address cannot hold every slot; 0 disables.
    pub max_connections_per_client: usize,
    /// Frames per connection and minute before the connection has joined (a legitimate client
    /// sends one or two); 0 disables.
    pub frame_rate_unjoined_per_min: u32,
    /// Frames per connection and minute once joined (pings and ICE candidates); 0 disables.
    pub frame_rate_per_min: u32,
    /// Rooms whose admission key can be held at once (Required mode).
    pub max_admission_keys: usize,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            admission: Admission::Open,
            max_parents_per_room: 1,
            hello_timeout_ms: 30_000,
            admission_timeout_ms: 5_000,
            ping_interval_ms: 10_000,
            hello_rate_per_min: 20,
            new_room_rate_per_min: 10,
            max_frame_bytes: 64 * 1024,
            max_connections: 10_000,
            max_connections_per_client: 16,
            frame_rate_unjoined_per_min: 120,
            frame_rate_per_min: 600,
            max_admission_keys: 16,
        }
    }
}

/// Why the relay closes a connection. `code` is the WebSocket close code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
    HelloTimeout,
    AdmissionTimeout,
    AdmissionRequired,
    AdmissionFailed,
    RateLimited,
    VersionMismatch,
    InvalidRoomId,
    BabyTaken,
    ParentCap,
    TooLarge,
    Stale,
    Capacity,
}

impl CloseReason {
    pub fn code(self) -> u16 {
        match self {
            CloseReason::TooLarge => 1009,
            CloseReason::Stale => 1001,
            _ => 1008,
        }
    }

    pub fn text(self) -> &'static str {
        match self {
            CloseReason::HelloTimeout => "hello timeout",
            CloseReason::AdmissionTimeout => "admission timeout",
            CloseReason::AdmissionRequired => "admission required",
            CloseReason::AdmissionFailed => "admission failed",
            CloseReason::RateLimited => "rate limited",
            CloseReason::VersionMismatch => "version mismatch",
            CloseReason::InvalidRoomId => "invalid room id",
            CloseReason::BabyTaken => "baby taken",
            CloseReason::ParentCap => "parent cap",
            CloseReason::TooLarge => "message too large",
            CloseReason::Stale => "stale",
            CloseReason::Capacity => "server full",
        }
    }
}

/// Operational events. They carry no addresses, ids, room ids or key material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogEvent {
    Connected,
    Admitted,
    Joined(Role),
    Left,
    Closed(CloseReason),
}

impl fmt::Display for LogEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogEvent::Connected => f.write_str("connected"),
            LogEvent::Admitted => f.write_str("admitted"),
            LogEvent::Joined(role) => write!(f, "joined as {}", role.as_str()),
            LogEvent::Left => f.write_str("left"),
            LogEvent::Closed(reason) => write!(f, "closed: {}", reason.text()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelayEffect {
    /// Send one WebSocket text frame.
    Send {
        conn: ConnId,
        text: String,
    },
    /// Send a WebSocket ping.
    Ping {
        conn: ConnId,
    },
    /// Close the socket (after flushing earlier `Send`s for it).
    Close {
        conn: ConnId,
        reason: CloseReason,
    },
    Log {
        event: LogEvent,
    },
}

#[derive(Debug)]
struct Member {
    conn: ConnId,
    client_id: String,
    role: Role,
}

#[derive(Debug)]
struct Room {
    /// Protocol version of the first member; the room is pinned to it while it is non-empty.
    protocol_version: u64,
    /// In join order.
    members: Vec<Member>,
}

#[derive(Debug)]
struct Joined {
    room: String,
    client_id: String,
}

#[derive(Debug)]
struct Conn {
    client: ClientKey,
    connected_at: Millis,
    /// Pending admission nonce; consumed by the first `admit`.
    nonce: Option<String>,
    joined: Option<Joined>,
    awaiting_pong: bool,
}

#[derive(Debug)]
pub struct Relay<E: Entropy> {
    cfg: RelayConfig,
    entropy: E,
    conns: HashMap<ConnId, Conn>,
    rooms: HashMap<String, Room>,
    keys: HashMap<String, AdmissionKey>,
    hello_limiter: RateLimiter<ClientKey>,
    room_limiter: RateLimiter<ClientKey>,
    unjoined_frame_limiter: RateLimiter<ConnId>,
    joined_frame_limiter: RateLimiter<ConnId>,
    per_client: HashMap<ClientKey, usize>,
    last_liveness: Option<Millis>,
}

/// Room ids of v3 rooms are 128-bit HKDF output, base64url without padding (ADR-004 amendment).
fn valid_v3_room_id(room_id: &str) -> bool {
    room_id.len() == 22
        && room_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn error_text(code: &str, message: &str) -> String {
    json!({ "type": "error", "code": code, "message": message }).to_string()
}

fn send(conn: ConnId, text: String) -> RelayEffect {
    RelayEffect::Send { conn, text }
}

impl<E: Entropy> Relay<E> {
    pub fn new(cfg: RelayConfig, entropy: E) -> Self {
        Self {
            hello_limiter: RateLimiter::new(cfg.hello_rate_per_min),
            room_limiter: RateLimiter::new(cfg.new_room_rate_per_min),
            // Entries are removed with their connection, so the map is bounded by `max_connections`.
            unjoined_frame_limiter: RateLimiter::new(cfg.frame_rate_unjoined_per_min)
                .with_max_keys(usize::MAX),
            joined_frame_limiter: RateLimiter::new(cfg.frame_rate_per_min)
                .with_max_keys(usize::MAX),
            per_client: HashMap::new(),
            cfg,
            entropy,
            conns: HashMap::new(),
            rooms: HashMap::new(),
            keys: HashMap::new(),
            last_liveness: None,
        }
    }

    pub fn admission(&self) -> Admission {
        self.cfg.admission
    }

    pub fn room_count(&self) -> usize {
        self.rooms.len()
    }

    pub fn conn_count(&self) -> usize {
        self.conns.len()
    }

    /// Registers the admission key of one room (Required mode). Returns `false` when the relay
    /// already holds `max_admission_keys` other rooms.
    pub fn set_admission_key(&mut self, room_id: &str, key: AdmissionKey) -> bool {
        if !self.keys.contains_key(room_id) && self.keys.len() >= self.cfg.max_admission_keys {
            return false;
        }
        self.keys.insert(room_id.to_owned(), key);
        true
    }

    /// `None` when no key is registered for the room, else whether it equals `key`.
    pub fn admission_key_matches(&self, room_id: &str, key: &AdmissionKey) -> Option<bool> {
        self.keys.get(room_id).map(|k| k == key)
    }

    pub fn remove_admission_key(&mut self, room_id: &str) {
        self.keys.remove(room_id);
    }

    fn random_bytes<const N: usize>(&mut self) -> [u8; N] {
        let mut buf = [0u8; N];
        self.entropy.fill(&mut buf);
        buf
    }

    fn new_client_id(&mut self) -> String {
        let mut b: [u8; 16] = self.random_bytes();
        // RFC 9562 version 4 / variant 1 bits; the web client treats the id as opaque.
        if let Some(v) = b.get_mut(6) {
            *v = (*v & 0x0f) | 0x40;
        }
        if let Some(v) = b.get_mut(8) {
            *v = (*v & 0x3f) | 0x80;
        }
        let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
        format!(
            "{}-{}-{}-{}-{}",
            hex.get(0..8).unwrap_or_default(),
            hex.get(8..12).unwrap_or_default(),
            hex.get(12..16).unwrap_or_default(),
            hex.get(16..20).unwrap_or_default(),
            hex.get(20..32).unwrap_or_default(),
        )
    }

    pub fn on_connect(&mut self, conn: ConnId, client: ClientKey, now: Millis) -> Vec<RelayEffect> {
        let mut out = Vec::new();
        if self.conns.contains_key(&conn) {
            return out;
        }
        let per_client_cap = self.cfg.max_connections_per_client;
        let over_client_cap = per_client_cap > 0
            && self.per_client.get(&client).copied().unwrap_or(0) >= per_client_cap;
        if self.conns.len() >= self.cfg.max_connections || over_client_cap {
            out.push(RelayEffect::Close {
                conn,
                reason: CloseReason::Capacity,
            });
            out.push(RelayEffect::Log {
                event: LogEvent::Closed(CloseReason::Capacity),
            });
            return out;
        }
        let nonce = (self.cfg.admission == Admission::Required)
            .then(|| URL_SAFE_NO_PAD.encode(self.random_bytes::<16>()));
        if let Some(n) = &nonce {
            out.push(send(conn, encode_challenge(n)));
        }
        self.last_liveness.get_or_insert(now);
        *self.per_client.entry(client).or_insert(0) += 1;
        self.conns.insert(
            conn,
            Conn {
                client,
                connected_at: now,
                nonce,
                joined: None,
                awaiting_pong: false,
            },
        );
        out.push(RelayEffect::Log {
            event: LogEvent::Connected,
        });
        out
    }

    /// The socket is gone. Idempotent.
    pub fn on_close(&mut self, conn: ConnId) -> Vec<RelayEffect> {
        let mut out = Vec::new();
        self.drop_conn(conn, None, &mut out);
        out
    }

    pub fn on_pong(&mut self, conn: ConnId) {
        if let Some(c) = self.conns.get_mut(&conn) {
            c.awaiting_pong = false;
        }
    }

    pub fn on_tick(&mut self, now: Millis) -> Vec<RelayEffect> {
        let mut out = Vec::new();
        let required = self.cfg.admission == Admission::Required;
        let deadline = if required {
            self.cfg.admission_timeout_ms
        } else {
            self.cfg.hello_timeout_ms
        };
        let reason = if required {
            CloseReason::AdmissionTimeout
        } else {
            CloseReason::HelloTimeout
        };
        let mut expired: Vec<ConnId> = self
            .conns
            .iter()
            .filter(|(_, c)| c.joined.is_none() && now.saturating_sub(c.connected_at) >= deadline)
            .map(|(id, _)| *id)
            .collect();
        expired.sort();
        for conn in expired {
            self.drop_conn(conn, Some(reason), &mut out);
        }

        let last = *self.last_liveness.get_or_insert(now);
        if now.saturating_sub(last) >= self.cfg.ping_interval_ms {
            self.last_liveness = Some(now);
            let mut ids: Vec<ConnId> = self.conns.keys().copied().collect();
            ids.sort();
            for conn in ids {
                let Some(c) = self.conns.get_mut(&conn) else {
                    continue;
                };
                if c.awaiting_pong {
                    self.drop_conn(conn, Some(CloseReason::Stale), &mut out);
                } else {
                    c.awaiting_pong = true;
                    out.push(RelayEffect::Ping { conn });
                }
            }
            self.hello_limiter.prune(now);
            self.room_limiter.prune(now);
        }
        out
    }

    pub fn on_frame(&mut self, conn: ConnId, bytes: &[u8], now: Millis) -> Vec<RelayEffect> {
        let mut out = Vec::new();
        let Some(c) = self.conns.get_mut(&conn) else {
            return out;
        };
        c.awaiting_pong = false;
        let within_budget = if c.joined.is_some() {
            self.joined_frame_limiter.take(conn, now)
        } else {
            self.unjoined_frame_limiter.take(conn, now)
        };
        if !within_budget {
            self.reject(
                conn,
                "RATE_LIMITED",
                "too many messages",
                CloseReason::RateLimited,
                &mut out,
            );
            return out;
        }
        if bytes.len() > self.cfg.max_frame_bytes {
            self.drop_conn(conn, Some(CloseReason::TooLarge), &mut out);
            return out;
        }
        let msg = std::str::from_utf8(bytes)
            .ok()
            .and_then(parse_client_message);
        let Some(msg) = msg else {
            out.push(send(conn, error_text("BAD_JSON", "invalid message")));
            return out;
        };
        match msg {
            ClientMessage::Ping => out.push(send(conn, json!({ "type": "pong" }).to_string())),
            ClientMessage::Hello {
                role,
                room_id,
                protocol_version,
            } => self.on_hello(conn, role, room_id, protocol_version, now, &mut out),
            ClientMessage::Admit {
                role,
                room_id,
                protocol_version,
                mac,
            } => self.on_admit(conn, role, room_id, protocol_version, &mac, now, &mut out),
            ClientMessage::Signal {
                room_id,
                to,
                payload,
            } => self.on_signal(conn, &room_id, to.as_deref(), payload, &mut out),
        }
        out
    }

    fn is_joined(&self, conn: ConnId) -> bool {
        self.conns.get(&conn).is_some_and(|c| c.joined.is_some())
    }

    fn on_hello(
        &mut self,
        conn: ConnId,
        role: Role,
        room_id: String,
        version: u64,
        now: Millis,
        out: &mut Vec<RelayEffect>,
    ) {
        if self.is_joined(conn) {
            out.push(send(conn, error_text("INVALID_MESSAGE", "already joined")));
            return;
        }
        if self.cfg.admission == Admission::Required {
            self.reject(
                conn,
                "ADMISSION_REQUIRED",
                "admission required",
                CloseReason::AdmissionRequired,
                out,
            );
            return;
        }
        if !self.take_hello(conn, now) {
            self.reject(
                conn,
                "RATE_LIMITED",
                "too many attempts",
                CloseReason::RateLimited,
                out,
            );
            return;
        }
        self.join(conn, role, room_id, version, now, out);
    }

    #[allow(clippy::too_many_arguments)]
    fn on_admit(
        &mut self,
        conn: ConnId,
        role: Role,
        room_id: String,
        version: u64,
        mac: &str,
        now: Millis,
        out: &mut Vec<RelayEffect>,
    ) {
        if self.cfg.admission != Admission::Required {
            out.push(send(
                conn,
                error_text("INVALID_MESSAGE", "unexpected message"),
            ));
            return;
        }
        if self.is_joined(conn) {
            out.push(send(conn, error_text("INVALID_MESSAGE", "already joined")));
            return;
        }
        if !self.take_hello(conn, now) {
            self.reject(
                conn,
                "RATE_LIMITED",
                "too many attempts",
                CloseReason::RateLimited,
                out,
            );
            return;
        }
        // One attempt per connection: the nonce is spent whether or not the answer verifies.
        let nonce = self.conns.get_mut(&conn).and_then(|c| c.nonce.take());
        let admitted = match (nonce, self.keys.get(&room_id)) {
            (Some(nonce), Some(key)) => verify_admission(key, &nonce, role, &room_id, mac),
            _ => false,
        };
        if !admitted {
            self.reject(
                conn,
                "ADMISSION_FAILED",
                "admission failed",
                CloseReason::AdmissionFailed,
                out,
            );
            return;
        }
        out.push(RelayEffect::Log {
            event: LogEvent::Admitted,
        });
        self.join(conn, role, room_id, version, now, out);
    }

    fn take_hello(&mut self, conn: ConnId, now: Millis) -> bool {
        match self.conns.get(&conn).map(|c| c.client) {
            Some(client) => self.hello_limiter.take(client, now),
            None => false,
        }
    }

    /// Everything after the caller has been authenticated (Required) or rate limited (Open).
    fn join(
        &mut self,
        conn: ConnId,
        role: Role,
        room_id: String,
        version: u64,
        now: Millis,
        out: &mut Vec<RelayEffect>,
    ) {
        let supported =
            version == u64::from(PROTOCOL_VERSION) || version == u64::from(LEGACY_PROTOCOL_VERSION);
        if !supported {
            self.reject_version(conn, out);
            return;
        }
        if version >= 3 && !valid_v3_room_id(&room_id) {
            self.reject(
                conn,
                "INVALID_MESSAGE",
                "invalid room id",
                CloseReason::InvalidRoomId,
                out,
            );
            return;
        }
        match self.rooms.get(&room_id).filter(|r| !r.members.is_empty()) {
            Some(room) => {
                if room.protocol_version != version {
                    self.reject_version(conn, out);
                    return;
                }
            }
            None => {
                let client = self.conns.get(&conn).map(|c| c.client);
                if !client.is_some_and(|k| self.room_limiter.take(k, now)) {
                    self.reject(
                        conn,
                        "RATE_LIMITED",
                        "too many rooms",
                        CloseReason::RateLimited,
                        out,
                    );
                    return;
                }
            }
        }
        let max_parents = self.cfg.max_parents_per_room;
        let verdict = self.rooms.get(&room_id).map_or(Ok(()), |room| match role {
            Role::Baby if room.members.iter().any(|m| m.role == Role::Baby) => Err((
                "BABY_TAKEN",
                "room already has a baby unit",
                CloseReason::BabyTaken,
            )),
            Role::Parent
                if room
                    .members
                    .iter()
                    .filter(|m| m.role == Role::Parent)
                    .count()
                    >= max_parents =>
            {
                Err((
                    "PARENT_CAP",
                    "too many parent units",
                    CloseReason::ParentCap,
                ))
            }
            _ => Ok(()),
        });
        if let Err((code, message, reason)) = verdict {
            self.reject(conn, code, message, reason, out);
            return;
        }

        let client_id = self.new_client_id();
        let room = self.rooms.entry(room_id.clone()).or_insert_with(|| Room {
            protocol_version: version,
            members: Vec::new(),
        });
        room.members.push(Member {
            conn,
            client_id: client_id.clone(),
            role,
        });
        if let Some(c) = self.conns.get_mut(&conn) {
            c.joined = Some(Joined {
                room: room_id.clone(),
                client_id: client_id.clone(),
            });
        }
        out.push(RelayEffect::Log {
            event: LogEvent::Joined(role),
        });
        out.push(send(
            conn,
            json!({ "type": "welcome", "clientId": client_id, "roomId": room_id }).to_string(),
        ));
        self.broadcast_members(&room_id, out);
        if let Some(room) = self.rooms.get(&room_id) {
            let announce = json!({ "type": "peer", "peerId": client_id }).to_string();
            for m in room.members.iter().filter(|m| m.conn != conn) {
                out.push(send(m.conn, announce.clone()));
            }
            let first_other = room
                .members
                .iter()
                .find(|m| m.conn != conn)
                .map(|m| m.client_id.as_str());
            out.push(send(
                conn,
                json!({ "type": "peer", "peerId": first_other }).to_string(),
            ));
        }
    }

    fn on_signal(
        &mut self,
        conn: ConnId,
        room_id: &str,
        to: Option<&str>,
        payload: Value,
        out: &mut Vec<RelayEffect>,
    ) {
        let Some(joined) = self
            .conns
            .get(&conn)
            .and_then(|c| c.joined.as_ref())
            .map(|j| (j.room.clone(), j.client_id.clone()))
        else {
            out.push(send(conn, error_text("NOT_JOINED", "send hello first")));
            return;
        };
        let (own_room, own_id) = joined;
        if room_id != own_room {
            out.push(send(conn, error_text("INVALID_MESSAGE", "room mismatch")));
            return;
        }
        let Some(room) = self.rooms.get(&own_room) else {
            return;
        };
        let target = match to {
            Some(id) => match room
                .members
                .iter()
                .find(|m| m.client_id == id && m.client_id != own_id)
            {
                Some(m) => m,
                None => {
                    out.push(send(
                        conn,
                        error_text("INVALID_MESSAGE", "unknown signal target"),
                    ));
                    return;
                }
            },
            None if room.members.len() <= 2 => {
                match room.members.iter().find(|m| m.client_id != own_id) {
                    Some(m) => m,
                    None => return,
                }
            }
            None => {
                out.push(send(
                    conn,
                    error_text(
                        "SIGNAL_TARGET_REQUIRED",
                        "set signal.to when more than two peers are in the room",
                    ),
                ));
                return;
            }
        };
        out.push(send(
            target.conn,
            json!({ "type": "signal", "from": own_id, "payload": payload }).to_string(),
        ));
    }

    fn reject_version(&mut self, conn: ConnId, out: &mut Vec<RelayEffect>) {
        self.reject(
            conn,
            "UNSUPPORTED_PROTOCOL",
            "version mismatch",
            CloseReason::VersionMismatch,
            out,
        );
    }

    fn reject(
        &mut self,
        conn: ConnId,
        code: &str,
        message: &str,
        reason: CloseReason,
        out: &mut Vec<RelayEffect>,
    ) {
        out.push(send(conn, error_text(code, message)));
        self.drop_conn(conn, Some(reason), out);
    }

    fn broadcast_members(&self, room_id: &str, out: &mut Vec<RelayEffect>) {
        let Some(room) = self.rooms.get(room_id) else {
            return;
        };
        let peers: Vec<Value> = room
            .members
            .iter()
            .map(|m| json!({ "clientId": m.client_id, "role": m.role.as_str() }))
            .collect();
        let text = json!({ "type": "members", "peers": peers }).to_string();
        for m in &room.members {
            out.push(send(m.conn, text.clone()));
        }
    }

    /// Forgets the connection; with a reason, also tells the platform to close it. The remaining
    /// members of its room get the new member list.
    fn drop_conn(&mut self, conn: ConnId, reason: Option<CloseReason>, out: &mut Vec<RelayEffect>) {
        let Some(c) = self.conns.remove(&conn) else {
            return;
        };
        self.unjoined_frame_limiter.remove(&conn);
        self.joined_frame_limiter.remove(&conn);
        if let Some(n) = self.per_client.get_mut(&c.client) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                self.per_client.remove(&c.client);
            }
        }
        if let Some(reason) = reason {
            out.push(RelayEffect::Close { conn, reason });
            out.push(RelayEffect::Log {
                event: LogEvent::Closed(reason),
            });
        }
        let Some(joined) = c.joined else {
            return;
        };
        out.push(RelayEffect::Log {
            event: LogEvent::Left,
        });
        let Some(room) = self.rooms.get_mut(&joined.room) else {
            return;
        };
        room.members.retain(|m| m.conn != conn);
        if room.members.is_empty() {
            self.rooms.remove(&joined.room);
        } else {
            self.broadcast_members(&joined.room, out);
        }
    }
}

#[cfg(test)]
mod tests;
