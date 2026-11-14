# Design

## Context

`GuardedSleepRoot(controller)` needs only an `AppController`. `LinkController` already runs under plain JUnit with its I/O behind seams; `LinkControllerTest` has `FakeNet` (an in-memory relay on its own thread), fake media and a `unit()` builder, but uses `RealClock`, polling `await` and bare sleeps, and `LinkController` hard-codes `turnIo` (Critical Review C2). No Robolectric test loads the real core yet, and `app` tests have no JNA wiring (I1). `app` must not depend on `core-bindings` (AGENTS.md §5.1, I2).

## Decisions

### 1. Two devices side by side on one virtual clock

- **Time and threads (review C2, option a):** a `ManualScheduler` implements `ScheduledExecutorService` and `Clock`, single-threaded, driven from the test thread. It is injected as the core executor (`newExecutor`), into the relay fake, and into a new `turnIo` seam (and any other executor `LinkController` creates). Compose runs with `mainClock.autoAdvance = false`; a step advances the scheduler and the Compose clock together until idle. Deterministic, fast; link loss after 60 s costs no wall time.
- **Two roots (review I3):** the test hosts two `ComposeView`s in one activity, each wrapped in a root tagged `device_baby` / `device_parent`, each on its own controller. Steps select with `hasAnyAncestor(hasTestTag("device_parent"))`. No controller swapping, so UI-local state (overlays, half-typed fields, dialogs) behaves as on two phones.
- **Fixtures module (review I2):** a `:test-fixtures` module (not AGP `testFixtures`, still experimental) depends on `platform` and `core-bindings`; its public API returns only `AppController` and contract types (`TestPhones.create()`, `phones.baby`, `phones.parent`, `phones.advance(ms)`, `phones.injectNoise()`, `phones.dropNetwork()`), so `app/src/test` never sees uniffi types. `LinkControllerTest` moves onto it too.
- **Real core under Robolectric (review I1):** task 2.0 is a spike: one Robolectric test in `app` creates two real cores and pairs them, with the Gradle wiring (`jna.library.path` to the host core build, the desktop JNA artifact, as `platform` does). Abort criterion: if JNA cannot load in the Robolectric sandbox within a day, the journeys run as plain JVM tests on the controllers plus Compose checks of the contract state; the design is revisited before continuing.
- **Pairing path:** the baby's code is read from the code node and typed into the parent's paste field, the path a user takes when not scanning; the camera stays out of scope (decoder: `QrScanTest`).
- **Log check (review M7):** the fixtures pair with a fixed test PSK and assert in teardown that `ShadowLog` holds no raw, b64url or hex form of it.

### 2. Contract completeness

- No default bodies in the Kotlin `AppController`; fakes and previews implement every method explicitly.
- **Behavioural writer check (review I4):** a recorder subscribes to `app`, `parent`, `baby`, `settings` and `pairing` of both devices during every journey and collects which leaf fields (nested types included) ever left their default. A final test fails for any field never seen non-default, unless it is on an allowlist with a reason. This catches "written, but never to a real value", which a source scan cannot.

### 3. Exposed product gaps (review I5)

What the test answer does, whether the first-run coach exists, and what "Test alert" in Settings does while not monitoring are not specified (`parent-alerts` only says the parent asks; the coach is only in non-normative `alpha-ux-flows.md`). This change does not invent behaviour: `LinkController` implements them as explicit, commented no-ops pointing at backlog entries, the fields go on the allowlist with that pointer, and the journeys for them are written but `@Ignore`d with the same pointer. A separate fix change specifies and builds them after the owner decides.

## Discarded alternatives

- **Real threads with `waitUntil` (review C2, option b).** Pros: no new scheduler. Cons: wall-clock cost (link loss alone tens of seconds), flakes under load, conflicts with the 90 s budget.
- **One composition with a swapped controller.** Cons: leaks or loses UI-local state; `setContent` once per rule.
- **Source-scan writer test.** Cons: false passes on common names, positional constructors, dead code; blind to "never non-default".
- **Making `FakeAppController` stricter.** Cons: still a second implementation that can drift; the bug was the drift.
- **Compose UI tests on emulators for every PR.** Cons: minutes per run and KVM on every PR; covered cheaper here, devices in `gate-ui-journeys`.

## Risks / Trade-offs

- Robolectric cannot prove Doze, lock-screen notifications, real audio or the `AppGraph` wiring in `MainActivity`; a Robolectric smoke test builds `AppGraph` with seams faked (review M1); the rest stays in the gate.
- The virtual scheduler must cover every executor `LinkController` uses; a missed one shows up as a hanging journey, caught by a per-journey timeout.
