# Design

## Context

Sources: ADR-020 §3, ADR-019 (Rust is the only protocol source), AGENTS.md §5.1 (binding attributes never in `core`), `docs/ci-gates.md` notes (the `uniffi` feature pulls `getrandom` into the core graph today).

## Goals / Non-Goals

**Goals:** a binding-free core, generated types, visible API changes. **Non-Goals:** the Android module split (`split-android-modules`), the web client (`rebuild-web-client`).

## Decisions

- **UniFFI pinned exactly** and bumped in its own Renovate PR, because generated Kotlin and the runtime must match.
- **Snapshots are committed, generated bindings are not**; CI builds and generates twice and compares hashes to catch non-determinism. The second round cleans and recompiles `guardedsleep-core`, `-ffi` and `-wasm` (about 15 s on a warm cache, the dependencies stay cached), so macro expansion and metadata emission (uniffi, tsify, wasm-bindgen) are covered, not only the generators.
- **Discarded alternatives:** hand-written JNI (unsafe surface, double maintenance); Diplomat (smaller ecosystem for Kotlin); keeping `packages/protocol` (a second schema drifts from the core).

- **`packages/protocol` deletion moves to `add-rust-signaling-server`** (decided 2026-10-22): the Node signaling server is its last real consumer, so the package goes in the same change that deletes the server. `apps/mobile` only uses it in one test, which switches to the generated types.
- **cargo-public-api runs on a separately pinned nightly** (`tools/nightly/rust-toolchain.toml`) inside the stock `rust:1-bookworm` image, because rustdoc JSON needs nightly and the build image lacks `libssl-dev`. The pin lives in `mise.toml` `[vars]`, not `[tools]`, so `mise install` works without a host cargo.
- **tsify stays an optional core feature enabled only by `guardedsleep-wasm`**: wrapper types would duplicate the protocol types; the default core graph stays free of wasm-bindgen.
- **Binding-free core is enforced in three layers** (decided 2026-10-22): `deny.core.toml` bans `uniffi` on the all-features graph; `scripts/lib/core-bindings-check.sh` bans `uniffi*`, `wasm-bindgen`, `js-sys` and `web-sys` on the default-feature graph (`cargo tree -p guardedsleep-core`) and greps `crates/guardedsleep-core/src` for `uniffi::`, `#[uniffi` and `wasm_bindgen`. cargo-deny cannot do the default-feature check itself because it unifies features across the workspace, where `guardedsleep-wasm` enables the optional `tsify` feature (which legitimately pulls wasm-bindgen, js-sys and web-sys), so a ban on those three in the all-features run would always fail.

- **Core dependency count 48 on the all-features graph is justified** (decided 2026-10-22): the shipped default graph is unchanged (30 unique crates; the earlier 35 and 60 were line counts that included `(*)` repeats, so the guard now dedupes them). The count is taken on the all-features graph so the optional `tsify` feature is reviewed too; the 18 extra crates are tsify, tsify-macros, wasm-bindgen and their dependencies (gloo-utils, web-sys, js-sys, futures-*). Disabling tsify's default `json` feature was tried and rejected: tsify 0.5.8 does not compile without `json` or `js`, and `js` (serde-wasm-bindgen) maps `Option` to `undefined` instead of `null` in `core.d.ts`, which contradicts the serde_json wire format the façade passes (`api-check` showed the diff). The `json` feature stays.

## Risks / Trade-offs

- The core dependency count changes when `uniffi` leaves the core; re-bless and review the diff (ci-gates note).
