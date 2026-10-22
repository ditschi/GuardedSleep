# Proposal

## Why

Binding attributes still live in the core (`uniffi` feature, `wasm.rs`), the web client's protocol types are hand-written in the legacy `packages/protocol`, and nothing stops a binding surface from changing unnoticed. ADR-020 §3 decides separate façade crates, generated TypeScript types and API-surface snapshots. Tier T0, wave 2 (group 1).

## What Changes

- New crates `guardedsleep-ffi` (UniFFI, exact pin `=0.32.x`, in-repo bindgen) and `guardedsleep-wasm` (wasm-bindgen + tsify); `core` loses every binding attribute.
- TypeScript types generated from the core into `packages/core-wasm`; `packages/protocol` is deleted.
- Committed API-surface snapshots (`cargo public-api` on `ffi` and `wasm`, the generated `.d.ts`); CI generates bindings twice and compares hashes.
- Conformance vectors, including negative ones, run through every binding (Rust, Kotlin, TypeScript).
- iOS check for `core`, `relay` and `ffi`; weekly Swift-bindings smoke test.
- Optional: move the invite payload codec from Kotlin into the core (backlog `IDEA`).

## Capabilities

### New Capabilities

### Modified Capabilities

- `dev-toolchain`: API snapshots, generate-twice check, vectors through every binding.

## Impact

`crates/guardedsleep-{core,ffi,wasm}`, `packages/core-wasm`, `packages/protocol` (deleted), Android `:core` module, `deny.core.toml` (count re-blessed with `--all-features`), `docs/ci-gates.md`.
