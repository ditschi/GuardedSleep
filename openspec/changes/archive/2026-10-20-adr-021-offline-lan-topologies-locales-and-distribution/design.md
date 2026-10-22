# ADR-021: Offline LAN topologies, locale policy and distribution channels

- Status: DECIDED (owner, 2026-10-20)
- Date: 2026-10-20
- Deciders: Project owner. Team review 2026-10-20: Product Owner, Technical Architect, Security/Privacy Architect, UX/Usability Expert, Critical Reviewer.
- Supersedes:
  - [ADR-012](../2026-10-21-adr-012-connection-model-offline-lan-and-handoff/design.md): the platform matrix row "no internet, same LAN" for web clients, and the Capacitor implementation of the embedded listener.
  - The owner decision of 2026-10-19 that limited i18n to en, de, fr, es (AGENTS.md §9).
  - [ADR-018](../2026-10-20-adr-018-alpha-milestone-scope-and-trust-features/design.md): the cleartext, language-fallback and offline-LAN deferral statements (partly, marked inline there).
  - The "F-Droid" store wording in [vision.md](../../../../docs/concept/vision.md) and [ADR-005](../2026-04-15-adr-005-parent-alerts-product-naming-and-distribution/design.md).
- Related: [ADR-020](../2026-10-20-adr-020-toolchain-and-development-setup/design.md), [ADR-003](../2026-04-15-adr-003-connectivity-webrtc-signaling-and-qr-pairing/design.md), [ADR-004](../2026-04-15-adr-004-encryption-transport-and-key-storage/design.md)

> **Migrated to OpenSpec (2026-10-22):** §2 is decided but only partly built: the four shipped locales are normative in [`openspec/specs/i18n`](../../../specs/i18n/spec.md), the ~20-locale policy goes into the prepared change `expand-locales`. §3 is normative in [`openspec/specs/distribution`](../../../specs/distribution/spec.md) for what is built; the Android channels go into the prepared change `add-android-release`. §1 stays binding until `add-offline-lan` is prepared.

## Context

The toolchain review ([ADR-020](../2026-10-20-adr-020-toolchain-and-development-setup/design.md)) raised three product questions that tooling cannot decide.

1. **Offline LAN.** A web client served over HTTPS cannot open `ws://` to a phone on the LAN: it is mixed content, and Chrome's Local Network Access does not yet cover WebSockets. A phone without internet cannot obtain a certificate that browsers trust. `connectivity-pairing.md` also contradicted itself: offline LAN is listed as MVP, but the phone listener as Phase 2.
2. **Locales.** The 2026-10-19 decision fixed four locales and implied native-speaker review before shipping. The team found no named reviewers for fr/es, and noted that hiding a locale in the picker does not stop Android from using its resources.
3. **Distribution.** F-Droid main builds from source. The app ships a prebuilt libwebrtc AAR and JNA's prebuilt native library. Google Play requires a 12-tester, 14-day closed test for personal accounts created after 2023-11-13. Google's developer verification applies to sideloaded installs from 2026-09-30 in four countries and worldwide in 2027.

## Decision

### 1. Offline LAN topologies

Offline means "no internet, devices on the same Wi-Fi or hotspot". Two topologies are supported.

| Topology                                                                          | How it works                                                                                                                                                                                                                                                                                           | Status                                                 |
| --------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------ |
| **Android ↔ Android**                                                             | The Android baby unit runs an embedded signaling listener while monitoring is active. The parent finds it by NSD or from the QR invite.                                                                                                                                                                | DECIDED; target **beta**                               |
| **Android ↔ PC/laptop, PC ↔ PC**                                                  | The PC runs the GuardedSleep server binary (or its compose bundle) in **local mode**: it serves the web UI on `http://localhost` and signaling on the LAN. Android devices connect to the PC's LAN address. A second PC also runs the binary locally for its own UI and uses the first PC's signaling. | DECIDED; target **beta**                               |
| Browser over HTTPS ↔ phone listener                                               | Not supported. A self-signed certificate click-through would teach users to accept certificate warnings, which undermines pairing as the trust anchor.                                                                                                                                                 | REJECTED                                               |
| Browser over HTTPS ↔ phone via Chrome Local Network Access (long-polling `fetch`) | Chromium only. WebSockets are not yet covered by LNA.                                                                                                                                                                                                                                                  | `STATUS: IDEA`; re-check when Chrome covers WebSockets |
| iOS offline                                                                       | Revisit at the iOS gate.                                                                                                                                                                                                                                                                               | DEFERRED (Phase 2)                                     |

