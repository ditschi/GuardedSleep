# Proposal

## Why

Self-hosters need multi-architecture images with a predictable tag strategy. Migrated verbatim from ADR-008 (DECIDED 2026-05-13, amended by ADR-020: native runners, no QEMU); `design.md` is the original ADR.

## What Changes

- GHCR images `guardedsleep-signaling` and `guardedsleep-web`, multi-arch, `:dev` for every `main` push, `:latest` and versions on release tags.

## Capabilities

### New Capabilities

### Modified Capabilities

Outcome lives in `distribution` (migrated 2026-10-22).

## Impact

Archive-only.
