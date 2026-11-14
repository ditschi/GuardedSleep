# Proposal

## Why

Two standard flows were broken on Android without any test noticing (owner, 2026-11-14): the parent's "Set up new devices" did nothing (`startPairing()` returned early for the parent) and the baby setup's device check spun forever (nothing set `AppState.deviceCheck`). A coverage analysis of the same day found the cause is structural, not a missing case:

- every Compose and Roborazzi test runs against `FakeAppController`, which re-implements the controller's state changes by hand;
- `LinkControllerTest` runs the real controller without UI;
- the Kotlin `AppController` gives nine methods empty default bodies, so a missing override compiles and does nothing. Live cases: `reportTestResult` (the "Did you hear it?" answer) and `dismissCoach`; `coachPending` is never set, so the parent's first-run coach never appears.

The web client does not have this gap: Playwright drives the real UI, the real `WebController`, the WASM core and the real server, and TypeScript forces every contract method. The emulator side (Maestro journeys on two emulators) is split into `gate-ui-journeys` (Critical Review 2026-11-14, I11), so this change stays cheap and gates every PR.

## What Changes

- **Journeys in `just test`, every PR:** Android journey tests in Robolectric that host two real devices side by side: each a real `LinkController` with its real core, rendered in its own `ComposeView` under a `device_baby`/`device_parent` root, connected through an in-memory relay on one virtual clock. Journeys click stable test tags through the standard flows: first run baby and parent with pairing by pasted code, start and stop on both sides, saved connections with the default and automatic start, "Pair again", noise alert with stop sound and snooze, test alert, kiosk lock and unlock, link loss and recovery, the device check result, switch mode, the Settings test alert. Budget: all journeys under 90 s.
- **Complete UI contract:** the Kotlin `AppController` loses its default method bodies, so the compiler enforces every method as TypeScript does on web. A recorder on the journeys' state flows fails when a leaf field of the UI state never reaches a non-default value in any journey (allowlist with reasons).
- **Exposed product gaps** (test answer, coach marks, Settings test alert while not monitoring) are not specified well enough to fix here; until the owner decides, they are allowlisted with a pointer to the backlog entry, and their journeys are marked pending. They get their own fix change.

## Capabilities

### Modified Capabilities

- `dev-toolchain`: MODIFIED "Test layers"; ADDED "Android UI journeys through the real controller" and "Complete UI contract".

## Impact

New `apps/android/test-fixtures` module (virtual scheduler and clock, in-memory relay, fake media, a two-device builder exposing only contract types), seams in `LinkController` for its remaining executors, Gradle wiring for the real core under Robolectric in `app` tests, `UiContract.kt` (no default bodies) and every implementer, journeys in `apps/android/app/src/test`, `docs/ci-gates.md` (test job row), `docs/backlog.md`.
