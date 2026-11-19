# Tasks

Branch `ci/speed-up-ci`. Each implementation task is a brief for the `implementer` sub-agent. Layer rule: CI and build tooling only. SHA-pinned actions with version comments, `permissions` per job, a doc-link comment for every version-specific setting (AGENTS.md §5.2).

## 1. Browser E2E shards

- [x] 1.1 **Shard argument.** Goal: `just e2e` passes extra arguments through to Playwright (`scripts/lib/e2e.sh` already forwards `"$@"` to `pnpm run e2e`; check that the `justfile` recipe forwards them through `slot.sh` and `_e2e`). Files: `justfile`, `scripts/lib/e2e.sh`. Acceptance: `just e2e -- --shard=1/3 --list` lists about a third of the tests.
- [x] 1.2 **Matrix job.** Goal (design §1): the `e2e` job in `.github/workflows/ci.yml` becomes `strategy: { fail-fast: false, matrix: { shard: [1, 2, 3] } }`, name `browser E2E (Playwright, shard N/3)`, runs `just e2e -- --shard=${{ matrix.shard }}/3`, and uploads `playwright-e2e-artifacts-${{ matrix.shard }}` on failure. `ci-ok` keeps `needs: e2e`. Acceptance: `just lint-workflows`. On the PR, every shard runs; a deliberately failing assertion in one spec (temporary commit, reverted before review) turns `ci-ok` red.

## 2. TURN-lifetime job

- [x] 2.1 **Skip switch.** Goal (design §2): `_gate` in the `justfile` runs `scripts/poc/turn-lifetime.sh` unless `GS_GATE_TURN_LIFETIME=0`. Comment that CI runs the probe as its own job. Acceptance: `GS_GATE_TURN_LIFETIME=0 just --dry-run _gate` (or reading the recipe) shows the probe skipped; locally the default still runs it.
- [x] 2.2 **Job.** Goal: a `turn-lifetime` job in `.github/workflows/android-native.yml` with the gate's triggers and path conditions, `permissions: { contents: read }`, Docker only (no KVM, no Android setup), running `just turn-lifetime` (or `scripts/poc/turn-lifetime.sh` under `GS_STRICT=1`), uploading `target/turn-lifetime` on failure. The gate step sets `GS_GATE_TURN_LIFETIME: "0"` and no longer uploads the probe directory. `dev-apk` keeps `needs: gate` only (design §2). If a release workflow or the release checklist requires the gate on `android-v*` tags, add `turn-lifetime` there too. Acceptance: `just lint-workflows`; on the PR, both jobs start at the same time and the gate log shows no probe.

## 3. Chromium cache

- [x] 3.1 Goal (design §3): in the E2E job, a step reads the Playwright version, `actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9 # v6.1.0` caches `~/.cache/ms-playwright` keyed by it, and `scripts/lib/e2e.sh` runs `playwright install-deps chromium` when the browser is present under `GS_STRICT=1` in CI, so system packages exist on a cache hit. Acceptance: `just lint-workflows`, shellcheck. On the PR's second run, the cache step reports a hit, and the E2E step skips the browser download but still passes.

## 4. Docs and checks

- [ ] 4.1 `docs/ci-gates.md`: update the `e2e` row (3 shards, local re-run of one shard), add the `android-native` / `turn-lifetime` row with runbook (design §4), and update the gate row (the probe now runs in its own job). `docs/contributing/development.md` if it describes the gate order. Acceptance: `just lint-text`.
- [ ] 4.2 Record the measured times of the PR's CI runs (E2E shards, gate, `turn-lifetime`) in the PR description, against the baseline in the design. Final: `just lint` and `just test-affected origin/main` green; report any SKIPPED task.

## Workflow follow-up

- Add an `IDEA` to `docs/backlog.md`: cache the emulator AVD and boot snapshot, measured against the cache budget (design, Discarded alternatives).
- Archive with `/opsx:archive` after the first green `main` run with the new jobs.