How the local PC mode stays secure and simple:

- **The browser stays on loopback.** It talks only to its own local GuardedSleep binary (`http://localhost` page, `ws://localhost` signaling) and never connects to a LAN address directly. Toward the LAN, the local binary is either the relay itself (the hosting PC) or a client of the hosting relay (a second PC: its binary forwards to the host over the LAN). Because only loopback is used, this works in all browsers and there is no mixed-content or Local Network Access question.
- `http://localhost` is a secure context, so `getUserMedia`, the service worker, notifications and Wake Lock work.
- **Loopback binding:** the local UI and its loopback API bind to `127.0.0.1`/`::1` only. The binary checks `Origin` and `Host` on every request (DNS-rebinding and cross-site protection). Only the LAN signaling port binds to the LAN interface.
- LAN signaling is an untrusted relay. End-to-end security comes from the direction-bound pairing proof and the DTLS fingerprint binding ([ADR-004](../2026-04-15-adr-004-encryption-transport-and-key-storage/design.md)), as for the phone listener, and is unchanged by the admission rule below.
- **Relay admission (Android listener and PC relay alike):**
  - The paired client gives its local relay an admission key, `HKDF(PSK, "guardedsleep relay admission v1")`. The client is the phone app itself, or the local UI over loopback. The key is not reversible to the PSK, so the PC relay never holds the PSK.
  - The relay sends each joiner a fresh nonce.
  - The joiner answers with `HMAC(admission key, nonce ‖ direction ‖ room id)`.
  - A role slot is assigned only after a valid answer, so replaying a captured first message fails and cannot take the single parent slot.
  - Unauthenticated sockets are closed within 5 s.
  - This needs a protocol change in the `relay` crate (wave 2), with a negative vector for a replayed first message ([ADR-020](../2026-10-20-adr-020-toolchain-and-development-setup/design.md) §3).
- **Listener rules (phone and PC):**
  - It binds only while a session is active.
  - It never advertises anything derived from the PSK.
  - The server, not the page, reports the PC's LAN address for the QR.
- **Android cleartext:** the network security config allows cleartext toward private LAN addresses (it takes domains, not CIDR ranges, so it cannot be tighter). One check in the platform layer, before any connection, rejects every non-private destination: only RFC 1918, RFC 4193 (ULA) and link-local literal addresses pass; 100.64/10 and hostnames are refused. The release-APK assertion checks that the base config stays restrictive. This partly supersedes [ADR-018](../2026-10-20-adr-018-alpha-milestone-scope-and-trust-features/design.md) ("Release builds disable cleartext traffic"), which holds for the alpha only.
- The PC shows the LAN signaling address in the pairing QR, so the phone needs no manual input.
- The room and admission logic lives once, in the sans-IO Rust `relay` crate. The platform owns the sockets: axum in the server binary, Kotlin on Android.
- **User copy:** the app says plainly that a laptop without internet needs the local GuardedSleep app running. It never implies that an ordinary browser tab works offline.

PC ↔ PC follows from the owner's "Android ↔ laptop on LAN" plus the second-PC case. Beta is the target because the team review set offline as a beta exit criterion; the owner may move either earlier.

The alpha uses internet signaling for all pairs. Offline Android ↔ Android and offline PC mode are beta exit criteria.

### 2. Locale policy

- **AI-translated catalogues** are shipped for frequently spoken languages. Native speakers improve them over time.
- **Initial set (about 20):** en (master), de, fr, es, it, pt, nl, pl, sv, da, cs, ro, zh-Hans, ja, ko, hi, ar, tr, uk, id. Arabic is included so that right-to-left layout is proven early.
- **Completeness rule:** a locale is generated only when every key tagged `critical` (alerts, link loss, connection state, safety warnings) is present, so a mixed-language alarm is impossible. Non-critical keys may fall back to English.
- **Review markers:** every non-English value carries a `machineTranslated` or `reviewed` marker, so review progress is visible.
  - Review is an improvement path, **not** a gate for shipping.
  - The owner reviews German.
  - Weblate opens to volunteers at beta.
