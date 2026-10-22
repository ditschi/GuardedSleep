# Proposal

## Why

The tooling had grown by accretion; a greenfield review by five roles and a Critical Reviewer defined one toolchain for the Rust core, Android, web and server. Migrated verbatim from ADR-020 (DECIDED 2026-10-20); `design.md` is the original ADR.

## What Changes

- Wave 1 (built): just, mise, lefthook, pnpm 11, cargo-deny, REUSE, Betterleaks, CI hardening, Renovate.
- Waves 2 and 3: façade crates, Rust server, WebRTC vendor, Android modules, web rebuild, i18n and tokens, quality and security gates; T1 and T2 items.

## Capabilities

### New Capabilities

### Modified Capabilities

Wave 1 lives in `dev-toolchain`, `i18n` and `distribution`; waves 2 and 3 are the prepared changes `add-rust-signaling-server`, `add-core-facades`, `swap-webrtc-sdk`, `split-android-modules`, `add-android-release`, `rebuild-web-client`, `expand-locales` and `add-quality-gates`; T1/T2 items are in `docs/backlog.md`.

## Impact

Archive-only.
