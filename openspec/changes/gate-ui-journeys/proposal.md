# Proposal

## Why

The emulator gate starts monitoring through `DebugLaunchActivity` and asserts log lines; its Maestro assertion flows have never run, because Maestro is installed neither locally nor in CI and a missing Maestro only logs "skipping". So no automated run has ever paired two Android devices through the real UI. The owner decided on 2026-11-14 to introduce Maestro properly (previously T1) and to run long device tests for larger changes and releases, not on every PR. Split from `ui-journey-tests` (Critical Review 2026-11-14, I11), which covers the cheap per-PR journeys.

## What Changes

- **Maestro pinned with mise**, analytics and update checks off, heap capped, mounted read-only into the gate's runner container so local and CI use one binary.
- **One UI pairing journey on two emulators (minimal on purpose):** both emulators go through onboarding in the real UI; the flows point both devices at the gate's own signaling server and TURN through the Settings UI (own relay, project server off) before the baby shows its code; the baby's code is read from the screen and pasted into the parent; the gate asserts verification, a test alert and "Stop monitoring". The existing assertion flows start running.
- **The code never lands in a published file:** read into a shell variable, handed to the host-side log canary through a mode-0600 file outside the uploaded output, deleted in the exit trap; `canary-scan.sh` accepts extra canaries.
- **Strict runs fail without Maestro** (`SKIPPED: gate-ui needs maestro`, a failure under `GS_STRICT=1`).
- **When:** as the gate today (nightly, dispatch, PRs on the `gate` path filter with 2 min Doze); a release needs a green strict gate run on the tagged commit as a checklist item (automation belongs to `add-android-release`).

## Capabilities

### Modified Capabilities

- `dev-toolchain`: ADDED "UI journeys on two emulators".

## Impact

`mise.toml`/`mise.lock` (+ `[env]`), `just doctor`, `scripts/poc/{gate,local-gate,canary-scan}.sh`, `apps/android/maestro/` (new journey flows, existing ids verified), `.github/workflows/android-native.yml` (Maestro only in the gate job, job name, header), `docs/ci-gates.md` (gate row with its own last-catch date for the UI journeys), `docs/contributing/development.md` (RAM note), AGENTS.md (§4 table, §5.2a Maestro in CI now T0, §9), `renovate.json` (Maestro monthly).
