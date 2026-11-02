//! Sans-IO signaling relay (ADR-021 section 1, ADR-020): rooms, peer caps, admission, size and
//! rate limits, liveness. The platform owns the sockets (axum in `guardedsleep-server`, Kotlin on
//! Android) and the clock; it feeds connection events and frames in and carries out the returned
//! [`RelayEffect`]s. No networking, threads, clocks or OS randomness live here.
//!
//! Driving contract:
//! - Call [`Relay::on_connect`] for every accepted socket, [`Relay::on_frame`] for every inbound
//!   text or binary message, [`Relay::on_pong`] for every WebSocket pong, [`Relay::on_close`] when
//!   the socket is gone, and [`Relay::on_tick`] at least every second.
//! - Perform effects in order. A `Send` before a `Close` for the same connection must be flushed
//!   before the socket is closed. The relay forgets a connection as soon as it emits `Close` for
//!   it, so a later `on_close` for that connection is a no-op.
//! - `now` is a monotonic millisecond clock supplied by the caller.

#![forbid(unsafe_code)]
// Untrusted input is parsed here: return errors instead of panicking (AGENTS.md section 5.3).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

mod limiter;
mod relay;

pub use limiter::RateLimiter;
pub use relay::{
    Admission, ClientKey, CloseReason, ConnId, DeterministicEntropy, Entropy, LogEvent, Millis,
    Relay, RelayConfig, RelayEffect,
};
