# Tasks

Branch `fix/parent-pairing-entry` from `origin/main`. Commit after each task; `just test-affected` after each task.

## 1. Platform

- [x] 1.1 Parent `startPairing()` opens `PairingState(SCANNING)` (cap respected, no run started). Verify: `LinkControllerTest` (parent start → scanning; at cap → no change; cancel → null; submit after scan unchanged).
- [x] 1.2 `RolePairings.markedDefaultId` / `clearDefault`; `SavedPairing.isDefault` reports the marker only; parent `activeId` without the implied default; baby `defaultId` unchanged. `AppController.clearDefaultConnection()` (+ fakes, web contract mirror). Verify: JVM tests (new pairing not default, clear, baby fallback kept).
- [x] 1.3 Automatic start in `onForeground()`, once per process, only for a marked, saved parent default with monitoring not active and no pairing open; resume of an active run unaffected. Verify: `LinkControllerTest` (starts once; not after stop; not without marker; not for the baby; not while pairing).

## 2. UI and copy

- [x] 2.1 Parent home: scan directly on entry with no saved connection (once per entry, saveable flag); the auto-connect switch for the selected connection (also with one connection); Settings → Connections "Remove default". Stable test tags. Verify: Robolectric tests; Roborazzi baselines recorded in the pinned image.
- [x] 2.2 Copy in `locales/*.json` (en master, `machineTranslated` elsewhere). Verify: `just gen`, `just lint-text`.

## 3. Specs

- [x] 3.1 Reword the parent part of `multi-parent-pairings` "Connection choice per role" (default preselected → marked default starts automatically, see this change). Verify: `openspec validate --all --strict`.
- [ ] 3.2 `just lint`, `just verify-android` green.
