//! The mailbox set of a `Link` (`alpha-connection-model`, design section 1): the rendezvous
//! channels through which a pairing exchanges signaling. The core decides which mailbox is open
//! and speaks each kind's wire protocol; the platform only owns one socket per mailbox id, opens
//! it when the core emits `open_mailbox`, closes it on `close_mailbox`, and feeds
//! `mailbox_opened`, `mailbox_closed` and `mailbox_frame` back. A platform never reconnects, fails
//! over or retries a mailbox on its own.
//!
//! The set holds one mailbox per `relay` entry of the pairing (JSON text frames of the relay
//! protocol; the mailbox id is its position). The LAN listener joins with `add-offline-lan`.

use serde::{Deserialize, Serialize};

use crate::protocol::Member;

/// Identifies one mailbox of the set for the lifetime of a `Link`. A mailbox keeps its id across
/// closing and reopening; the platform's socket for it does not (late events of a closed socket
/// are the platform's to discard).
pub type MailboxId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub enum MailboxKind {
    /// Our signaling relay (`signaling-relay`): `hello`/`welcome`/`members`/`peer`/`signal`.
    Relay,
    /// A LAN mailbox (`l:` in an invite); the listener joins with `add-offline-lan`.
    Lan,
}

/// Transport state of one mailbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailboxState {
    Closed,
    Opening,
    /// Socket open, `hello` sent, no `welcome` yet.
    Open,
    Joined,
}

/// One relay-protocol mailbox with the state its protocol needs: transport state, backoff, room
/// membership on this relay and what the core learned from the peer on it.
#[derive(Debug)]
pub(crate) struct Mailbox {
    pub id: MailboxId,
    pub kind: MailboxKind,
    /// The address the platform opens for this mailbox (one `r:` or `l:` entry of the pairing).
    pub url: String,
    pub state: MailboxState,
    pub changed_at: u64,
    pub failures: u32,
    pub next_open_at: u64,
    pub last_rx: u64,
    pub last_ping: u64,
    pub client_id: Option<String>,
    pub members: Vec<Member>,
    /// Relay client ids of the members that carry the peer's role, as of the last `members`.
    pub known_peers: Vec<String>,
    /// The client id the peer's confirmed session last spoke from on this relay. A confirmed
    /// session counts as reachable here only while a member with this id exists.
    pub peer_client: Option<String>,
    /// Unsolicited `here`s sent since a peer member appeared on this relay, and the next repeat.
    pub here_sent: u32,
    pub here_next_at: u64,
    /// This relay answered `hello` with `UNSUPPORTED_PROTOCOL` (cleared by a `welcome`).
    pub unsupported: bool,
}

impl Mailbox {
    fn new(id: MailboxId, kind: MailboxKind, url: String) -> Self {
        Self {
            id,
            kind,
            url,
            state: MailboxState::Closed,
            changed_at: 0,
            failures: 0,
            next_open_at: 0,
            last_rx: 0,
            last_ping: 0,
            client_id: None,
            members: Vec::new(),
            known_peers: Vec::new(),
            peer_client: None,
            here_sent: 0,
            here_next_at: 0,
            unsupported: false,
        }
    }

    pub fn peer_present(&self) -> bool {
        !self.known_peers.is_empty()
    }
}

/// The set: one mailbox per `relay` or `lan` entry of the pairing (a LAN signaling server speaks
/// the relay protocol too), with ids 0, 1, ... in the order of the entries. The LAN listener of
/// `add-offline-lan` joins later.
#[derive(Debug)]
pub(crate) struct MailboxSet {
    pub relays: Vec<Mailbox>,
}

impl MailboxSet {
    /// `entries` are the validated `(kind, url)` pairs; duplicates by URL are dropped.
    pub fn new(entries: &[(MailboxKind, String)]) -> Self {
        let mut relays: Vec<Mailbox> = Vec::new();
        for (kind, url) in entries {
            if relays.iter().any(|m| m.url == *url) {
                continue;
            }
            let id = u32::try_from(relays.len()).unwrap_or(u32::MAX);
            relays.push(Mailbox::new(id, *kind, url.clone()));
        }
        Self { relays }
    }

    pub fn get(&self, id: MailboxId) -> Option<&Mailbox> {
        self.relays.get(id as usize)
    }
}
