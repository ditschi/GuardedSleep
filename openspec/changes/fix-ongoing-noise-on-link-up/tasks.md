# Tasks

Each task is a brief for the `implementer` sub-agent. Branch `fix/noise-alert-on-link-up` off `origin/main`; commit after each task with a Conventional subject. Layer rules: the alert decision lives only in the core (AGENTS.md §5.1); platforms only render it. Copy only in `locales/*.json`.

## 1. Core: announce an ongoing loud state on trust

- [x] 1.1 Core unit test in `crates/guardedsleep-core/src/link.rs` that reproduces the bug: a baby link whose gate turns loud (continuous loud `AudioLevel` for longer than the hold time) before any trusted attempt, then becomes trusted while still loud; assert the parent receives `noise_alert{probe: false}`. It must fail before 1.2.
- [x] 1.2 In the baby branch of `on_trusted`, after `send_missed_events`, send `noise_alert` when the gate is loud (add `NoiseGate::is_loud` with a unit test). Re-bless `guardedsleep-sim` traces only if they change; review the diff by hand.

## 2. Web E2E

- [x] 2.1 Remove the `test.fixme` quarantine (and its comment) from "noise alert screen" in `apps/e2e/tests/31-web-screens-a11y.spec.ts`. Acceptance: `--repeat-each 10 -g "noise alert"` passes, and so does the whole spec file.

## 3. Copy: missed-events notice for a zero-length gap

- [x] 3.1 Add `native.ui.missed.unknownAt` ("We don't know what happened at {time}.") to every locale (machine translated, critical key) and use it on web and Android when start and end format to the same time.

## 4. Checks

- [ ] 4.1 `just test-affected`, `just lint`, `just test`, `just e2e` green; `openspec validate --all --strict` green.
