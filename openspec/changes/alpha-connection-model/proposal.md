# Proposal

## Why

Parents with little technical knowledge must be able to pair and monitor without typing a server address or running a server, already in the alpha. Today release builds ship without a default signaling URL, and the only signaling path is our own relay. The owner does not want to run a public community server long term, and an outage of a single project server must not stop every family (owner, 2026-10-23).

This change revisits the internet connection model. It changes:

- the **architecture**: one signaling transport becomes several parallel mailboxes in the core;
- the **security model**: signaling is sealed end to end, third-party mailboxes and STUN are used by default, and the baby pins its parent device;
- the **alpha scope**: ADR-018's tester setup.

The Critical Reviewer role is therefore mandatory (AGENTS.md §6). A first review on 2026-10-23 asked for rework, which `design.md` answers in its review-response table. Offline LAN is a separate alpha change applied after this one (`add-offline-lan`, retargeted to the alpha).

## What Changes

- **Parallel mailboxes.** The core keeps a set of mailboxes (our relay, public Nostr relays and, with `add-offline-lan`, a LAN listener) and sends every signaling envelope to all of them. While monitoring is connected, the internet mailboxes are closed and only checked every 30 minutes.
- **Default internet rendezvous without a project-run server.** Release builds use a curated set of independent EU Nostr relays (`spike.md`) and need no configuration. **BREAKING** for "Release builds SHALL ship without a default signaling URL".
- **Sealed, session-bound signaling.** Every signaling message is sealed with XChaCha20-Poly1305 under a key derived only for this purpose from the PSK. It is bound to the recipient's current session, so a stored or replayed message cannot stop escalation. Nostr tags and keys rotate daily at 12:00 UTC.
- **Parent pin.** After the first verification the baby device accepts only that parent device until "Forget".
- **Invite v4** with compact mailbox entries (≤ 300 bytes), a mailbox update over the DataChannel, and v3 dropped. **BREAKING**, but nothing has been released.
- **Automatic EU STUN.** Without an own relay, two vetted EU STUN servers of distinct operators are used, and both are disclosed. Advanced has the off switch "Only same Wi-Fi or my own server".
- **No default TURN in the alpha.** The alpha offers a user-entered TURN, plain troubleshooter copy, and a local connection-type diagnostic. Project TURN is decided for beta with data.
- **Go/no-go gate.** A night-long soak on public relays runs before the Android and web work. If it fails, the alpha ships with relay mailboxes only.
- **Withdrawn:** `add-stun-server-choice`, whose alternatives are carried into `design.md`.
- **Out of scope:**
  - LAN listener and local PC mode (`add-offline-lan`);
  - a project-run server or TURN;
  - Web Push;
  - iOS;
  - remote pairing without a QR code;
  - a post-pairing key change (`IDEA`);
  - list refresh without a release.

## Capabilities

### New Capabilities

- `signaling-mailboxes`: parallel rendezvous channels, the sealed and session-bound envelope, Nostr addressing, budgets and parking, the mailbox update, curated public services and their disclosure.

### Modified Capabilities

- `signaling-relay`: default signaling configuration without a URL; client ICE configuration with automatic EU STUN; the relay carries sealed envelopes.
- `pairing`: invite v4; one parent through a parent pin; encrypted storage of the mailbox set and the parent id.
- `link-supervision`: presence and sessions replace relay membership in offer direction, accepted senders, scheduling, causes, stop and transport liveness; wall-clock and CSPRNG inputs.
- `device-setup`: the Connection section with disclosure, the off switch in Advanced, the diagnostics view, and the baby device check over the mailbox set.

## Impact

- **Tier:** T0 (alpha). It amends ADR-018's tester setup (task 1.1) and AGENTS.md §7.1 and §7.3 (exact wording in `design.md`).
- **Personas:** serves Persona B (daily home use without a server) and Persona C (old phones, no setup). Persona A ("Festtags-Eltern" on venue Wi-Fi or mobile ↔ mobile) is served only where a direct path exists, because there is no TURN in the alpha; a hardware round measures it for the beta TURN decision.
- **Trade-off:** a few EU relay and STUN operators learn IP addresses and usage times. This is disclosed, and the off switch turns it off.
- **Code:**
  - core: `Link` mailbox set, envelope, Nostr codec, invite v4, new inputs;
  - `guardedsleep-relay` and `guardedsleep-server` (sealed payloads);
  - Android and web platform mailbox clients and settings;
  - `locales/*.json`;
  - new `data/` lists with a generator and a weekly job;
  - the web CSP (`rebuild-web-client`).
- **Dependencies:** `chacha20poly1305`, `rand_chacha` and `k256` (schnorr), all with `default-features = false`. The dependency count of `core` rises and is justified in the PR.
- **Effort and upkeep:** large to build. Afterwards about 1–2 h per month for the relay list (weekly job, churn, operator contact).
