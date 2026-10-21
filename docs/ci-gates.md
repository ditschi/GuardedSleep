# CI gates

Register of every automated gate, per [ADR-020](decisions/ADR-020-toolchain-and-development-setup.md) §9 and §12. The only required status check on `main` is `ci-ok` (skipped jobs count as passing); everything else feeds into it or runs on its own schedule.

## Budget rule

- Tooling maintenance has a **ceiling of 12 hours per month** (T0 estimate: 7-13 h).
- Every gate has an owner, a one-line failure runbook and a last-catch date in the table below. Update "last catch" whenever a gate stops a real defect (not a tooling flake).
- A gate with no catch in **six months** is removed or demoted to nightly.
- When the ceiling is exceeded, the most expensive such gate goes first.
- A test that flakes twice in seven days is quarantined with a tracking issue. Jobs are never retried as a whole.

## Gates

Owner is the sole maintainer for every gate during the alpha.

| Gate                                             | Protects                                                                    | Owner (owner) | Runbook (one line)                                                                                | Last catch |
| ------------------------------------------------ | --------------------------------------------------------------------------- | ------------- | ------------------------------------------------------------------------------------------------- | ---------- |
| `ci-ok` (aggregator)                             | Branch protection has one stable required check                             | owner         | Open the failing needed job; a cancelled job means re-run the workflow, not the job.              |            |
| PR title (`amannn/action-semantic-pull-request`) | Squash-merge subject follows Conventional Commits                           | owner         | Rename the PR; allowed types are listed in `AGENTS.md` section 8.                                 |            |
| Workflow lint (actionlint + zizmor)              | Workflow syntax errors, injection and permission mistakes                   | owner         | Run `just lint` locally; fix the finding, never suppress without a comment naming the reason.     |            |
| `lint` (`just lint`)                             | Formatting, typos, clippy, cargo-deny, REUSE, i18n, UI literals, secrets    | owner         | Reproduce with `just lint`; `just fmt` fixes formatting.                                          |            |
| `test` (`just test`)                             | Core vectors, host simulation, WASM, web and Android JVM tests              | owner         | Reproduce with `just test`; for vector diffs bless, then review the diff by hand.                 |            |
| `build` (`just build`)                           | Core, WASM, web and server still build                                      | owner         | Reproduce with `just build`; check the toolchain pins in `rust-toolchain.toml` and `mise.toml`.   |            |
| Browser E2E (`just e2e`)                         | Baby and parent web flows against the local stack                           | owner         | Download the `playwright-e2e-artifacts` artifact; trace viewer shows the failing step.            |            |
| `android-native` / android                       | Native core + app build, Roborazzi screenshots, size and release-APK checks | owner         | Reproduce with `just build-android`; never hand-edit screenshot baselines.                        |            |
| `android-native` / emulator gate (`just gate`)   | Doze, foreground service, alerts, link recovery (AC-1 ... AC-4, AC-10)      | owner         | Download `poc-gate-results`; quarantine a twice-flaking test with an issue, do not retry the job. |            |
| `docker-signaling` / `docker-web`                | Dockerfiles build natively on amd64 and arm64; image publish                | owner         | Reproduce with `docker build -f docker/<image>/Dockerfile .`; merge job needs both digests.       |            |
| `audit` (daily)                                  | New advisories in Rust (`cargo deny`) and npm (`pnpm audit`)                | owner         | Bump or override the dependency; never silence an advisory without an ADR-level reason.           |            |
| Renovate                                         | Dependency freshness within release-age gates                               | owner         | Triage the Dependency Dashboard weekly; merge uniffi and wasm-bindgen PRs by hand.                |            |
