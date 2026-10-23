# Design

## Context

Owner decision 2026-10-22, during the review of `add-rust-signaling-server`: built-in STUN with bounded amplification and a per-IP rate limit is the default; Google STUN is excluded; a vetted European list plus manual entry is the fallback, with a clear warning for less technical users. Depends on `add-rust-signaling-server` (default) and, for the web part, `rebuild-web-client`.

## Goals / Non-Goals

**Goals:** a safe default, an understandable opt-in, a list the project can keep current. **Non-Goals:** TURN (T1), a project-run community STUN server (backlog `IDEA`), automatic fallback without user consent.

## Decisions

- **Vetting criteria** (to be finalised in this change): operator in the EU/EEA with a published privacy policy under GDPR; no advertising or analytics business model tied to the service; documented logging practice; reachable on UDP 3478; re-vetted at least yearly (date stored per entry).
- **The list is data, not code**, so updating it is a small reviewed change.
- **Never automatic:** if the relay's STUN does not answer, the app explains the situation and offers the setting; it does not switch on its own.
- **Discarded alternatives:** Google or other big-tech public STUN (third party sees IP and usage times; owner rejected); a single hard-coded fallback (no user choice, a single point of trust); only manual entry (too technical for many parents).

## Risks / Trade-offs

- List entries can change their policy or disappear; the yearly re-vetting date and a CI reachability check (non-blocking) mitigate this.

## Open Questions

- Initial entries: research candidates (e.g. operators in Germany and other EU countries) against the criteria before implementation.
