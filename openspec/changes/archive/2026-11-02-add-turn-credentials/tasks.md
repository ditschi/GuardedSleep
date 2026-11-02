# Tasks

Commit after each task; `just test-affected` after each task. Branch `feat/turn-credentials`.

## 1. Android

- [x] 1.1 `platform`: `TurnStore` (file `turn.bin`, Keystore alias `guardedsleep_turn`, `KeystorePairingCipher`) with `load`, `save`, `clear`; a pure `TurnServerInput.validate(url, username, password)` with the case table of design.md; wiring in `AppGraph`. Verify: JVM tests for the store (fake cipher) and the validator.
- [x] 1.2 `ui-contract`: `SettingsState.turnUsername`, `turnPasswordSaved`; `AppController.setTurnServer(url, username, password)` returning a validation result, `clearTurnServer()`. `platform` implements them; `SettingsRepository` stops persisting `turn_url` and migrates a legacy value into `TurnStore` once. Verify: JVM tests for keep-password, clear and the migration.
- [x] 1.3 `LinkController` passes the stored user name and password to `IceConfig` (debug `pending.turn` first). Verify: a JVM test on the built `IceConfig`.
- [x] 1.4 `app` settings screen: address, user name and password fields (password masked, "Password saved" placeholder), Save and Remove, error for each rule; stable test tags `settings_turn`, `settings_turn_user`, `settings_turn_pass`, `settings_turn_save`, `settings_turn_remove`. Verify: Robolectric test; Roborazzi baselines re-recorded in the pinned image (report `SKIPPED` without it).

## 2. Web

- [x] 2.1 Contract models and controller: `turnUsername`, `turnPasswordSaved`, `setTurnServer`, `clearTurnServer`, memory only; validator with the same case table. Verify: Vitest.
- [x] 2.2 ICE servers: relay STUN plus the TURN entry with `username` and `credential`. Verify: Vitest on the servers passed to `createMedia`.
- [x] 2.3 Settings screen fields with the same test ids; the browser-storage test covers the TURN password. Verify: Vitest; Playwright settings flow (report `SKIPPED` without browsers).

## 3. Copy and docs

- [x] 3.1 `locales/en.json`: `native.ui.settings.turn.username`, `.password`, `.passwordSaved`, `.credentialsInvalid` (one message for the both-or-neither and length rules); de, fr, es translated (`machineTranslated`). Verify: `just gen`, `just lint-text`.
- [x] 3.2 Update `docs/contributing/turn-server-vps.md` §5.3 and §9; add the backlog `IDEA` "TURN entry in the QR invite". Verify: `just lint-text`.
- [x] 3.3 Full checks: `just lint`, `just test`. Verify: both green.
