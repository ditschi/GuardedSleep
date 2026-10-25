//! The mailbox set of a `Link` (`alpha-connection-model`, design section 1): the rendezvous
//! channels through which a pairing exchanges signaling. The core decides which mailbox is open
//! and speaks each kind's wire protocol; the platform only owns one socket per mailbox id, opens
//! it when the core emits `open_mailbox`, closes it on `close_mailbox`, and feeds
//! `mailbox_opened`, `mailbox_closed` and `mailbox_frame` back. A platform never reconnects, fails
//! over or retries a mailbox on its own.
//!
//! The set holds the `relay` mailbox (our signaling server, JSON text frames of the relay protocol,
//! id [`RELAY_MAILBOX`]) and one `nostr` mailbox per Nostr entry of the pairing (public relays,
//! NIP-01 frames, ids from 1). The `lan` kind joins with `add-offline-lan`.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::protocol::Member;

/// Identifies one mailbox of the set for the lifetime of a `Link`. A mailbox keeps its id across
/// closing and reopening; the platform's socket for it does not (late events of a closed socket
/// are the platform's to discard).
pub type MailboxId = u32;

/// The only mailbox of the first stage: our relay.
pub const RELAY_MAILBOX: MailboxId = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub enum MailboxKind {
    /// Our signaling relay (`signaling-relay`): `hello`/`welcome`/`members`/`peer`/`signal`.
    Relay,
    /// A Nostr relay (`n:` in an invite); its wire protocol joins with task 3.5.
    Nostr,
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

/// One mailbox with the state its kind's protocol needs. The relay fields (`client_id`,
/// `members`) are meaningful for the `relay` kind only.
#[derive(Debug)]
pub(crate) struct Mailbox {
    pub id: MailboxId,
    pub kind: MailboxKind,
    /// The address to open. Empty means "the platform's configured address" (the first stage keeps
    /// the URL list on the platform; invite v4, task 3.3, moves it into the core).
    pub url: String,
    pub state: MailboxState,
    pub changed_at: u64,
    pub failures: u32,
    pub next_open_at: u64,
    pub last_rx: u64,
    pub last_ping: u64,
    pub client_id: Option<String>,
    pub members: Vec<Member>,
}

impl Mailbox {
    fn relay() -> Self {
        Self {
            id: RELAY_MAILBOX,
            kind: MailboxKind::Relay,
            url: String::new(),
            state: MailboxState::Closed,
            changed_at: 0,
            failures: 0,
            next_open_at: 0,
            last_rx: 0,
            last_ping: 0,
            client_id: None,
            members: Vec::new(),
        }
    }
}

/// A sealed envelope waiting for its turn on a Nostr relay. It is wrapped in a freshly signed
/// event at the moment it is sent, so a resend of the same envelope is a new event.
#[derive(Debug, Clone)]
pub(crate) struct Outgoing {
    pub sealed: Vec<u8>,
    /// Presence (`here`) still goes when the hourly budget is used up.
    pub presence: bool,
}

/// One Nostr mailbox: a public relay and the per-relay policy state (pacing, budget, parking).
#[derive(Debug)]
pub(crate) struct NostrBox {
    pub id: MailboxId,
    pub url: String,
    /// `Closed`, `Opening` (waiting for the socket) or `Open` (socket up, subscribed or subscribing).
    pub state: MailboxState,
    pub changed_at: u64,
    pub failures: u32,
    pub next_open_at: u64,
    /// The relay is left out until this time (a refusal, forged events or a flood).
    pub parked_until: u64,
    pub park_level: u32,
    pub last_park_at: Option<u64>,
    /// The id of our one subscription on this relay (random per socket).
    pub sub_id: String,
    /// The epochs of the last `REQ`, to notice a rollover.
    pub sub_epochs: Vec<u32>,
    pub sub_sent_at: u64,
    /// The relay must answer the `REQ` (EOSE or CLOSED) before this time.
    pub eose_deadline: Option<u64>,
    pub outbox: VecDeque<Outgoing>,
    pub last_send_at: Option<u64>,
    pub hour_start: u64,
    pub hour_sent: u32,
    pub rx_start: u64,
    pub rx_count: u32,
    pub bad_start: u64,
    pub bad_count: u32,
    /// A reachability check is waiting for EOSE on this subscription id.
    pub probe_sub: Option<String>,
}

impl NostrBox {
    pub fn new(id: MailboxId, url: String) -> Self {
        Self {
            id,
            url,
            state: MailboxState::Closed,
            changed_at: 0,
            failures: 0,
            next_open_at: 0,
            parked_until: 0,
            park_level: 0,
            last_park_at: None,
            sub_id: String::new(),
            sub_epochs: Vec::new(),
            sub_sent_at: 0,
            eose_deadline: None,
            outbox: VecDeque::new(),
            last_send_at: None,
            hour_start: 0,
            hour_sent: 0,
            rx_start: 0,
            rx_count: 0,
            bad_start: 0,
            bad_count: 0,
            probe_sub: None,
        }
    }

    /// Socket up and the relay is in use (not a reachability check).
    pub fn is_open(&self) -> bool {
        self.state == MailboxState::Open && self.probe_sub.is_none()
    }

    pub fn reset_connection(&mut self) {
        self.state = MailboxState::Closed;
        self.sub_epochs.clear();
        self.eose_deadline = None;
        self.outbox.clear();
        self.last_send_at = None;
        self.probe_sub = None;
    }
}

/// The set: the relay mailbox and the Nostr mailboxes of the pairing.
#[derive(Debug)]
pub(crate) struct MailboxSet {
    pub relay: Mailbox,
    pub nostr: Vec<NostrBox>,
    next_id: MailboxId,
}

impl MailboxSet {
    pub fn new() -> Self {
        Self {
            relay: Mailbox::relay(),
            nostr: Vec::new(),
            next_id: RELAY_MAILBOX + 1,
        }
    }

    /// A Nostr mailbox for `url` (an existing one keeps its id and state).
    pub fn add_nostr(&mut self, url: &str) {
        if self.nostr.iter().any(|b| b.url == url) {
            return;
        }
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.nostr.push(NostrBox::new(id, url.to_owned()));
    }
}
