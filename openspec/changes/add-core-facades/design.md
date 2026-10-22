# Design

## Context

Sources: ADR-020 §3, ADR-019 (Rust is the only protocol source), AGENTS.md §5.1 (binding attributes never in `core`), `docs/ci-gates.md` notes (the `uniffi` feature pulls `getrandom` into the core graph today).

## Goals / Non-Goals

**Goals:** a binding-free core, generated types, visible API changes. **Non-Goals:** the Android module split (`split-android-modules`), the web client (`rebuild-web-client`).

## Decisions

- **UniFFI pinned exactly** and bumped in its own Renovate PR, because generated Kotlin and the runtime must match.
- **Snapshots are committed, generated bindings are not**; CI generates twice and compares hashes to catch non-determinism.
- **Discarded alternatives:** hand-written JNI (unsafe surface, double maintenance); Diplomat (smaller ecosystem for Kotlin); keeping `packages/protocol` (a second schema drifts from the core).

## Risks / Trade-offs

- The core dependency count changes when `uniffi` leaves the core; re-bless and review the diff (ci-gates note).
