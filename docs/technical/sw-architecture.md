# Software Architecture

> **Partially superseded (2026-10-20):** this document describes the single-app SvelteKit + Capacitor + Node setup. The current architecture is the Rust core with native clients ([ADR-015](../decisions/ADR-015-native-clients-with-shared-rust-core.md)) and the polyglot toolchain of [ADR-020](../decisions/ADR-020-toolchain-and-development-setup.md). Superseded points are marked inline "(superseded, see ADR-0xx)"; the sections below stay as decision history and are not rewritten. Target state: web client `apps/web` on Vite 8 + Svelte 5 + vite-plugin-pwa (no SvelteKit); Rust axum signaling server on the `relay` crate; protocol types generated from the Rust core (`packages/protocol` deleted); `just` as the only entry point with tools pinned by mise; pnpm 11 without corepack; lefthook instead of Husky/pre-commit; native amd64/arm64 image builds (no QEMU).

## Technology Stack

> STATUS: **DECIDED** -- Accepted by user.

| Area                  | Recommendation                                                                  | Priority   | Alternatives                |
| --------------------- | ------------------------------------------------------------------------------- | ---------- | --------------------------- |
| **Web Framework**     | Svelte 5 + SvelteKit (superseded, ADR-020 §5: Vite 8 + Svelte 5)                | IMPORTANT  | React, Vue 3, Solid.js      |
| **UI Framework**      | Tailwind CSS + shadcn-svelte                                                    | IMPORTANT  | Skeleton UI, DaisyUI, Ionic |
| **Project Structure** | Monorepo (pnpm workspaces) (refined, ADR-020 §1: Cargo + Gradle + pnpm, `just`) | CRITICAL   | Single package              |
| **Settings Storage**  | Capacitor Preferences + Secure Storage (Keystore) (superseded, ADR-019 §8)      | CRITICAL   | localStorage, SQLite        |
| **Signaling Server**  | Node.js + ws + TypeScript (superseded, ADR-020 §6: Rust axum)                   | SUGGESTION | Go, Deno, Bun               |
| **Package Manager**   | pnpm (pnpm 11 via mise, no corepack, ADR-020 §5)                                | SUGGESTION | npm, yarn                   |
| **Build Tool**        | Vite (via SvelteKit) (superseded: plain Vite 8, ADR-020 §5)                     | implicit   | --                          |
| **Language**          | TypeScript (everywhere) (superseded, ADR-019 §1)                                | CRITICAL   | --                          |
| **Linting**           | ESLint + Prettier + svelte-check (superseded, ADR-020 §5: no ESLint)            | SUGGESTION | Biome                       |

## 1. Web Framework

> STATUS: **DECIDED**

### Options

- [ ] **React**
  - Pros: Largest ecosystem, most Capacitor examples (Ionic React), huge community, most WebRTC tutorials, industry standard JSX
  - Cons: Largest bundle (~40-45 kB gzipped), Virtual DOM overhead, boilerplate for state management (useEffect chains for real-time data), needs additional libraries for reactivity (Zustand/Jotai), re-render management for frequent WebRTC stat updates

- [x] **Svelte 5 / SvelteKit (RECOMMENDED)**
  - Pros: Compiler-based (no runtime overhead), smallest bundle (~2-5 kB framework overhead), built-in reactivity (ideal for WebRTC stats, audio level visualization), minimal boilerplate, excellent code readability, Svelte 5 with Runes is stable (Oct 2024), SvelteKit offers SSR/SSG/SPA modes, works with Capacitor (standard Vite project), growing community
  - Cons: Smaller ecosystem than React, fewer Capacitor-specific tutorials, fewer contributors know Svelte (but easy to learn)

- [ ] **Vue 3**
  - Pros: Good middle ground, clean template syntax, Composition API is reactive, good Capacitor integration (Ionic Vue), solid community
  - Cons: Template syntax divides opinions, larger bundle than Svelte (~33 kB gzipped), Vue 3 migration fragmented community

- [ ] **Solid.js**
  - Pros: Smallest runtime (~7 kB), React-like JSX, true reactivity without Virtual DOM, excellent performance
  - Cons: Smallest community, almost no Capacitor examples, risk for contributor access, too risky for a community project

### Recommendation: Svelte 5 / SvelteKit

**Key reasons:**

1. **Reactivity is the core argument.** The app has high-frequency state updates: WebRTC stats, audio RMS levels, connection status, battery level. Svelte's built-in reactivity (Runes: `$state`, `$derived`, `$effect`) maps this directly -- no useEffect chains or external state libraries.
2. **Bundle size.** For a Capacitor app on potentially older Android devices (repurposed phones as baby unit), every kB matters.
3. **Code readability.** A Svelte component is closer to "normal HTML/CSS/JS" than any other framework, lowering the contributor barrier.
4. **Stability.** Svelte 5 stable since Oct 2024, SvelteKit since Dec 2022. Rich Harris works at Vercel -- long-term support is secured.
5. **Risk mitigation.** Core logic (WebRTC, Audio, Crypto) is framework-agnostic in pure TypeScript. If contributor issues arise, the framework-specific parts are small (~10-20 screens).

