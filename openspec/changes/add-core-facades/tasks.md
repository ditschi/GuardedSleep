# Tasks

Coarse tasks; refine them from the wave-2 plan before `/opsx:apply`.

- [ ] 1.1 Create `guardedsleep-ffi` and move all UniFFI attributes out of `core`
- [ ] 1.2 Create `guardedsleep-wasm` with tsify types; generate TS into `packages/core-wasm`; delete `packages/protocol`
- [ ] 1.3 Commit `cargo public-api` and `.d.ts` snapshots; add the generate-twice hash check
- [ ] 1.4 Run conformance vectors (incl. negative) through Kotlin and TypeScript
- [ ] 1.5 iOS check for `core`, `relay`, `ffi`; weekly Swift smoke test
- [ ] 1.6 Re-bless the core dependency count with `--all-features`; `docs/ci-gates.md` rows; `just ci` green
