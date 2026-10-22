# Feature: Desktop client (installable)

> STATUS: IDEA — Optional distribution path for users who prefer a normal desktop app over installing a PWA or using only mobile. Not part of MVP.

## Overview

Ship the same web UI (SvelteKit static build; superseded, see [ADR-020](../../openspec/changes/archive/2026-10-20-adr-020-toolchain-and-development-setup/design.md) §5: Vite 8 + Svelte 5 static build) inside a **native desktop shell** so users get a familiar **installer** (e.g. `.exe` on Windows, `.dmg` on macOS, `.AppImage`/`.deb` on Linux) instead of “Add to Home Screen” / browser install flows.

## Motivation

- Some users find PWA installation confusing or do not distinguish browser vs. app.
- A dedicated window can feel clearer than a browser tab for a “parent station” on a PC.

## Technical direction (non-binding)

- **Capacitor** (legacy, see ADR-015/ADR-020) targets **Android, iOS, and web**; it is **not** the primary path for Windows/macOS/Linux desktop binaries.
- Typical options: **Tauri** (smaller artifacts, system WebView) or **Electron** (heavier, widely used). Both wrap the existing `adapter-static` output.
- **Local PC mode (DECIDED, target beta, [ADR-021](../../openspec/changes/archive/2026-10-20-adr-021-offline-lan-topologies-locales-and-distribution/design.md) §1):** the GuardedSleep server binary can serve the static web UI on `http://localhost` and signaling on the LAN, so a laptop works as baby or parent unit without internet and without certificate warnings. This covers the offline need that a native desktop shell was partly meant for; the installable desktop client itself stays an `IDEA`.
- **Signaling server** remains deployed **via Docker** for operators; the desktop app is **client-only**, same as PWA/APK (signaling URL configurable in-app per product direction).

## Requirements (draft)

- `[STATUS: IDEA]` Parity with parent (and optionally baby) flows that make sense on desktop: pairing, monitoring, settings including signaling URL.
- `[STATUS: IDEA]` Auto-update strategy (in-app or OS package manager) — TBD.
- `[STATUS: IDEA]` Code signing (Windows/macOS) for trustworthy installs — TBD.

## Dependencies

- Stable web build and deployment story (PWA/static hosting).
- Clarify which roles (parent-only vs. baby) are in scope for desktop.
