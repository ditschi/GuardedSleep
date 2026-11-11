# Proposal

## Why

Owner feedback from the first dev APK (2026-11-11):

1. The APK shows no launcher icon. The app declares a plain vector drawable as `android:icon`; there is no adaptive icon (`mipmap-anydpi-v26`) and no `roundIcon`.
2. The dev APK is 53 MB. It carries the `x86_64` libraries the emulator gate needs (about 18 MB) that no phone uses, and AGP stores native libraries uncompressed (35 MB stored as-is).
3. Parents take turns putting the child to bed, so each phone is sometimes the baby unit and sometimes the parent unit. Today the role is chosen once and choosing the other role deletes the pairing, so every swap means re-pairing and there is no switch in the UI. The owner wants the mode chosen at every start (or a default), a way to switch whenever monitoring is not active, and each mode's state kept across restarts.

## What Changes

- **Session role and default role (Android).** The role becomes a per-session choice. A new setting "Start mode" offers _Ask every time_ (default for new installs), _Baby unit_ and _Parent unit_; the role screen gets a "Use as default" toggle. With a default the app opens in that role; without one it shows the role screen. "Switch mode" is reachable from the home and pairing screens whenever monitoring is not active, and never changes the default.
- **One pairing per role (Android).** A phone keeps at most one pairing as baby unit and one as parent unit, each an ordinary pairing (own PSK, own invite, own pins). Switching roles keeps both; "Forget" deletes only the current role's pairing. The pairing's own rules (1 baby + 1 parent, direction-bound proof, encrypted storage) are unchanged. Picks up the backlog `IDEA` "Role switching in the UI" (deleted from the backlog).
- **Launcher icon:** an adaptive icon (background, foreground, monochrome layer) as `android:icon` and `android:roundIcon`.
- **APK size:** native libraries are packaged compressed (`useLegacyPackaging`); the `dev-apk` job removes the `x86_64` libraries from the gate-tested APK before re-signing it.
- Out of scope: the web client keeps its current role flow (backlog `IDEA` for parity); R8 for debug builds; per-role copies of the shared settings (they already survive restarts and switches).

Persona impact: serves the two-parent household that alternates nights; the single-role household sets a default once and sees no change.

## Capabilities

### New Capabilities

### Modified Capabilities

- `device-setup`: role choice becomes session role + default role, mode switch.
- `pairing`: one stored pairing per role; forget is per role.
- `distribution`: adaptive launcher icon, compressed native libraries.

## Impact

`apps/android` (`ui-contract` `AppState`/`AppController`, `platform` `SettingsRepository`, `PairingStore`, `LinkController`, `app` `Entry.kt`, `RoleHomeScreens.kt`, `SettingsScreen.kt`, manifest and resources, `app/build.gradle.kts`), `locales/*.json` (new keys), `.github/workflows/android-native.yml` (`dev-apk` job), `docs/backlog.md`, Roborazzi baselines (re-recorded in the pinned image). Storage migration: an existing install's role becomes its default role and its pairing becomes that role's pairing, so nothing changes for it until the user changes the setting. Security review: two pairings instead of one, both Keystore-encrypted under the same rules; no crypto, protocol or relay change.
