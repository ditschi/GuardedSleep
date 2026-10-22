# Design

## Context

Sources: ADR-020 §3, ADR-019 (Rust is the only protocol source), AGENTS.md §5.1 (binding attributes never in `core`), `docs/ci-gates.md` notes (the `uniffi` feature pulls `getrandom` into the core graph today).

## Goals / Non-Goals

**Goals:** a binding-free core, generated types, visible API changes. **Non-Goals:** the Android module split (`split-android-modules`), the web client (`rebuild-web-client`).

## Decisions

- **UniFFI pinned exactly** and bumped in its own Renovate PR, because generated Kotlin and the runtime must match.
- **Snapshots are committed, generated bindings are not**; CI generates twice and compares hashes to catch non-determinism.
- **Discarded alternatives:** hand-written JNI (unsafe surface, double maintenance); Diplomat (smaller ecosystem for Kotlin); keeping `packages/protocol` (a second schema drifts from the core).

- **`packages/protocol` deletion moves to `add-rust-signaling-server`** (decided 2026-10-22): the Node signaling server is its last real consumer, so the package goes in the same change that deletes the server. `apps/mobile` only uses it in one test, which switches to the generated types.
- **cargo-public-api runs on a separately pinned nightly** (`tools/nightly/rust-toolchain.toml`) inside the stock `rust:1-bookworm` image, because rustdoc JSON needs nightly and the build image lacks `libssl-dev`. The pin lives in `mise.toml` `[vars]`, not `[tools]`, so `mise install` works without a host cargo.
- **tsify stays an optional core feature enabled only by `guardedsleep-wasm`**: wrapper types would duplicate the protocol types; the default core graph stays free of wasm-bindgen.

## Risks / Trade-offs

- The core dependency count changes when `uniffi` leaves the core; re-bless and review the diff (ci-gates note).
