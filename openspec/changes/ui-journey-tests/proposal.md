# Proposal

## Why

Two standard flows were broken on Android without any test noticing (owner, 2026-11-14): the parent's "Set up new devices" did nothing (`startPairing()` returned early for the parent) and the baby setup's device check spun forever (nothing set `AppState.deviceCheck`). A coverage analysis of the same day found the cause is structural, not a missing case:

- every Compose and Roborazzi test runs against `FakeAppController`, which re-implements the controller's state changes by hand;
- `LinkControllerTest` runs the real controller without UI;
- the emulator gate starts monitoring through `DebugLaunchActivity` and asserts log lines; its Maestro flows never ran, because Maestro is installed neither locally nor in CI, and a missing Maestro only logs "skipping";
- the Kotlin `AppController` gives nine methods empty default bodies, so a missing override compiles and does nothing. Live cases: `reportTestResult` (the "Did you hear it?" answer) and `dismissCoach`; `coachPending` is never set, so the parent's first-run coach never appears.

The web client does not have this gap: Playwright drives the real UI, the real `WebController`, the WASM core and the real server, and TypeScript forces every contract method.

## What Changes

- **Tier 1, every PR (`just test`):** Android journey tests in Robolectric that render the real `GuardedSleepRoot` on a real `LinkController` per device, two devices connected through the in-memory relay and fake media already used by `LinkControllerTest` (moved into shared test fixtures). Journeys cover the standard flows by clicking stable test tags: first run baby and parent through pairing by pasted code, start and stop monitoring, saved connections and the default with automatic start, "Pair again", noise alert with stop sound and snooze, test alert with the answer, kiosk lock and unlock, link loss and recovery, the device check result, switch mode, the Settings test alert.
- **Contract completeness:** the Kotlin `AppController` loses its default method bodies, so every method is implemented or the build fails; a test fails when a `UiContract` state field has no writer in `platform`. The defects this exposes are fixed in this change (test answer, coach marks, Settings test alert while not monitoring).
- **Tier 2, nightly, on dispatch and before a release tag:** Maestro, pinned in `mise.toml` (owner, 2026-11-14; previously T1), runs in the emulator gate. New UI journeys on two emulators click through onboarding on both devices, pair by copying the code shown on the baby (read through `uiautomator`) into the parent's paste field, then monitor, alert and stop. The existing assertion flows finally run. A missing Maestro fails the gate under `GS_STRICT=1`. The debug launch stays for the long Doze and alarm scenarios.
- **Release rule:** a release tag requires a green gate run on the tagged commit (as today for release builds) that includes the UI journeys; larger changes can trigger it by dispatch or the `gate` path filter.

## Capabilities

### Modified Capabilities

- `dev-toolchain`: MODIFIED "Test layers"; ADDED "Android UI journeys through the real controller", "Complete UI contract" and "UI journeys on two emulators".

## Impact

`apps/android/app/src/test` (journeys), shared test fixtures for `platform`, `UiContract.kt` (no default bodies) and every implementer, `LinkController` (missing methods), `scripts/poc/gate.sh` and `local-gate.sh`, `apps/android/maestro/`, `mise.toml`/`mise.lock`, the gate workflow, `docs/ci-gates.md`, AGENTS.md (§5.2a Maestro in CI no longer T1, §9 table), `docs/backlog.md`.
