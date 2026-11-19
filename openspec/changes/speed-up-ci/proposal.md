# Proposal

## Why

CI time is spent in serial test runs and waits, not in downloads. Measured on the last green `main` runs (2026-11-19):

- **Pull requests:** `ci-ok` waits for the browser E2E job, 15 min. Of that, 14.1 min are 47 Playwright tests running one after another with `workers: 1`; every other job finishes within 7 min.
- **`main` and Android pull requests:** the emulator gate takes 63 min. 12 min of that is the TURN-lifetime probe (`scripts/poc/turn-lifetime.sh`), which runs before the emulators start, although it needs no emulator. `android-native` runs one at a time per ref, so a new dev APK reaches testers about an hour after a merge.

Tooling only: no product behaviour, architecture, scope or security model changes (`skip_specs: true`). Tier T0.

## What Changes

1. **Shard the browser E2E** into 3 parallel jobs with Playwright's `--shard`. Each shard keeps `workers: 1` and its own `just dev` server, so the tests stay as isolated as today. `ci-ok` needs all shards. Expected: about 6 min instead of 15 on pull requests that touch web or core code.
2. **Run the TURN-lifetime probe as its own job** in `android-native.yml`, in parallel with the emulator gate. The CI gate step skips the probe; `just gate` keeps running both locally. Expected: about 50 min instead of 63 for the gate workflow.
3. **Cache the Playwright Chromium download** (`~/.cache/ms-playwright`, keyed by the Playwright version). The system dependencies are still installed on a cache hit. Expected: about 40 s less per E2E shard.
4. `docs/ci-gates.md` gets the new rows (E2E shards as one row, the `turn-lifetime` job) with runbooks.

Out of scope:

- caching the emulator AVD and boot snapshot (measure later; it competes for the 10 GB cache budget);
- running the gate only on `main` (rejected: Android regressions would surface after the merge);
- the Robolectric jar downloads (handled in PR #87).

Persona impact (`docs/concept/personas.md`): no end-user persona. The owner and contributors wait about half as long for `ci-ok`, and testers get dev builds sooner.

## Capabilities

### New Capabilities

### Modified Capabilities

## Impact

- `.github/workflows/ci.yml`: the `e2e` job becomes a 3-way matrix with a Chromium cache; `ci-ok` needs it unchanged by job id.
- `.github/workflows/android-native.yml`: new `turn-lifetime` job with the gate's triggers and path filters; the gate step skips the probe; the TURN results upload moves to the new job.
- `justfile` / `scripts/lib/e2e.sh`: pass a shard argument through; a switch to skip the TURN probe inside `_gate` (used by CI only).
- `docs/ci-gates.md`, and `docs/contributing/development.md` if it describes the gate's order.
- Runner minutes rise slightly (3 × E2E setup of about 1.5 min, one more job for the probe). Wall time falls.
