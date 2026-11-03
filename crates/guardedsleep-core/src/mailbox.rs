//! The mailbox set of a `Link` (`alpha-connection-model`, design section 1): the rendezvous
//! channels through which a pairing exchanges signaling. The core decides which mailbox is open
//! and speaks each kind's wire protocol; the platform only owns one socket per mailbox id, opens
//! it when the core emits `open_mailbox`, closes it on `close_mailbox`, and feeds
//! `mailbox_opened`, `mailbox_closed` and `mailbox_frame` back. A platform never reconnects, fails
//! over or retries a mailbox on its own.
//!
//! The set holds the `relay` mailbox (our signaling server, JSON text frames of the relay protocol,
//! id [`RELAY_MAILBOX`]). The `lan` kind joins with `add-offline-lan`.

use serde::{Deserialize, Serialize};

use crate::protocol::Member;

/// Identifies one mailbox of the set for the lifetime of a `Link`. A mailbox keeps its id across
/// closing and reopening; the platform's socket for it does not (late events of a closed socket
/// are the platform's to discard).
pub type MailboxId = u32;

/// The relay mailbox: our signaling server.
pub const RELAY_MAILBOX: MailboxId = 0;

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

/// The set: today the relay mailbox only (task 8.7 of `alpha-connection-model` adds more relay
/// mailboxes, `add-offline-lan` the LAN one).
#[derive(Debug)]
pub(crate) struct MailboxSet {
    pub relay: Mailbox,
}

impl MailboxSet {
    pub fn new() -> Self {
        Self {
            relay: Mailbox::relay(),
        }
    }
}
