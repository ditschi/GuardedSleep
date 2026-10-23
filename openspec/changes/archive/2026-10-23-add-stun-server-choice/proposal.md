# Proposal

> **Withdrawn 2026-10-23, not implemented.** Superseded by `alpha-connection-model` (owner: vetted EU STUN is used automatically with disclosure and an off switch instead of opt-in only). Its discarded alternatives are carried over into that change's `design.md`. The specs were not changed.

## Why

The relay's built-in STUN is the default (`add-rust-signaling-server`), but not every user runs or reaches a relay whose STUN works for their network, and not every user can run their own server. A community server comes later, if at all. Public Google STUN is not an option: it shows the user's IP address and usage times to a large third party. The owner decided (2026-10-22) on a user choice from a vetted list of European, privacy-respecting STUN servers, plus manual entry, with a clear warning.

## What Changes

- A curated list of third-party STUN servers in the repository (data file), each with operator, country, privacy policy link and the date it was last vetted; vetting criteria documented in `design.md`.
- Android and web: a setting "Connection helper (STUN)" in Advanced, default "Your signaling server". The user can pick a server from the list or enter one manually (`host:port`).
- Choosing a third-party server shows a plain-language warning naming the operator and that it sees the device's IP address and when the app connects; the choice is never automatic.
- No Google STUN in the list, in defaults or in code.
- Copy in all shipped locales; critical-key rules apply to the warning.

## Capabilities

### New Capabilities

### Modified Capabilities

- `signaling-relay`: third-party STUN choice.

## Impact

Android settings and ICE configuration, web client settings (`rebuild-web-client`), `locales/*.json`, a new data file for the server list, `docs/backlog.md` (community server stays `IDEA`).
