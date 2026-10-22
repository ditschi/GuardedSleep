# Tasks

Refined 2026-10-22 from the wave-2 plan. Items 1.1, 1.3 (snapshots), 1.4 and 1.6 were implemented as W2-A (commits 1b830803, 472a3ad8, 21831967).

## 1. Façade crates

- [x] 1.1 Create `guardedsleep-ffi` and move all UniFFI attributes out of `core` (`#[uniffi::remote]` for the boundary types; namespace stays `guardedsleep_core`, so Kotlin needs no source change)
- [x] 1.2 Create `guardedsleep-wasm` with tsify types; generate TS into `packages/core-wasm` (`types.ts` re-exports the generated types)
- [x] 1.3 Lint fails when a UniFFI or wasm-bindgen attribute or dependency appears in `guardedsleep-core` (cargo-deny ban in `deny.core.toml` plus a source check in `just lint`)

## 2. Snapshots and determinism

- [x] 2.1 Commit `cargo public-api` snapshots for `ffi` and `wasm` and the generated `.d.ts`; `just lint` fails on a diff (`scripts/lib/api-snapshot.sh`, pinned nightly in `tools/nightly/`)
- [x] 2.2 Generate-twice check: build the Kotlin bindings and the wasm package twice, compare hashes, fail on a difference (`just lint`, CI)

## 3. Bindings and platforms

- [x] 3.1 Conformance vectors (61, of which 16 negative) and traces run through Rust, Kotlin (`ConformanceTest.kt`) and TypeScript (`conformance.test.mjs`)
- [x] 3.2 iOS check for `core` and `ffi` in `just lint` (`relay` is added by `add-rust-signaling-server`)
- [x] 3.3 Weekly CI job: generate the Swift bindings from the ffi library on Linux and fail if generation fails; upload them as an artifact

## 4. Close

- [x] 4.1 Core dependency count re-blessed with `--all-features` (48 unique crates; the guard now dedupes `(*)` repeats; trimming tsify's `json` feature was tried and rejected, see design.md)
- [x] 4.2 `docs/ci-gates.md` rows for the API snapshot, generate-twice, binding-free core and Swift smoke gates; `just ci` green
