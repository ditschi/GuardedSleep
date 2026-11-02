# Design

## Context

Android already carries `TurnCredentials(url, username, credential)` from the debug launch through `LinkController` into `LibWebRtcMediaEngine`, and `PairingStore` can seal it. The settings screen saves only `turnUrl` into DataStore (plain preferences), and `LinkController` uses it without credentials. The web client has `turnUrl` in its `SettingsState` but never reads it; `relayIceServers()` returns only the relay STUN entry. Web settings live in memory except the theme. Guide: `docs/contributing/turn-server-vps.md` (coturn with a static long-term credential).

## Decisions

- **One TURN entry per device, set in Advanced.** Address, user name, password; "Save" and "Remove". Both devices need it only when their own network blocks a direct path, so each device keeps its own entry.
- **Validation in the platform layer, not the core.** The rule is syntax only (same as the signaling URL check today): `^turns?:` followed by a host (IPv4, bracketed IPv6 or DNS name), optional `:port`, optional `?transport=udp|tcp`; no whitespace; user name and password both empty or both set, each at most 256 characters. One pure function per client with unit tests on the same case table.
  - Pros: no binding surface change, no `api.txt` churn, matches how signaling URLs are validated.
  - Cons: two implementations of a small rule; the shared case table in the tests keeps them aligned.
- **Android: a sealed `TurnStore`.** Its own file (`turn.bin`) and Keystore alias (`guardedsleep_turn`), AES-GCM through the existing `KeystorePairingCipher`, so forgetting the pairing does not delete the TURN entry and vice versa. The password is a secret like pairing material (§7.2), so it never goes to DataStore. On first start a legacy `turn_url` preference is moved into the store and removed.
- **The contract never carries the password back.** `SettingsState` gets `turnUsername: String?` and `turnPasswordSaved: Boolean` next to `turnUrl`. `AppController.setTurnServer(url, username, password)` writes; an empty password with an unchanged address and user name keeps the saved one; `clearTurnServer()` removes it. `updateSettings` no longer writes TURN fields. Web mirrors the same shape.
- **Web: memory only.** The entry lives in the controller next to the signaling list and is lost on reload, like every other web setting except the theme. The no-storage test that guards pairing material also covers the TURN password.
- **ICE servers:** relay STUN servers first, then `{urls: [turnUrl], username, credential}`. Android keeps `pending.turn` (debug launch) ahead of the setting.
- **Logging:** neither client logs the address, user name or password; the diagnostics show only "relayed".

## Discarded alternatives

- **TURN data in the QR invite:** one entry for both devices, but it changes the v3 invite in the core and both bindings while `alpha-connection-model` reworks pairing; recorded as a backlog `IDEA`.
- **Validator in the Rust core with UniFFI and wasm exports:** one implementation, but a binding surface change for a syntax rule; revisit if the invite carries TURN.
- **Password in DataStore or `EncryptedSharedPreferences`:** plain storage breaks §7.2; Jetpack Security is deprecated and adds a dependency where a cipher exists.
- **Persist the web entry in `localStorage`:** survives a reload, but puts a credential into browser storage, which the web guardrail forbids for secrets.

## Risks / Trade-offs

- A wrong TURN entry can make a connection worse only when a relay candidate wins over a broken direct one; ICE still tries direct pairs, and Advanced warns that its settings can break monitoring.
- The TURN operator sees IP addresses and session times; the user runs it (guide §1 and §8).
