# Tasks

Branch `feat/alternate-roles-apk-size` from `origin/main`. Commit after each group; `just test-affected` after each group.

## 1. Launcher icon and APK size

- [x] 1.1 Adaptive icon (`mipmap-anydpi-v26/ic_launcher.xml`, `ic_launcher_round.xml`, background colour, foreground and monochrome vectors in the safe zone) from the current crescent artwork; manifest `icon` and `roundIcon`; remove the old drawable. Verify: `aapt2 dump badging` on the debug APK names the icon.
- [x] 1.2 `packaging.jniLibs.useLegacyPackaging = true` in `app/build.gradle.kts` with a doc-link comment. Verify: libraries are deflated in the APK (`unzip -v`), `scripts/poc/release-check.sh` still passes.
- [x] 1.3 `dev-apk` job: delete `lib/x86_64/*` from the downloaded APK, `zipalign -P 16 -f 4`, then sign; the release notes say "arm64 phones only". Verify: actionlint (`just lint-workflows`).

## 2. Per-role state (platform, contract)

- [x] 2.1 `PersistedState.defaultRole` (nullable) and per-role peer name, snooze end and last test; migration `defaultRole = role` when absent. Verify: `SettingsRepository` JVM tests incl. migration.
- [x] 2.2 `PairingStore` keyed by role (two sealed files); one-time move of the legacy file to the current role's slot, retried on failure. Verify: JVM tests for both slots, migration and retry.
- [x] 2.3 `AppController`: `chooseRole(role, makeDefault)`, `switchMode()` (only while not monitoring; cancels an open invite; keeps pairings), `setDefaultRole(Role?)`; `AppState.defaultRole`; cold start picks `defaultRole` unless monitoring resumes; `chooseRole` no longer forgets; `forgetPairing` deletes only the current role's pairing. Verify: `LinkController` JVM tests per spec scenario.

## 3. UI and copy

- [x] 3.1 Role screen "Use as default" toggle; "Switch mode" on home and pairing screens (stable test tags `role_default_toggle`, `switch_mode`); Settings Basic "Start mode" radio group (`start_mode_ask`, `start_mode_baby`, `start_mode_parent`). Verify: Robolectric tests; Roborazzi baselines re-recorded in the pinned image (report `SKIPPED` without it).
- [x] 3.2 New keys in all `locales/*.json` (`machineTranslated` markers for non-English). Verify: `just gen`, `just lint-text`.

## 4. Docs

- [x] 4.1 Delete the backlog `IDEA` "Role switching in the UI"; add an `IDEA` for the web client role-switch parity.
