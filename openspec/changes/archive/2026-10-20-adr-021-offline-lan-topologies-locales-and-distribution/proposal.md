# Proposal

## Why

Offline use, the locale policy and the distribution channels were open after the toolchain review. Migrated verbatim from ADR-021 (DECIDED 2026-10-20); `design.md` is the original ADR.

## What Changes

- Offline LAN: Android ↔ Android listener and local PC mode, relay admission; HTTPS web ↔ phone rejected.
- About 20 AI-translated locales with critical-key completeness.
- Play and GitHub APK at alpha, IzzyOnDroid at beta, F-Droid main deferred.

## Capabilities

### New Capabilities

### Modified Capabilities

Outcome lives in `i18n` and `distribution`, and in the prepared changes `add-offline-lan`, `expand-locales` and `add-android-release` (migrated 2026-10-22).

## Impact

Archive-only.
