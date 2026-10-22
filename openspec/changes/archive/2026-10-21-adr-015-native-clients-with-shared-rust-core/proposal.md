# Proposal

## Why

The WebView/Capacitor client could not guarantee background audio, alarms and link recovery. Migrated verbatim from ADR-015 (proposed 2026-10-19, decided by the owner 2026-10-21 after the PoC gate AC-1…AC-4 passed); `design.md` is the original ADR.

## What Changes

- Native Android (Kotlin + Compose) and a web client on one sans-IO Rust core with UniFFI and wasm-bindgen façades; safety logic lives once in the core.
- A native PoC gate on the emulator.

## Capabilities

### New Capabilities

### Modified Capabilities

Archive-only; the layer rules live in AGENTS.md §5.1, the behaviour in the capability specs (migrated 2026-10-21/22).

## Impact

Archive-only.