- **Prompting rules for AI translation:**
  - Keep placeholders and plural categories intact, and keep alert copy short and unambiguous.
  - The generator validates the output: placeholders, CLDR plural categories, length budgets on critical keys.
  - Screenshot and overflow tests cover de (long) and ar (RTL).
- **Further locales** can be added the same way without a new ADR.

### 3. Distribution channels

| Channel                                                       | Alpha                              | Beta                | 1.0                                                                        |
| ------------------------------------------------------------- | ---------------------------------- | ------------------- | -------------------------------------------------------------------------- |
| Google Play (internal → closed → open testing)                | yes (`org.guardedsleep.app.alpha`) | yes                 | yes                                                                        |
| GitHub Releases APK (signed with the direct-distribution key) | yes                                | yes                 | yes                                                                        |
| IzzyOnDroid (developer-signed, reproducibility-verified)      | —                                  | yes                 | yes                                                                        |
| F-Droid main repository                                       | —                                  | —                   | `STATUS: DEFERRED` until a from-source build of libwebrtc and JNA is sized |
| PWA (static container) and local PC mode                      | yes (online)                       | yes (incl. offline) | yes                                                                        |

- **Play account:** use an organisation account if a D-U-N-S number is easy to get, which removes the 14-day closed-test requirement. Otherwise the closed test with at least 15 recruited testers _is_ the beta.
- **Developer verification:** the owner registers the developer identity and package names **now**, because only calendar time is at stake. The same registration covers sideloaded and IzzyOnDroid installs.
- **Store wording:** vision and store texts say "F-Droid clients via IzzyOnDroid; F-Droid main repository later". There is no public promise of F-Droid main until the deferral is resolved.

## Options Considered

### Offline LAN

- **A: Android ↔ Android only** (team recommendation). Pros: smallest scope. Cons: excludes the laptop baby unit, which [ADR-019](../2026-10-20-adr-019-language-repository-and-client-policy/design.md) made a supported product role.
- **B: Android ↔ Android plus a local PC mode (CHOSEN by the owner).** Pros: laptops work offline without certificate warnings, and the same binary serves self-hosters. Cons: the PC user must install and run a local app.
- **C: HTTPS web client ↔ phone listener with a self-signed certificate.** REJECTED: certificate warnings break the trust model.
- **D: Chrome Local Network Access.** IDEA: Chromium only, WebSockets not yet covered.

### Locales

- **A: Four locales with native-speaker review before shipping.** REJECTED by the owner: too narrow, and blocked on reviewers who do not exist yet.
- **B: AI-translated set of about 20 with review markers and a completeness rule for critical keys (CHOSEN).**
- **C: All 24 EU languages plus world languages (about 32).** Rejected for now: catalogue upkeep for every string change. It can be extended later without an ADR.

### Distribution

- **A: F-Droid main at beta.** REJECTED: it requires building libwebrtc and JNA from source, which is hours of build time and an unsized effort.
- **B: IzzyOnDroid at beta, F-Droid main deferred (CHOSEN).**

## Consequences

### Positive

- Offline use covers both product roles of the laptop without weakening the pairing trust model.
- The language reach grows from four to about twenty locales at almost no cost, while alarms stay single-language.
- The distribution plan names only channels the build can actually satisfy.

### Negative

- Offline laptop use requires installing a local app; a plain browser tab is not enough.
- Machine-translated alert copy may read awkwardly until a native speaker reviews it.
- The Android listener has to be rebuilt in Kotlin on the `relay` crate. The Capacitor implementation is legacy.

### Risks

- Machine translation can produce wrong safety copy. Mitigations: completeness and length checks, the `machineTranslated` marker shown to reviewers, and owner review for German. A user-visible "translation feedback" link is an `IDEA`.
- Desktop operating systems may block incoming LAN connections to the local binary (firewall prompts). The PC mode documentation covers the firewall prompt per OS.
