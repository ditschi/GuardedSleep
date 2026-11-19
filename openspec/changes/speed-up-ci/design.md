# Design

## Context

Numbers from `main`, run 44586265250 (CI) and 44524225364 / the #85 gate run (android-native):

| Job           | Wall time | Main cost                                                                                     |
| ------------- | --------- | --------------------------------------------------------------------------------------------- |
| browser E2E   | 15 min    | 47 tests, `workers: 1`, `fullyParallel: false`: 14.1 min; build of wasm and server ~1.5 min   |
| android       | 7 min     | `just verify-android` 5.7 min                                                                 |
| test          | 6 min     | `just test` 4.8 min                                                                           |
| lint (code)   | 4 min     | `just lint-code` 3.5 min                                                                      |
| emulator gate | 63 min    | build 5 min, TURN-lifetime probe 12 min (`DURATION_S` 700), emulator boot 4.5 min, AC-1…AC-12 |

Gradle, Rust (Swatinem, sccache), mise and wasm-bindgen are already cached. The remaining minutes in `lint`, `test` and `android` are real compilation and test time.

Layer: CI and build tooling only; nothing in UI, State, Platform, Bindings or Core. Privacy guardrails: unaffected. The TURN probe keeps running in containers on the runner, and no new secret or service is involved.

## 1. Browser E2E in parallel

**Option A — 3 Playwright shards as a job matrix (recommended).** `matrix.shard: [1, 2, 3]`, `just e2e -- --shard=${{ matrix.shard }}/3`. Each job builds, starts its own `just dev` and runs a third of the spec files.

- Pros: no change to test isolation: each shard still runs one worker against its own signaling server, as today. Playwright distributes by file, and the 16 spec files spread well. Failures stay attributable to a shard.
- Cons: the ~1.5 min setup runs 3 times (runner minutes, not wall time). A failure report is split over 3 artifacts (named `playwright-e2e-artifacts-<shard>`).

**Option B — raise `workers` within one job.**

- Pros: one setup.
- Cons: all workers share one `just dev` signaling server and one rate-limit configuration. `workers: 1` and `fullyParallel: false` were chosen because tests pair devices through that server; parallel pairings would need a per-worker port and server. It is a bigger change with flakiness risk.

**Option C — more shards (4–6).**

- Pros: shorter still.
- Cons: setup then dominates (a shard of 8 tests takes ~3 min plus 1.5 min setup); more runner minutes for little gain.

Recommendation: A with 3 shards. `ci-ok` keeps `needs: [… e2e]`; a matrix job reports one aggregate result to `needs`, so the aggregator logic is unchanged. Verify this in the PR: one shard failing must fail `ci-ok`.

## 2. TURN-lifetime probe in parallel

**Option A — own job `turn-lifetime` in `android-native.yml` (recommended).** Same triggers and path filters as `gate`. Docker only, no KVM, no Android SDK; uploads `target/turn-lifetime` on failure. The gate step runs with `GS_GATE_TURN_LIFETIME=0`, which `_gate` honours by skipping `scripts/poc/turn-lifetime.sh`. Locally `just gate` keeps running both (default `1`).

- Pros: removes 12 min from the gate's critical path; the probe's result appears earlier and independently.
- Cons: one more job and `ci-gates.md` row. Release tags require both jobs: the release workflow's `needs` (or the documented release checklist) must list `turn-lifetime` as well as `gate`.

**Option B — start the probe in the background inside the gate job.**

- Pros: no new job.
- Cons: interleaved logs. Docker and CPU load compete with two emulators on one runner, which risks AC timing (AC-3 latency, AC-10 alarm timing). Failure handling in one bash script gets harder.

Recommendation: A.

`dev-apk` keeps `needs: gate`. Whether it should also wait for `turn-lifetime` is a policy question: the probe tests the TURN server setup, not the APK. Recommendation: no. A dev build is not held back by a TURN regression; the red job is visible on `main`.

## 3. Chromium cache

`actions/cache` on `~/.cache/ms-playwright`. The key is the Playwright version, read in a step from `apps/e2e/package.json` / the lockfile (`pnpm --filter @guardedsleep/e2e exec playwright --version`). On a hit, `scripts/lib/e2e.sh` still has to install the system packages: `playwright install-deps chromium` (apt, ~20 s), because the cache holds only the browser. On a miss it keeps today's `install --with-deps chromium`. SHA-pinned `actions/cache` (v6.1.0) with a doc-link comment.

The browser is about 170 MB per cache entry, well within the 10 GB repository budget.

## 4. Gate register

- `docs/ci-gates.md`: the `e2e` row says "3 shards" and how to re-run one shard locally (`just e2e -- --shard=2/3`). A new row `android-native` / `turn-lifetime` describes what it protects (REST credentials keep relaying past their lifetime on coturn) and its runbook (reproduce with `just turn-lifetime`; logs in the `turn-lifetime` artifact).
- `docs/contributing/development.md`: if it describes the gate as "probe, then emulators", say that CI runs them in parallel.

## Expected effect

- Pull requests: `ci-ok` after about 7–8 min instead of 16, bounded by `android` (7 min) and an E2E shard (~6 min).
- `android-native`: about 50 min instead of 63 per run, so dev builds reach testers about 13 min sooner and queued `main` runs drain faster.
- Runner minutes: about +3 min per E2E run, about +1 min for the probe job's setup.

## Discarded alternatives

- **Raising Playwright workers in one job** (§1 B): needs per-worker signaling servers; flakiness risk.
- **More than 3 shards** (§1 C): setup dominates.
- **Probe in the background of the gate job** (§2 B): resource contention with the emulators, messy logs.
- **Caching the AVD and boot snapshot:** about 3 min gain for several GB of cache; revisit with measurements after this change (backlog `IDEA`).
- **Gate only on `main` and nightly:** saves PR runner time, but Android regressions would land on `main` first.
