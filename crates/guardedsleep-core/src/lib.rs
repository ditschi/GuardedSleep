//! GuardedSleep shared core (ADR-015, ADR-017): wire protocol v3, QR pairing proof, noise gate and
//! the sans-IO `Attempt` / `Link` / `Session` state machines. No networking, threads or clocks:
//! platforms supply all I/O and a monotonic millisecond clock.

#![forbid(unsafe_code)]

pub mod attempt;
pub mod conformance;
pub mod link;
pub mod noise;
pub mod pairing;
pub mod protocol;
pub mod session;
