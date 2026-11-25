//! Typed indices of the simulation, so an actor, a socket and a peer connection cannot be mixed up.
//!
//! The public fields of [`Actor`](super::Actor) and [`Pc`](super::Pc) keep plain numbers (the
//! checks and `raw.rs` read them); the conversion happens at the accessors of [`World`](super::World).

/// Index of an actor in `World::actors`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ActorIx(usize);

/// Id of a simulated socket (a client end of a mailbox connection).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SockIx(usize);

/// Handle of a peer connection within one actor (the core's attempt number).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PcHandle(u32);

impl ActorIx {
    pub fn index(self) -> usize {
        self.0
    }
}

impl SockIx {
    pub fn index(self) -> usize {
        self.0
    }
}

impl PcHandle {
    pub fn raw(self) -> u32 {
        self.0
    }
}

impl From<usize> for ActorIx {
    fn from(ix: usize) -> Self {
        ActorIx(ix)
    }
}

impl From<usize> for SockIx {
    fn from(ix: usize) -> Self {
        SockIx(ix)
    }
}

impl From<u32> for PcHandle {
    fn from(handle: u32) -> Self {
        PcHandle(handle)
    }
}

/// One end of a simulated peer connection: which actor, which of its connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PcRef {
    pub actor: ActorIx,
    pub handle: PcHandle,
}

impl PcRef {
    pub fn new(actor: ActorIx, handle: PcHandle) -> Self {
        PcRef { actor, handle }
    }

    /// The pair as the public `Pc::peer` field stores it.
    pub fn raw(self) -> (usize, u32) {
        (self.actor.0, self.handle.0)
    }
}

impl From<(usize, u32)> for PcRef {
    fn from((actor, handle): (usize, u32)) -> Self {
        PcRef::new(ActorIx(actor), PcHandle(handle))
    }
}
