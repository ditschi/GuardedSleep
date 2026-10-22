# Project and app lifecycle (trust, updates, continuity)

> STATUS: PROPOSED -- companion to [concept review 2026-05](../reviews/concept-review-2026-05.md) (IMP-6) and [ADR-009](../../openspec/changes/archive/2026-10-21-adr-009-personas-and-failure-model/design.md).

## Why this document exists

GuardedSleep is an open-source, small-team project. Parents who adopt it for **daily** monitoring (Persona B in [personas.md](../concept/personas.md)) may depend on it for **years**. Unlike a hardware vendor, there is no implicit warranty, no forced recall path, and no guaranteed long-term binary availability in app stores.

This document states **honest expectations** and the **minimum continuity commitments** we document for users and contributors.

## What we commit to communicate

- **Update cadence:** When releases slow, store listings and in-app **About** should still state the last security-relevant dependency bump date (manual until automated).
- **Stale build warning (future implementation):** If the installed build is older than N months **and** the project has published newer security fixes, show a non-blocking banner: "A newer GuardedSleep release is available -- please update when convenient."
- **Abandonment / archive:** If the project is archived, the README must point to the last known-good APK hashes and source tag; users self-hosting signaling must receive migration notes in the same release.

## Saved connections and continuity

- **Export / import (IDEA Phase 2+):** Allow advanced users to export **non-secret** connection metadata (names, signaling URLs) and re-import after reinstall. **Secrets remain in keystore** -- export must never include raw PSK material.
- **Pairing redo:** If export is unavailable, users always retain the QR re-pair path ([connectivity-pairing.md](../features/connectivity-pairing.md)).

## Relationship to privacy policy

Operational lifecycle text belongs here; [privacy-policy.md](privacy-policy.md) remains the GDPR-facing document. Cross-link both when either changes.

## Discarded alternatives

| Approach                                  | Reason                                                                                                                                                       |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Mandatory cloud account for continuity    | Contradicts privacy-first positioning.                                                                                                                       |
| Silent auto-updates without release notes | Unacceptable for security-sensitive users; Release Please + changelog already decided ([ADR-006](../decisions/ADR-006-release-and-changelog-automation.md)). |
