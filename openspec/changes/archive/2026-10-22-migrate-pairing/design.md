# Design

## Context

Sources: ADR-001 (app lock), ADR-003 §3–§4, ADR-004 with the amendments of 2026-10-19 and 2026-10-20, ADR-017 §1, ADR-018 §2 (pairing row), `docs/features/connectivity-pairing.md` (QR pairing, further parents, babysitter mode, security list, cluster management). Checked against `crates/guardedsleep-core/src/pairing.rs` and the Android `pairing/` and `link/LinkController.kt` code.

## Goals / Non-Goals

**Goals:** one `pairing` spec that describes what is built today; every superseded statement resolved explicitly.

**Non-Goals:** behaviour changes; the frozen legacy web client `apps/mobile` (the web client is specified by its wave-3 change); signaling server behaviour (`migrate-signaling-relay`).

## Decisions

- **Specs describe what is built** (owner, 2026-10-21). Decided but unbuilt items become prepared changes or `PLANNED` backlog items.
- **Newest source wins** where sources disagree; each case is listed so it can be checked:

| Topic                                  | Older statement                                                                     | Resolved as                                                         | Source                                        |
| -------------------------------------- | ----------------------------------------------------------------------------------- | ------------------------------------------------------------------- | --------------------------------------------- |
| QR payload                             | `psk`, `dtls_fingerprint`, `ips`, `signal`, `nonce`, `ts` (ADR-003 §3, feature doc) | `{"v":3,"k","s","x"}`, no room id                                   | ADR-017 §1, code                              |
| QR validity                            | one-time, ~30 s (feature doc)                                                       | 5 min, countdown, fresh PSK, closes after the first verified parent | ADR-018 §2, code                              |
| Join request after the QR is dismissed | ADR-003 §4, feature doc                                                             | not in a 1:1 pairing; kept as `DEFERRED` with multi-parent          | ADR-018 (1:1 alpha)                           |
| Long-term keys over the DataChannel    | feature doc step 7                                                                  | dropped: the PSK is the long-term key until re-pairing              | ADR-004, AGENTS.md §9 key rotation `DEFERRED` |
| Fingerprint MAC                        | symmetric `HMAC(PSK, room \| sorted fps \| version)`                                | direction-bound v3                                                  | ADR-004 amendment                             |
| Keystore storage                       | `IDEA` in the feature doc                                                           | built                                                               | code                                          |
| WSS for signaling                      | ADR-004 §4                                                                          | release builds refuse cleartext; `ws://` only in debug              | code, ADR-018                                 |

- **Finding, not decided here:** the invite payload is parsed in Kotlin, not in the core. The web client will need the same codec, and AGENTS.md wants protocol shapes from the core only. Recorded as `IDEA` in the backlog for the wave-2 façades.

## Risks / Trade-offs

- An "as built" spec can freeze an accidental behaviour. Mitigation: each requirement was traced to a decided source, not only to code.
