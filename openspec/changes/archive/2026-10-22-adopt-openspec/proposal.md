# Proposal

## Why

Decisions lived in 21 ADRs next to feature docs with STATUS tags, and the two drifted apart (old heartbeat values, superseded QR payloads, decided-but-unbuilt items that read like facts). The owner wanted one system long term (2026-10-21): current behaviour as specs, work as changes, history in one archive.

## What Changes

- OpenSpec 1.14.3 pinned in mise (telemetry and update check off), `openspec validate --all --strict` in `just lint-text`, generated skills for Claude Code and Cursor.
- Ten capability specs describing what is built: `link-supervision`, `pairing`, `signaling-relay`, `parent-alerts`, `connection-status`, `audio-monitoring`, `device-setup`, `i18n`, `distribution`, `dev-toolchain`.
- Nine prepared changes for decided but unbuilt work (waves 2 and 3, Android release, offline LAN).
- All 21 ADRs archived verbatim; `docs/features/` and `docs/decisions/` removed; ideas, deferred and rejected items in `docs/backlog.md`; `docs/concept/` and `docs/technical/` kept as non-normative reference.

## Capabilities

### New Capabilities

### Modified Capabilities

Process change; the capability specs were created by the `migrate-*` changes archived next to this one.

## Impact

`AGENTS.md`, `.cursor/rules/`, `CONTRIBUTING.md`, `README.md`, the PR template, `openspec/`, `docs/backlog.md`.
