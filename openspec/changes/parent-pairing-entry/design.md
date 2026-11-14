# Design

## Context

`LinkController.startPairing()` starts with `if (role != BABY) return`. `AppNavigator` shows `PairParentScreen` only while `pairing != null`, and `PairParentScreen` already treats `SCANNING` as its start phase, so the parent never gets there. `RolePairings.defaultId()` falls back to the first saved pairing, so today every saved parent pairing set has a default, whether the user chose one or not.

## Decisions

### 1. Parent `startPairing()` opens the scan screen

`startPairing()` on the parent: return when `pairings.freeId(PARENT)` is null (cap), else `cancelPairing()` and set `PairingState(SCANNING)`. No run starts until a code is submitted (`submitPairingCode`, unchanged). `cancelPairing()` already clears the state.

### 2. Scan directly when nothing is saved

The decision belongs to the UI layer as navigation, with a guard against loops: the parent home, on entry with no saved connection and no pairing open, calls `startPairing()` once per entry (a saveable flag). Cancel returns to the empty home and the flag keeps it there. After the setup checklist the onboarding path only calls `completeOnboarding()`; the empty parent home is entered and its one-time auto-scan opens the scan screen (any scan screen that opens counts as this entry's scan, so cancelling does not rescan).

### 3. Explicit default, automatic start once per app start

- `RolePairings` gets `markedDefaultId(role)` (the marker only, no fallback) and `clearDefault(role)`. `defaultId()` keeps its fallback for the baby. `SavedPairing.isDefault` / `ConnectionUi.isDefault` report the marker only.
- Parent `activeId()` uses the session choice, else the marked default, else the first saved one (unchanged order, minus the implied default).
- Automatic start lives in `LinkController` and means "when the app opens in parent mode". A pending flag is set once per process (at construction) and settled by the first thing that decides it: the first `onForeground()` with a role (PARENT with onboarding done: act; BABY: clear), or, when that call finds the role screen (start mode "Ask every time"), the first `chooseRole()` (PARENT: act; BABY: clear). `switchMode()` clears it. Acting means: monitoring not active, no pairing open, a marked default still saved → `selectConnection(default)` + `startMonitoring()`. Whether it acted or not, it never fires twice per process, so rotation, returning from the background, a stop followed by a resume, or a baby → switch mode → parent round trip never start monitoring mid-session. When it starts monitoring, `onForeground()` skips its own resume `startHost()`. Resume of an active monitoring run after process death keeps its own path and wins (the automatic start sees monitoring active and does nothing).
- Existing installs: a marker file exists only where the user pressed "Set default" in #56, so it stays; nobody gets an automatic start they did not choose.

### 4. UI

- Parent home, with at least one saved connection: a switch "Connect automatically when the app opens" bound to "the selected connection is the marked default" (on → `setDefaultConnection`, off → `clearDefaultConnection`). Shown with one connection too, since the list is hidden then.
- Settings → Connections: "Set as default" and, on the default, "Remove default".

## Discarded alternatives

- **Preselect only (multi-parent-pairings as written).** Pros: no surprise connection. Cons: the owner wants one-tap-free start for the usual setup. Rejected by the owner, 2026-11-14.
- **Always auto-start the first pairing.** Pros: no setting. Cons: no way to land on the menu without stopping first; a second pairing would be unreachable at start. Rejected by the owner, 2026-11-14.
- **Auto-scan from the platform (open `SCANNING` in `refreshApp`).** Cons: cancel would reopen it at once; navigation is a UI concern.

## Risks / Trade-offs

- An automatic start right after the app opens connects without a tap. It only happens for a connection the user marked, and the switch on the home screen is the way out.
