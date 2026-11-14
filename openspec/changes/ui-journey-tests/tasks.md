# Tasks

Branch `test/ui-journey-tests` from `origin/main`; the pairing journeys need ditschi/GuardedSleep#61 (parent pairing): stack on `fix/parent-pairing-entry` until it merges, then rebase onto `origin/main`. Commit after each task; `just test-affected` after each task.

## 1. Contract completeness

- [x] 1.1 Remove the default bodies from the Kotlin `AppController`; implement every method in `LinkController` (`reportTestResult`, `dismissCoach` as explicit, commented no-ops pointing at the backlog entries of task 4.1), `FakeAppController` and previews. Verify: build.

## 2. Journey harness

- [x] 2.0 Spike: a Robolectric test in `app` creates two real cores and pairs them; Gradle wiring for JNA and the host core in `app` tests. Abort criterion in design §1. Verify: the spike test passes in `just test`.
- [x] 2.1 `ManualScheduler` (executor + clock) and seams for every executor in `LinkController` (core, `turnIo`, device check, any other). Verify: `LinkControllerTest` runs on it without sleeps.
- [x] 2.2 `:test-fixtures` module: relay fake on the scheduler, fake media, `TestPhones` API with contract types only; `LinkControllerTest` migrated. Verify: `just test`; `app/src/test` imports no `uniffi.*`.
- [x] 2.3 Two-device host: two `ComposeView`s with `device_baby`/`device_parent` roots, step helpers advancing scheduler and Compose clock, per-journey timeout, `ShadowLog` PSK check in teardown. Verify: a smoke journey (both roles chosen) passes.

## 3. Journeys and the writer check

- [ ] 3.1 Journeys: first run baby and parent with pairing by pasted code; start and stop on both sides; saved connections, default and automatic start, "Choose another connection"; "Pair again"; noise alert with stop sound, snooze and acknowledge; kiosk lock and unlock; link loss and recovery; device check result; switch mode. Verify: `just test`, total under 90 s; each journey fails when its controller method is stubbed out (checked once by hand, noted in the PR).
- [ ] 3.2 Test answer, coach and Settings test alert journeys written and `@Ignore`d with the backlog pointer.
- [ ] 3.3 State recorder and the final writer check with allowlist. Verify: fails on a planted unwritten field.
- [ ] 3.4 `AppGraph` smoke test with seams faked. Verify: `just test`.

## 4. Docs

- [ ] 4.1 `docs/backlog.md`: `IDEA` entries for the test answer, the first-run coach and the Settings test alert while not monitoring (owner to decide). `docs/ci-gates.md` test job row (journeys, contract check). `docs/contributing` test guide: which layer catches what. Verify: `just lint-text`.
