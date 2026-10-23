# Proposal

## Why

Parents with little technical knowledge must be able to pair and monitor without typing a server address or running a server. That must work at home on one Wi-Fi, without internet, between a phone and a PC, and across networks (Wi-Fi ↔ mobile data), already in the alpha. Today release builds ship without a default signaling URL. Offline LAN is planned for the beta (ADR-021 §1), TURN for T1, and switching between LAN and internet is manual (`IDEA`). The owner does not want to run a public community server long term, and an outage of a single project server must not stop every family (owner, 2026-10-23).

This change revisits the connection model independently of earlier decisions. It changes the **alpha scope** (ADR-018, ADR-021 §1), the **architecture** (one signaling transport becomes several parallel mailboxes) and the **security model** (signaling payloads end-to-end encrypted, third-party mailboxes, automatic fallbacks). The Critical Reviewer role is therefore mandatory (AGENTS.md §6).

## What Changes

- **Parallel mailboxes.** The core's signaling transport becomes a set of mailboxes used at the same time: the baby device's embedded LAN listener, a local PC relay, and internet mailboxes. A peer answering on any of them is enough. The invite carries all of them.
- **Default internet rendezvous without a project-run server.** Release builds ship with a vetted default set of independent, privacy-friendly (EU-first) community mailboxes plus fallbacks, Nostr relays chosen on the basis of the spike (`spike.md`). Our own relay stays the path for LAN, the local PC mode and self-hosting. **BREAKING** for the requirement "Release builds SHALL ship without a default signaling URL".
- **End-to-end encrypted signaling.** Signaling payloads (offer, answer, candidates, stop) are sealed with a key derived from the PSK, so a mailbox operator sees only IP addresses, timing and an opaque per-pairing address. Needs a Security Architect decision: AGENTS.md §7.3 allows HKDF/HMAC only for the pairing proof and room derivation today.
- **STUN fallbacks.** A vetted EU list of STUN servers is used automatically, in order, when the relay's built-in STUN is not available. The setup screen names the operators once. This supersedes "never automatic" in `add-stun-server-choice`.
- **No default TURN in the alpha.** Cross-network pairs behind mobile carrier NAT or DS-Lite may need TURN (about 10–25 %, `spike.md`). The alpha offers a user-entered TURN server, honest copy when no direct path exists, and a local connection-type diagnostic. Project TURN is decided for beta with that data.
- **Offline LAN moves to the alpha.** The scope of `add-offline-lan` (Android embedded listener, local PC mode, relay admission, private-address cleartext rule, invite with the LAN address) becomes alpha scope. Its open "live handoff" non-goal is replaced by the parallel mailboxes.
- **Network changes need no user action.** After Wi-Fi ↔ mobile data, the core's existing recovery (a fresh attempt) runs over whichever mailbox still reaches the peer.
- Out of scope: iOS, remote pairing without a QR code (`DEFERRED`; a short authentication string belongs there), a project-operated public server, Web Push, a desktop app (`IDEA`), video.

## Capabilities

### New Capabilities

- `offline-lan`: monitoring between devices on the same local network without internet (taken over from `add-offline-lan`, alpha target).
- `signaling-mailboxes`: the set of parallel rendezvous channels, their selection, failover and the end-to-end sealed signaling envelope.

### Modified Capabilities

- `signaling-relay`: default signaling configuration, client ICE configuration with automatic STUN fallbacks, the relay as one mailbox kind.
- `pairing`: the invite carries mailbox descriptors (LAN, local PC, internet) instead of a plain URL list; signaling transport security for private LAN addresses.
- `link-supervision`: signaling-transport state covers several mailboxes; recovery after a network change uses any mailbox that reaches the peer.
- `device-setup`: a Connection section with the operator disclosure and "Use only my own server", validation of own relay URLs, and a local connection-type diagnostic.

## Impact

- **Tier:** T0 (alpha). Pulls beta items forward: offline LAN (ADR-021 §1) and possibly a TURN path (T1). ADR-018's alpha scope is superseded where this change says so. `add-offline-lan` and `add-stun-server-choice` are folded into this change and withdrawn when it is applied.
- **Personas:** serves Persona A ("Festtags-Eltern": other people's Wi-Fi, no setup), Persona B (daily home use without a server) and Persona C (old phones on the home Wi-Fi). Trade-off: a third-party mailbox operator learns IP addresses and timing, disclosed in setup copy.
- **Code:** core (`Link` transport state, sealed envelope, invite v4), `guardedsleep-relay` and `guardedsleep-server`, Android platform (LAN listener, NSD, mailbox clients), web platform (`apps/web` link driver), `locales/*.json`, the STUN/mailbox list data file.
- **Effort:** large. It must fit the 12 h/month maintenance budget; the design names what the alpha can drop if the spike shows a backend is unreliable.
