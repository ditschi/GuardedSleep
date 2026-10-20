# ADR-002: Client Platform, UI Stack, and Repository Structure

- Status: DECIDED — partly SUPERSEDED: language, repository-layout and settings-storage rows by [ADR-019](ADR-019-language-repository-and-client-policy.md) (2026-10-20); client platform under supersession by [ADR-015](ADR-015-native-clients-with-shared-rust-core.md) (PROPOSED). Do not extend the Capacitor path.
- Date: 2026-04-15
- Deciders: Technical direction + user acceptance of stack

> **Further superseded (2026-10-20):** the web framework row (SvelteKit → Vite + Svelte 5 + vite-plugin-pwa, `apps/web`), the package-manager/tooling rows and the signaling-server row (Node `ws` → Rust axum, now) → [ADR-020](ADR-020-toolchain-and-development-setup.md) §5–§6. The Capacitor shell and Capacitor Preferences are legacy; offline-LAN listener → [ADR-021](ADR-021-offline-lan-topologies-locales-and-distribution.md).

## Context

We need a maintainable, contributor-friendly stack for **GuardedSleep – Privacy Baby Monitor** (this monorepo; workspace packages under `@guardedsleep/*`), Android-first MVP, with a path to iOS later.

## Options Considered

- **Web framework:** React vs Vue vs Svelte vs Solid.
- **UI:** Tailwind-only vs component library (shadcn-svelte, Skeleton, Ionic).
- **Storage:** Plain Preferences vs split Preferences + Secure Storage for keys.
- **Signaling server:** Node + ws vs Go vs Deno.
- **Repo layout:** Single package vs monorepo (app + signaling server).

## Decision

| Area             | Choice                                                                                                          |
| ---------------- | --------------------------------------------------------------------------------------------------------------- |
| Web framework    | **Svelte 5 + SvelteKit** (Vite) (superseded, see ADR-020 §5: Vite 8 + Svelte 5, no SvelteKit)                   |
| UI               | **Tailwind CSS + shadcn-svelte**                                                                                |
| Language         | **TypeScript** throughout                                                                                       |
| Mobile shell     | **Capacitor** (Android MVP) (superseded, see ADR-015/ADR-020)                                                   |
| Package manager  | **pnpm**                                                                                                        |
| Layout           | **Monorepo**: `apps/mobile` + `apps/signaling-server` (names may vary)                                          |
| Settings         | **Capacitor Preferences** for non-secrets; **Secure Storage / Keystore** for PSK and sensitive pairing material |
| Signaling server | **Node.js + `ws` + TypeScript**, Dockerized (superseded, see ADR-020 §6: Rust axum)                             |

**Architecture:** `lib/core/` = framework-agnostic TypeScript (WebRTC, audio, crypto, protocol); `lib/platform/` = Capacitor/browser; UI = Svelte.

Details: see [sw-architecture.md](../technical/sw-architecture.md).

## Consequences

### Positive

- Small client bundle; good fit for real-time UI updates.
- One language for app and signaling server; shared protocol types possible.

### Negative

- Smaller Svelte contributor pool than React (mitigated by thin UI layer).

### Risks

- SvelteKit SSR features mostly unused in Capacitor SPA mode -- use SPA adapter consciously.

## Discarded alternatives

- Pure PWA for baby unit -- iOS background audio blocker (see ADR on platform).
- React -- larger bundle; not chosen.
- Ionic full UI -- heavier; not chosen for MVP styling goals.
