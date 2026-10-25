//! GuardedSleep shared core (ADR-015, ADR-017): wire protocol v3, QR pairing proof, noise gate and
//! the sans-IO `Attempt` / `Link` / `Session` state machines. No networking, threads or clocks:
//! platforms supply all I/O and a monotonic millisecond clock.

#![forbid(unsafe_code)]

pub mod attempt;
pub mod conformance;
// Envelope vector evaluation needs fixed-nonce sealing: never part of a release build.
#[cfg(any(test, feature = "conformance"))]
mod conformance_envelope;
// The Nostr cases sign with a caller-chosen `aux_rand`: never part of a release build either.
#[cfg(any(test, feature = "conformance"))]
mod conformance_nostr;
pub mod envelope;
pub mod invite;
pub mod link;
pub mod mailbox;
pub mod noise;
pub mod nostr;
pub mod pairing;
pub mod protocol;
pub mod session;
// Sealed-signaling test support: a PSK-holding peer for tests and the simulation; never in a release.
#[cfg(any(test, feature = "testing"))]
pub mod testing;
