# Proposal

## Why

ADR-020 §3, §4, §7 and §11 decide quality and security gates that are not built yet: design tokens with a contrast test, accessibility checks on goldens, a log canary, fuzzing and size budgets. Tier T0, wave 3.

## What Changes

- Design tokens: a DTCG-shaped `tokens.json` generated for Compose and CSS by the Rust generator, with a contrast test for the dark, light and dim baby-unit themes.
- Roborazzi with `roborazzi-accessibility-check` on 11 goldens and a baseline-free text-overflow check.
- Log canary: simulation and gate runs pair with a known PSK, and CI fails if it appears in any log in any encoding.
- Two nightly cargo-fuzz targets (signaling envelope decode, invite parse) and proptest properties for the pairing proof.
- Size budgets for the WASM bundle and the APK (the core `.so` budget exists).
- `secrecy`/`zeroize` for secret types; `SECURITY.md`; the AGPL §13 source link on server and web.

## Capabilities

### New Capabilities

### Modified Capabilities

- `dev-toolchain`: design-token contrast, accessibility goldens, log canary, fuzzing, size budgets.

## Impact

`crates/i18n-gen` (tokens), Android and web themes, Roborazzi tests, `crates/guardedsleep-sim`, `fuzz/`, CI nightly workflow, `SECURITY.md`, `docs/ci-gates.md`.
