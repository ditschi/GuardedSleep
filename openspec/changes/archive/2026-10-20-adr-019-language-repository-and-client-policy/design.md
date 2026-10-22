# ADR-019: Language, repository and client policy after leaving the single web app

- Status: DECIDED (owner, 2026-10-20)
- Date: 2026-10-20
- Deciders: Project owner; Technical Architect inventory of web-app-era decisions (2026-10-19)
- Supersedes: the language ("TypeScript everywhere"), repository-layout and settings-storage rows of [ADR-002](../2026-04-15-adr-002-client-platform-and-repository-structure/design.md); the "Go/Rust for signaling rejected" rationale in [sw-architecture.md](../../../../docs/technical/sw-architecture.md)
- Related: [ADR-015](../2026-10-21-adr-015-native-clients-with-shared-rust-core/design.md) (native clients on a shared Rust core, PROPOSED until its gate passes), [ADR-006](../2026-05-13-adr-006-release-and-changelog-automation/design.md), [ADR-008](../2026-05-13-adr-008-container-image-build-and-distribution/design.md), [ADR-018](../../../../docs/decisions/ADR-018-alpha-milestone-scope-and-trust-features.md)

> **Partially superseded (2026-10-20):** §2 signaling timing → [ADR-020](../../../../docs/decisions/ADR-020-toolchain-and-development-setup.md) §6 (Rust server now, not after the alpha); §4 pnpm/`justfile` as task host → [ADR-020](../../../../docs/decisions/ADR-020-toolchain-and-development-setup.md) §1 (`just` is the only entry point, mise pins tools); §5 rename `apps/mobile` → `apps/web` → [ADR-020](../../../../docs/decisions/ADR-020-toolchain-and-development-setup.md) §5 (rebuilt on Vite 8 + Svelte 5, `apps/mobile` frozen then deleted); §3 `@guardedsleep/protocol` "generated package" → [ADR-020](../../../../docs/decisions/ADR-020-toolchain-and-development-setup.md) §5 (types generated into the WASM package, `packages/protocol` deleted); the follow-up on pre-commit → lefthook ([ADR-020](../../../../docs/decisions/ADR-020-toolchain-and-development-setup.md) §8). Language-by-layer, permanent full web client, versioning and storage wording stand.

## Context

The project began as one SvelteKit app wrapped by Capacitor (ADR-002), and many rules followed from that: TypeScript everywhere, one pnpm workspace, the TypeScript protocol package as schema source, Capacitor storage APIs in the security guardrails, Vitest/Playwright as the only test stack. With native clients on a shared Rust core (ADR-015) those rules steer contributors and agents against the architecture. The owner asked (2026-10-19) to identify every such decision and question it: "if Rust is better, then Rust".

## Decision

