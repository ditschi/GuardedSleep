# Proposal

## Why

Parents with little technical knowledge must be able to pair and monitor without typing a server address or running a server, already in the alpha. Before this change, release builds shipped without a default signaling URL, and the only signaling path was a relay the user configured.

The first version of this change (owner, 2026-10-23) used public Nostr relays so that no project-run server was needed. **The owner reversed that on 2026-11-02:** volunteer relays mean operator outreach, list churn, rate limits outside our control, soak tooling and a large part of the core; one own server with sealed envelopes leaks the same metadata (IP address and timing) to fewer parties. The project now runs one server, `gs.mydata.rocks`, as the default internet mailbox, with STUN and TURN on the same host, and Nostr is removed completely.

This change revisits the internet connection model. It changes:

- the **architecture**: one signaling transport becomes a mailbox set in the core (relay and LAN kinds); the signaling server issues ICE servers and TURN credentials;
- the **security model**: signaling is sealed end to end, a project-run server and its TURN are used by default, and the baby pins its parent device;
- the **alpha scope**: ADR-018's tester setup, and TURN moves from beta into the alpha.

The Critical Reviewer role is therefore mandatory (AGENTS.md §6). Reviews on 2026-10-23 (two passes) and on 2026-11-02 (this revision) are answered in `design.md`. Offline LAN is a separate alpha change applied after this one (`add-offline-lan`).

## What Changes

- **Mailbox set.** The core keeps a set of mailboxes (the project server, own relays and, with `add-offline-lan`, a LAN listener) and sends every signaling envelope to all of them. While monitoring is connected, the internet mailboxes are closed and only checked every 30 minutes.
- **Project signaling server as the default.** Release builds use `wss://gs.mydata.rocks/signaling` and need no configuration. The host is one entry in `data/project-server.json` with operator, privacy policy and imprint. **BREAKING** for "Release builds SHALL ship without a default signaling URL".
- **Nostr removed.** The Nostr mailbox, NIP-01 codec, BIP-340/`k256`, the daily epoch, budgets, parking, the relay pick, the curated relay list, the soak tooling and the weekly relay job are deleted (they were built in tasks 3.5, 3.6 and 4.1). The decision and its design move to "Discarded alternatives".
- **Sealed, session-bound signaling.** Every signaling message is sealed with XChaCha20-Poly1305 under a key derived only for this purpose from the PSK. It is bound to the recipient's current session, so a stored or replayed message cannot stop escalation.
- **Parent pin.** After the first verification the baby device accepts only that parent device until "Forget".
- **Invite v4** with compact mailbox entries (`r:` relay, `l:` LAN; ≤ 300 bytes), a mailbox update over the DataChannel, and v3 dropped. **BREAKING**, but nothing has been released.
- **STUN and TURN from the signaling server.** coturn on the project host serves STUN and TURN on 3478 (and `turns:` on 5349). The signaling server hands out the URLs and short-lived coturn REST credentials (TTL ≤ 10 min), only into a room with a baby and a parent, and clients accept ICE URLs only on the relay's own host. No third-party STUN by default any more. A user-entered TURN stays possible.
- **Public deployment as an example in this repository:** `docker/public-server/` (server, coturn, optional web) behind the owner's Traefik, with `.env.template` and a README; no IP logging anywhere in the stack.
- **Owner tasks:** privacy policy and imprint, data processing agreement with the hoster, firewall, Traefik access log off, Uptime Kuma check, domain renewal.
- **Withdrawn:** `add-stun-server-choice` (archived), whose alternatives are carried into `design.md`.
- **Out of scope:**
  - LAN listener and local PC mode (`add-offline-lan`);
  - a second project instance or failover (`IDEA`);
  - `turns:` on port 443 (`IDEA`);
  - Web Push;
  - iOS;
  - remote pairing without a QR code;
  - a post-pairing key change (`IDEA`).

## Capabilities

### New Capabilities

- `signaling-mailboxes`: parallel rendezvous channels, the sealed and session-bound envelope, the mailbox update, the project signaling server and its disclosure.

### Modified Capabilities

- `signaling-relay`: default signaling configuration with the project server; client ICE configuration from the relays of the pairing; ICE servers and short-lived TURN credentials issued by the relay; no IP logging in the public deployment; the relay carries sealed envelopes.
- `pairing`: invite v4; one parent through a parent pin; encrypted storage of the mailbox set and the parent id.
- `link-supervision`: presence and sessions replace relay membership in offer direction, accepted senders, scheduling, causes, stop and transport liveness; wall-clock and CSPRNG inputs.
- `device-setup`: the Connection section with disclosure, the off switch in Advanced, the diagnostics view, the baby device check over the mailbox set, and the own TURN entry next to the relay's ICE servers.

## Impact

- **Tier:** T0 (alpha). It amends ADR-018's tester setup and AGENTS.md §7.1, §7.3 and §9 (exact wording in `design.md` §2 and §7). It pulls the project TURN from T1 into the alpha.
- **Personas:** serves Persona B (daily home use without a server) and Persona C (old phones, no setup), and now Persona A ("Festtags-Eltern" on venue Wi-Fi or mobile ↔ mobile) through the project TURN.
- **Trade-off:** the project (the owner) learns IP addresses and usage times of every family on default settings, and relays encrypted audio when no direct path exists. This is disclosed with the operator's name, privacy policy and imprint, nothing is logged, and the off switch turns it off. One server is a single point of failure for internet recovery; verified monitoring is unaffected, and an own relay or the LAN path avoids it.
- **Code:**
  - core: Nostr removal; `ice` parsing with host binding and expiry; ICE servers attached to attempts;
  - `guardedsleep-relay` and `guardedsleep-server`: `welcome.ice`, the `ice` message and the credential issuer;
  - Android and web: Nostr removal, ICE servers from the core, project-server flag, settings and disclosure;
  - `locales/*.json`;
  - `data/project-server.json`, `public-lists` reduced, the web CSP;
  - `docker/public-server/`; docs (`turn-server-vps.md`, a deployment guide, `failure-model.md`, `alpha-hardware-test.md`, `ci-gates.md`).
- **Dependencies:** `chacha20poly1305` and `chacha20` stay; `k256` goes, so the dependency count of `core` drops. The server gains an HMAC-SHA1 implementation (RustCrypto `sha1` with the existing `hmac`), justified in the PR.
- **Effort and upkeep:** the removal and the TURN work are medium. Afterwards the server needs the owner: updates by `git pull && docker compose up -d`, monitoring alerts, renewals, about 1 h per month, instead of the 1–2 h per month the relay list would have cost.
