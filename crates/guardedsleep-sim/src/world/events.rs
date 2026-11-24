//! The events of the queue. `Timeline` steps stay at the top; the rest is grouped by who handles
//! it, one dispatcher each.

use super::*;

#[derive(Debug, Clone)]
pub(super) enum Ev {
    Timeline(usize),
    Actor(ActorEv),
    Relay(RelayEv),
    Peer(PeerEv),
}

/// Events of one actor's process: its pacing, injected inputs and lifecycle.
#[derive(Debug, Clone)]
pub(super) enum ActorEv {
    Tick { actor: ActorIx, chain: u32 },
    Input { actor: ActorIx, input: LinkInput },
    PathBack { actor: ActorIx },
    Restart { actor: ActorIx },
    RawTick { actor: ActorIx },
    RawAct { actor: ActorIx },
}

/// Events of the simulated signaling relay and the sockets towards it.
#[derive(Debug, Clone)]
pub(super) enum RelayEv {
    Accept {
        sock: SockIx,
    },
    SigToServer {
        sock: SockIx,
        frame: Frame,
    },
    SigToClient {
        actor: ActorIx,
        sock: SockIx,
        msg: ToClient,
    },
    Pong {
        sock: SockIx,
    },
    Sweep {
        r#gen: u32,
        srv: usize,
    },
    ServerClose {
        sock: SockIx,
    },
    /// Half-TTL renewal of the ICE servers of the room `key` on relay `srv`.
    IceRenew {
        srv: usize,
        key: RoomKey,
    },
}

/// Which local description a connection produces.
#[derive(Debug, Clone, Copy)]
pub(super) enum SdpKind {
    Offer,
    Answer,
}

impl SdpKind {
    /// The `type` the core expects in `AttemptLocalDescription`.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            SdpKind::Offer => "offer",
            SdpKind::Answer => "answer",
        }
    }
}

/// Events of the simulated media layer.
#[derive(Debug, Clone)]
pub(super) enum PeerEv {
    LocalDesc { pc: PcRef, kind: SdpKind },
    PcConnect { ends: [PcRef; 2], since: Time },
    Data { to: PcRef, text: String },
    DataClosed { to: PcRef },
    Watch,
}

impl From<ActorEv> for Ev {
    fn from(e: ActorEv) -> Self {
        Ev::Actor(e)
    }
}

impl From<RelayEv> for Ev {
    fn from(e: RelayEv) -> Self {
        Ev::Relay(e)
    }
}

impl From<PeerEv> for Ev {
    fn from(e: PeerEv) -> Self {
        Ev::Peer(e)
    }
}

impl Ev {
    /// The event after a wall-clock jump of `ms`: its delay to the queue key, and itself with any
    /// stored start time moved along. Timeline steps keep their place.
    pub(super) fn shifted(self, ms: u64) -> (u64, Ev) {
        match self {
            Ev::Timeline(_) => (0, self),
            Ev::Peer(PeerEv::PcConnect { ends, since }) => {
                let since = since + ms;
                (ms, PeerEv::PcConnect { ends, since }.into())
            }
            other => (ms, other),
        }
    }
}
