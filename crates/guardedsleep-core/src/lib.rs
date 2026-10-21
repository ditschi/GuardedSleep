//! GuardedSleep shared core (ADR-015, ADR-017): wire protocol v3, QR pairing proof, noise gate and
//! the sans-IO `Attempt` / `Link` / `Session` state machines. No networking, threads or clocks:
//! platforms supply all I/O and a monotonic millisecond clock.

// The generated UniFFI/wasm-bindgen glue needs `unsafe`; it moves to facade crates in wave 2
// (ADR-020); the workspace lint denies `unsafe_code` everywhere else. Without those features the crate
// is unsafe-free by construction.
#![cfg_attr(not(any(feature = "uniffi", feature = "wasm")), forbid(unsafe_code))]

pub mod attempt;
pub mod conformance;
pub mod link;
pub mod noise;
pub mod pairing;
pub mod protocol;
pub mod session;

#[cfg(feature = "uniffi")]
#[allow(unsafe_code)]
mod ffi;
#[cfg(feature = "wasm")]
#[allow(unsafe_code)]
mod wasm;

#[cfg(feature = "uniffi")]
uniffi::setup_scaffolding!();
