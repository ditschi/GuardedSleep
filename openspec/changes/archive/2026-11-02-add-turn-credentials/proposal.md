# Proposal

## Why

The alpha allows one user-entered TURN server (AGENTS.md §7.1 and §9), but no client can use a safe one: the Android settings take only a URL, the web client has no TURN setting, and a TURN server without authentication is an open relay. User name and password reach the Android media engine today only through the debug launch of the emulator gate. A self-hosted coturn (guide `docs/contributing/turn-server-vps.md`) can therefore not be tested end to end with a release build. Tier T0.

## What Changes

- Advanced → TURN server on Android and web: address, user name and password, validated together (address `turn:` or `turns:` with a host; user name and password both set or both empty).
- Android stores the TURN entry sealed with its own Android Keystore key, never in DataStore; an existing plain `turn_url` preference moves into the sealed store once and is deleted.
- Web keeps the TURN entry in memory only, like the signaling list; nothing goes to browser storage.
- The password is never shown back, never logged and never put into saved UI state; the screen shows only that one is saved.
- Both clients add the entry as one ICE server next to the relay STUN servers; the debug launch entry still takes precedence on Android.
- The TURN guide is updated: §5.3 uses the settings, §9 lists the remaining gaps.

Out of scope: TURN data in the QR invite (backlog `IDEA`), short-lived HMAC credentials and a project TURN (`PLANNED`, T1).

## Capabilities

### New Capabilities

### Modified Capabilities

- `device-setup`: new requirement for the TURN server entry in Advanced.

## Impact

`apps/android` (`ui-contract` SettingsState and AppController, `platform` TURN store, settings repository migration, link controller, `app` settings screen, Roborazzi baselines), `apps/web` (contract models, controller, ICE servers, settings screen, Vitest, Playwright), `locales/*.json` (three new keys), `docs/contributing/turn-server-vps.md`, `docs/backlog.md` (one `IDEA`). No core, protocol or binding change.
