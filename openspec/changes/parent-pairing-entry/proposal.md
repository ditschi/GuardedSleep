# Proposal

## Why

On Android the parent unit cannot pair at all: "Set up new devices" on the parent home does nothing, and finishing the parent setup checklist lands on an empty home screen. `LinkController.startPairing()` returns early for every role but the baby (since the native snapshot), so the parent never reaches the scan screen (S05). Found by the owner on 2026-11-14.

The owner also decided how the parent should enter a session (2026-11-14):

- With no saved pairing there is nothing to choose, so the parent goes straight to scanning the baby device's code.
- A saved pairing can be marked as the default. A marked default connects automatically when the app opens in the parent role; without a marked default the parent sees the known home menu with the saved connections and "Set up new devices".

## What Changes

- **Bug fix:** `startPairing()` on the parent opens the scan screen (`PairingPhase.SCANNING`); the camera permission is asked just in time there (alpha-ux-flows §3.2 step 3). At the cap of saved parent pairings the button stays hidden, as today.
- **No saved pairing → scan:** after the parent setup checklist, and whenever the parent home would show its empty state on entry, the scan screen opens directly. Cancelling it returns to the empty home with a working "Set up new devices" button; it does not reopen by itself until the parent home is entered again.
- **Optional default with automatic start (parent only):** the default is a user choice, never implied. Newly saved parent pairings are not default. When the app opens in the parent role with monitoring not active and a marked default that is still saved, it selects that connection and starts monitoring at once; once per app start, so stopping monitoring shows the home menu and does not restart it. The parent home offers a switch "Connect automatically when the app opens" for the selected connection; Settings → Connections offers set and remove default.
- **Baby unchanged:** the baby keeps using its default parent pairing (marked, else the first saved one).
- **Supersedes** the parent part of `multi-parent-pairings` "Connection choice per role" ("preselect the default connection"); that delta is reworded to match.

## Capabilities

### Modified Capabilities

- `device-setup`: ADDED "Parent pairing entry" and "Automatic start with the default connection".

## Impact

Android `platform` (`LinkController`, `RolePairings`), `ui-contract` (`AppState`/`ConnectionUi` default flag semantics, `clearDefaultConnection`), `app` (home, Settings → Connections, navigation), copy in all locales. The web client has no saved pairings yet and is unchanged; its contract mirror follows the new method.