1. **Language by layer.** Rust for the shared core and for tooling that shares protocol logic (e.g. the i18n generator `crates/i18n-gen`); Kotlin + Compose for Android; Swift + SwiftUI for iOS (after its gate); TypeScript/Svelte for the web client and simple repository tooling; no new plain JavaScript in app code.
2. **Signaling server:** moves to Rust (`axum` + the core's protocol types) **after the alpha** (superseded, see ADR-020 §6: now), proven by a conformance suite run against both servers; until then the Node server receives only protocol-v3 fixes. The deployment invariant (one HTTP port, TLS at the proxy) is unchanged.
3. **Protocol source of truth:** the Rust core's types. TypeScript types are generated from them; the hand-written `@guardedsleep/protocol` becomes a generated package (superseded, see ADR-020 §5: package deleted, types generated into the WASM package). Conformance vectors and traces stay as the regression net.
4. **One polyglot monorepo** (pnpm + Cargo workspace + Gradle) with a single entry point (refined by ADR-020 §1: pnpm is no longer the task host) (`justfile`: `just test`, `just lint`, `just build`).
5. **Naming and legacy:** `apps/mobile` is renamed `apps/web` (superseded, see ADR-020 §5: `apps/web` is rebuilt, `apps/mobile` deleted at parity) and the Capacitor shell (`apps/mobile/android`, Capacitor dependencies, hybrid E2E) is removed once ADR-015 is DECIDED (superseded, see ADR-020 waves: removed in wave 1 regardless of ADR-015 status); until then it is marked legacy and not extended.
6. **Versions:** one product version shared by the Android and web clients (same core and protocol); the core crate and the signaling server are versioned independently. The final application id is `org.guardedsleep.app`; alpha builds use `org.guardedsleep.app.alpha`.
7. **Web client: permanent full client in both roles.** A laptop or desktop browser using its internal microphone is a supported baby unit; a browser is also a supported parent unit. The web client runs the same WASM core. Honest limits are part of the product: the browser cannot read or set the device's silence state, cannot keep running when the device sleeps, and depends on the tab staying open — the app guides the user through a "keep this unit awake" setup and warns when it is not met.
8. **Storage wording:** pairing material is protected by property, not by API name: hardware-backed keys (Android Keystore/StrongBox, iOS Keychain); web clients keep the PSK in memory only.

## Options Considered

### Language policy

- **A: Rust wherever possible** (also UI tooling) — rewrites working tooling without user benefit.
- **B: Language by layer (CHOSEN)** — each language where it fits; logic shared through the core.
- **C: TypeScript default, Rust only for the core** — keeps a second hand-written protocol in the server. Rejected.

### Signaling server

- **A: Rust now** — blocks the alpha. Rejected for now.
- **B: Rust after the alpha with a parity suite (CHOSEN).**
- **C: Stay on Node** — permanent schema duplication. Rejected.

### Protocol source of truth

- **A: Rust types, TypeScript generated (CHOSEN).**
- **B: JSON Schema generating both** — extra layer, weaker validation. Rejected.
- **C: Both hand-written** — drift caught only after the fact. Rejected.

### Repository layout

- **A: One polyglot monorepo (CHOSEN).**
- **B: Split repositories** — cross-repo pinning for every protocol change. Rejected.

### Future role of the web client

- **A: Full client in both roles (CHOSEN, permanent).**
- **B: Parent-only viewer after the alpha** — recommended by the Technical Architect to avoid an unreliable baby unit; rejected by the owner because a laptop with its internal microphone is a real use case.
- **C: Drop the web client** — loses the interop harness and desktop story. Rejected.

## Consequences

### Positive

- Agents and contributors are no longer steered towards the Capacitor/TypeScript-only architecture.
- One protocol definition, one product version, one repository entry point.

### Negative

- Four languages to review; CI and the Dev Container must provide Rust, NDK, Gradle and Node.
- A browser baby unit can never match the native app's reliability; the UI must say so clearly.

### Risks

- Laptop baby units sleep (lid closed, OS idle sleep, night-time updates) — mitigated by the "keep awake" setup, Screen Wake Lock, battery/charging reporting and suspend detection (large clock jumps), and by the parent's normal escalation when contact is lost.
- The Rust server migration slips — the Node server stays functional for protocol v3 meanwhile.

## Follow-ups

- `STATUS: DECIDED` Update AGENTS.md (§1–§5, §7, §9, §10), the implementer/refactor-scout agents, `.cursor/rules`, Dev Container, pre-commit (superseded by lefthook, see ADR-020 §8) (rustfmt, clippy, UI-literal lint, i18n check), CI audit gates (`cargo audit`/`cargo deny`, Gradle dependency check) — `chore(docs): sync AGENTS.md`.
- `STATUS: DECIDED` Status notes on ADR-001 (PoC mechanism → ADR-015 gate), ADR-002 (superseded rows), ADR-004 (storage wording), ADR-006 (release components), ADR-007 (narrowed per ADR-016), ADR-008 (web image = full web client), ADR-010/012/013 (WebView and Capacitor references).
