# Vision and Goals

## Mission Statement

GuardedSleep aims to provide a secure, privacy-respecting, open-source baby monitor that works across all major platforms using devices people already own.

## Core Goals

> STATUS: DECIDED -- These are the foundational principles of the project.

### Cross-Platform

The application must work on Android and iOS phones and tablets. Desktop support (Windows, Linux, macOS) is a secondary goal. The same user experience should be available regardless of platform.

### Security and Privacy First

- All communication between devices must be end-to-end encrypted
- No audio, video, or metadata is sent to third-party servers
- The system must work without any cloud dependency
- Users must have full control over their data at all times

### Open Source and Community-Driven

- The full source code is publicly available and auditable
- Development decisions are documented transparently (see [decisions](../decisions/README.md))
- Community contributions are welcome and guided by clear documentation

### Usable by Non-Technical People

- Setup must be straightforward (ideally: install, scan QR code, done)
- Daily operation must be simple enough for a sleep-deprived parent at 3 AM
- The system should not require technical knowledge to deploy or maintain
- **Personas and failure expectations** are documented in [personas.md](personas.md), [use-cases.md](use-cases.md), and [failure-model.md](failure-model.md); binding decision: [ADR-009](../../openspec/changes/archive/2026-10-21-adr-009-personas-and-failure-model/design.md) (PROPOSED). Personas are **design and test lenses** (who we do not forget); they **do not** mean different app chrome or divergent primary flows -- the same build must work at home tonight and at a wedding tomorrow. **Engineering and copy priority** still stress Persona B (daily home) so edge cases (A/C) are not silently broken; **runtime behavior** is meant to be **context-aware** (network type, captive portal failure, metered data) per [connectivity-pairing.md](../features/connectivity-pairing.md).

## App name and store discovery (ASO)

> STATUS: **DECIDED**

**Product / marketing name:** **GuardedSleep – Privacy Baby Monitor**

- **Repository / technical identifiers:** The product is **GuardedSleep**. Workspace npm scope **`@guardedsleep`**, Docker images **`guardedsleep-*`**, and Android application ID **`org.guardedsleep.app`** align with that name. The git repository folder may still be named historically on some checkouts.
- **In-app and store-facing branding:** GuardedSleep, with subtitle/tagline _Privacy Baby Monitor_ where space allows (splash screen, store listings, about screen).

**Store listing:** The full string helps ASO (contains "Privacy", "Baby", "Monitor"). **Note:** Google Play app title length is often limited (~30 characters in many locales). If the full title does not fit, use a shortened **title** (e.g. `GuardedSleep Baby Monitor`) and put _Privacy-first, encrypted…_ in the **short description**, while keeping **GuardedSleep – Privacy Baby Monitor** as the canonical product name in the long description and on the website.

**Will users find the app when searching "baby monitor"?**

- Listing text should still repeat _baby monitor_, _encrypted_, _open source_ in the first lines of the description for search relevance.
- **F-Droid clients (via IzzyOnDroid at beta):** Summary line should include "baby monitor" and "privacy". The F-Droid main repository is `STATUS: DEFERRED` until a from-source build of libwebrtc and JNA is sized; make no public promise of F-Droid main ([ADR-021](../decisions/ADR-021-offline-lan-topologies-locales-and-distribution.md) §3). Wording: "F-Droid clients via IzzyOnDroid; F-Droid main repository later".

## Non-Goals

> STATUS: DECIDED -- Things we explicitly do NOT want to build.

- A commercial product with subscription fees
- A system that requires proprietary hardware
- A cloud-dependent service where audio/video passes through external servers
- A general-purpose surveillance tool -- this is specifically for child monitoring by parents/caregivers
- **Guaranteed alarm delivery when the parent receiver device is off, crashed, or out of battery** -- GuardedSleep follows the same contract as classic hardware baby monitors: the caregiver must notice the **missing receiver-side status** (see [failure-model.md](failure-model.md) and [ADR-009](../../openspec/changes/archive/2026-10-21-adr-009-personas-and-failure-model/design.md)). Mitigations are: persistent parent-side indicators, one-tap reconnect without touching the baby unit, and Phase 2 multi-parent failover -- not acoustic alarms on the baby unit and not cloud/SMS/VoIP alarm paths.
