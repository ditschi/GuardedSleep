//! Glue between the sans-IO relay and the sockets: one lock around the relay and the per-socket
//! outboxes, so the effects of one call reach a socket in the order the relay produced them.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

use guardedsleep_core::pairing::AdmissionKey;
use guardedsleep_relay::{
    Admission, ClientKey, CloseReason, ConnId, Entropy, Relay, RelayConfig, RelayEffect,
};
use tokio::sync::mpsc;

/// Messages queued per socket before it counts as stalled and is dropped.
const OUTBOX: usize = 256;

#[derive(Debug)]
pub enum Out {
    Text(String),
    Ping,
    Close(CloseReason),
}

/// Randomness from the operating system. A failing OS generator is not recoverable: nonces and ids
/// must be unpredictable, so the process stops rather than continue with weak ones.
pub struct OsEntropy;

impl Entropy for OsEntropy {
    fn fill(&mut self, buf: &mut [u8]) {
        if getrandom::fill(buf).is_err() {
            std::process::abort();
        }
    }
}

struct Inner {
    relay: Relay<OsEntropy>,
    outboxes: HashMap<ConnId, mpsc::Sender<Out>>,
}

pub struct Hub {
    inner: Mutex<Inner>,
    next_id: AtomicU64,
    start: Instant,
    log_events: bool,
}

/// Opaque rate-limit key of a client address; IPv4 is mapped into IPv6 so both families share one
/// key space. The relay never logs it.
pub fn client_key(ip: IpAddr) -> ClientKey {
    let v6 = match ip {
        IpAddr::V4(a) => a.to_ipv6_mapped(),
        IpAddr::V6(a) => a,
    };
    ClientKey(u128::from(v6))
}

#[derive(Debug, PartialEq, Eq)]
pub enum KeyError {
    /// The relay holds a different key for this room; remove it first.
    Conflict,
    /// The relay holds too many rooms.
    Full,
}

impl Hub {
    pub fn new(cfg: RelayConfig, log_events: bool) -> Self {
        Self {
            inner: Mutex::new(Inner {
                relay: Relay::new(cfg, OsEntropy),
                outboxes: HashMap::new(),
            }),
            next_id: AtomicU64::new(1),
            start: Instant::now(),
            log_events,
        }
    }

    fn now(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // A poisoned lock means a panic mid-update; the relay state cannot be trusted any more.
        self.inner.lock().unwrap_or_else(|_| std::process::abort())
    }

    pub fn admission(&self) -> Admission {
        self.lock().relay.admission()
    }

    pub fn connect(&self, ip: IpAddr) -> (ConnId, mpsc::Receiver<Out>) {
        let id = ConnId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let (tx, rx) = mpsc::channel(OUTBOX);
        let mut g = self.lock();
        g.outboxes.insert(id, tx);
        let fx = g.relay.on_connect(id, client_key(ip), self.now());
        self.dispatch(&mut g, fx);
        (id, rx)
    }

    pub fn frame(&self, id: ConnId, bytes: &[u8]) {
        let mut g = self.lock();
        let fx = g.relay.on_frame(id, bytes, self.now());
        self.dispatch(&mut g, fx);
    }

    pub fn pong(&self, id: ConnId) {
        self.lock().relay.on_pong(id);
    }

    pub fn closed(&self, id: ConnId) {
        let mut g = self.lock();
        g.outboxes.remove(&id);
        let fx = g.relay.on_close(id);
        self.dispatch(&mut g, fx);
    }

    pub fn tick(&self) {
        let mut g = self.lock();
        let fx = g.relay.on_tick(self.now());
        self.dispatch(&mut g, fx);
    }

    /// Registers the admission key of a room. The same key again is accepted; a different key for a
    /// room that already has one is refused until the room is removed.
    pub fn set_admission_key(&self, room_id: &str, key: AdmissionKey) -> Result<(), KeyError> {
        let mut g = self.lock();
        if g.relay.admission_key_matches(room_id, &key) == Some(false) {
            return Err(KeyError::Conflict);
        }
        if g.relay.set_admission_key(room_id, key) {
            Ok(())
        } else {
            Err(KeyError::Full)
        }
    }

    pub fn remove_admission_key(&self, room_id: &str) {
        self.lock().relay.remove_admission_key(room_id);
    }

    /// Performs relay effects. A socket whose outbox is full is dropped like a closed one, which
    /// can produce further effects (the member list of its room), hence the loop.
    fn dispatch(&self, g: &mut Inner, mut effects: Vec<RelayEffect>) {
        while !effects.is_empty() {
            let mut stalled = Vec::new();
            for e in effects {
                match e {
                    RelayEffect::Send { conn, text } => {
                        self.try_queue(g, conn, Out::Text(text), &mut stalled)
                    }
                    RelayEffect::Ping { conn } => self.try_queue(g, conn, Out::Ping, &mut stalled),
                    RelayEffect::Close { conn, reason } => {
                        self.try_queue(g, conn, Out::Close(reason), &mut stalled);
                        g.outboxes.remove(&conn);
                    }
                    RelayEffect::Log { event } => {
                        if self.log_events {
                            eprintln!("relay: {event}");
                        }
                    }
                }
            }
            effects = Vec::new();
            for conn in stalled {
                g.outboxes.remove(&conn);
                effects.extend(g.relay.on_close(conn));
            }
        }
    }

    fn try_queue(&self, g: &mut Inner, conn: ConnId, out: Out, stalled: &mut Vec<ConnId>) {
        if let Some(tx) = g.outboxes.get(&conn)
            && tx.try_send(out).is_err()
        {
            stalled.push(conn);
        }
    }
}