## 2. UI Component Framework

> STATUS: **DECIDED**

### Options

- [ ] **Tailwind CSS only (custom components)**: Maximum control, no dependency bloat. Cons: every component must be built from scratch, accessibility must be manually implemented.
- [x] **Tailwind CSS + shadcn-svelte (RECOMMENDED)**: Svelte-native port of shadcn/ui. Copy-paste architecture (components live in your code, not external dependency). Radix-based accessibility (ARIA, keyboard navigation). Dark mode native. Full control.
- [ ] **DaisyUI**: Framework-agnostic Tailwind plugin. Many themes. Cons: predefined aesthetics, less control, weaker accessibility.
- [ ] **Skeleton UI**: Svelte-native, good design system. Cons: stronger coupling, less community momentum.
- [ ] **Ionic Framework**: Built for Capacitor. Cons: large bundle, hard to customize, Svelte support community-maintained.

### Recommendation: Tailwind CSS + shadcn-svelte

**Key reasons:**

1. **Ownership.** Components are copied into your project, not installed as npm dependency. Full control for Night Mode UX and custom Volume Bars.
2. **Accessibility built-in.** Built on Bits UI (Svelte port of Radix) -- ARIA and keyboard handling are standard.
3. **Dark mode.** Tailwind `dark:` + CSS variables enables multiple themes: default dark, ultra-dark (OLED black for baby screen), night-red (red light doesn't disturb sleep).
4. **Custom components coexist.** Specialized UI (Volume Bars, Pulsing Alert, QR Scanner, Listen Button with Slide-to-Lock) are custom Tailwind. shadcn-svelte provides standard components (Buttons, Dialogs, Settings, Toggles).

## 3. Architecture Layers

```
┌─────────────────────────────────────────────────┐
│                   UI Layer                       │
│         Svelte Components + shadcn-svelte        │
│              (routes/, components/)               │
├─────────────────────────────────────────────────┤
│                State Layer                       │
│         Svelte Stores ($state, $derived)         │
│              (stores/)                           │
├─────────────────────────────────────────────────┤
│              Platform Layer                      │
│    Capacitor Plugins ↔ Web API Fallback          │
│         (platform/)                              │
├─────────────────────────────────────────────────┤
│               Core Layer                         │
│     Pure TypeScript, no framework imports        │
│   WebRTC │ Audio │ Crypto │ Protocol │ Discovery │
│              (core/)                             │
└─────────────────────────────────────────────────┘
```

Each layer may only access layers below it. Core has zero dependencies on Svelte, Capacitor, or DOM APIs.

## 4. Project Structure

```
guardedsleep/
├── apps/
│   ├── mobile/                    # Capacitor App (SvelteKit)
│   │   ├── src/
│   │   │   ├── lib/
│   │   │   │   ├── core/          # Framework-agnostic core logic (pure TS)
│   │   │   │   │   ├── webrtc/    # Connection management
│   │   │   │   │   ├── audio/     # Audio capture, RMS analysis, thresholds
│   │   │   │   │   ├── crypto/    # Encryption, key management, DTLS fingerprint
│   │   │   │   │   ├── protocol/  # DataChannel message format, heartbeat
│   │   │   │   │   └── discovery/ # Local discovery (mDNS/UDP), IP management
│   │   │   │   ├── platform/      # Capacitor / browser abstraction
│   │   │   │   │   ├── storage.ts
│   │   │   │   │   ├── background.ts
│   │   │   │   │   ├── notifications.ts
│   │   │   │   │   ├── battery.ts
│   │   │   │   │   ├── network.ts
│   │   │   │   │   └── haptics.ts
│   │   │   │   ├── stores/        # Svelte state management
│   │   │   │   ├── components/    # Reusable UI components
│   │   │   │   │   ├── ui/        # shadcn-svelte base components
│   │   │   │   │   ├── audio/     # VolumeBar, NoiseIndicator
│   │   │   │   │   ├── connection/# StatusBadge, QualityIndicator
│   │   │   │   │   └── pairing/   # QRDisplay, QRScanner
│   │   │   │   └── utils/
│   │   │   ├── routes/            # SvelteKit pages
│   │   │   │   ├── +layout.svelte
│   │   │   │   ├── +page.svelte   # Home (mode selection / saved connections)
│   │   │   │   ├── baby/          # Baby unit screen
│   │   │   │   ├── parent/        # Parent unit screen
│   │   │   │   ├── pairing/       # QR pairing flow
│   │   │   │   └── settings/      # Settings
│   │   │   ├── app.html
│   │   │   └── app.css            # Tailwind + theme variables
│   │   ├── android/               # Capacitor Android project (generated)
│   │   │   └── app/build.gradle   # Build flavors: playStore / foss
│   │   ├── ios/                   # Capacitor iOS project (Phase 2)
│   │   ├── capacitor.config.ts
│   │   ├── svelte.config.js
│   │   ├── vite.config.ts
│   │   └── tailwind.config.ts
│   │
│   └── signaling-server/          # Standalone deployable
│       ├── src/
│       │   ├── server.ts
│       │   ├── room.ts
│       │   ├── auth.ts
│       │   └── rate-limit.ts
│       ├── Dockerfile
│       ├── docker-compose.yml
│       └── package.json
│
├── docs/                          # Existing documentation
├── .github/workflows/             # CI/CD
├── pnpm-workspace.yaml
└── README.md
```

### Design Decisions

- **`lib/core/` is framework-agnostic.** WebRTC, audio analysis, crypto, and protocol have zero Svelte imports. Enables: unit tests without UI framework, potential framework switch, clear separation of concerns.
- **`lib/platform/` abstracts Capacitor vs. browser (Capacitor part superseded, see ADR-015/ADR-020).** Same interface, different implementations. PWA falls back to Web APIs.
- **`lib/stores/` bridges core to Svelte.** The only connection between framework-agnostic core and UI. Thin and replaceable.
- **Monorepo with pnpm workspaces.** App and signaling server share TypeScript types for the signaling protocol. Unified tooling (ESLint, Prettier). (Superseded, see ADR-020 §1 and §5: native workspaces per ecosystem, protocol types generated from Rust, Prettier + svelte-check, no ESLint.)
- **Build flavors** in `android/app/build.gradle` (`playStore` / `foss`). Web code uses `import.meta.env.VITE_FLAVOR` for conditional features.

## 5. Settings Storage

> STATUS: **DECIDED**

### Decided Architecture: Two-Layer Approach

| Data Type                        | Storage                                                  | Reason                                       |
| -------------------------------- | -------------------------------------------------------- | -------------------------------------------- |
| UI preferences (theme, language) | Capacitor Preferences                                    | No security concern                          |
| Audio thresholds, intervals      | Capacitor Preferences                                    | Configuration, not sensitive                 |
| PSK (Pre-Shared Key)             | Android Keystore (TEE) via Secure Storage                | **CRITICAL**: Access to encrypted connection |
| DTLS fingerprints                | Android Keystore (TEE) via Secure Storage                | Identifies trusted devices                   |
| Saved device IPs                 | Encrypted SharedPreferences (AES-256, key from Keystore) | Less sensitive but encrypted                 |
| Signaling server URL             | Capacitor Preferences                                    | Configuration                                |

Abstraction layer in `lib/platform/storage.ts` hides the two-layer split from the rest of the app.

## 6. Signaling Server

> STATUS: **DECIDED**

**Recommendation: Node.js + ws + TypeScript** (superseded, see ADR-020 §6: Rust axum on the sans-IO `relay` crate, now; the "Go/Rust advantages are irrelevant" reasoning below no longer holds because the validator would duplicate core logic)

Key reasons:

1. Same language as app (TypeScript) -- one mental model, one toolchain
2. Shared types in monorepo for signaling protocol (no drift)
3. Contributor access -- same skills as web app
4. Docker image with `node:22-alpine` ~50 MB (size is irrelevant for a deployed-once server)
5. The 300-400 LOC make Go/Rust advantages irrelevant at this scale

## 7. Protocol Versioning

> STATUS: IDEA -- Raised by Critical Reviewer

DataChannel handshake must include a protocol version number. On connection: minimum version check. Prevents issues when one device updates the app and the other doesn't.

## 8. App-Lock (Biometrics / PIN)

> STATUS: **DEFERRED** (Phase 2) -- See [ADR-001](../decisions/ADR-001-mvp-scope-poc-gate-and-authentication.md). **MVP:** no in-app PIN/biometrics; users rely on **OS screen lock**.

Future implementation (not MVP): optional App-Lock before saved connections, BiometricPrompt, panic "delete all keys".

**MVP:** "Delete all connections" in settings remains useful without biometrics gate.

## Discarded Alternatives

| Option                      | Reason                                                                         |
| --------------------------- | ------------------------------------------------------------------------------ |
| Plain localStorage for keys | No encryption, PSK in plaintext = security model compromised                   |
| SQLite for settings         | Massive overkill for key-value settings + a few pairing keys                   |
| Deno/Bun for signaling      | Smaller community, less production experience, deployment docs thinner         |
| Go/Rust for signaling       | Different language = halves contributor base, advantages irrelevant at 300 LOC |
| Ionic Framework             | Large bundle, hard to customize, Svelte support community-maintained only      |
