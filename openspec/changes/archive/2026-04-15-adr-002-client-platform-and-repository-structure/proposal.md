# Proposal

## Why

The project needed a client platform, UI stack, repository layout and signaling server stack. Migrated verbatim from ADR-002 (DECIDED 2026-04-15), fully superseded by ADR-015, ADR-019 and ADR-020; `design.md` is the original ADR.

## What Changes

- SvelteKit + Capacitor client and a Node signaling server in a pnpm monorepo (superseded: native Android and a Vite + Svelte web client on a shared Rust core, Rust server).

## Capabilities

### New Capabilities

### Modified Capabilities

Archive-only (history).

## Impact

Archive-only.
