# Design

## Context

See the proposal for the gap. `LinkControllerTest` already has the parts a journey needs: two real cores, `FakeNet` (in-memory relay), fake media and a `unit()` builder. `MainActivity` exposes test tags as resource ids (`testTagsAsResourceId`), so Maestro and `uiautomator` can select by tag. The gate runs its scripts in a runner container inside the emulator image (`scripts/poc/local-gate.sh`).

## Decisions

### 1. Journeys in Robolectric, not on emulators

A journey test composes `GuardedSleepRoot` for one device at a time on that device's real `LinkController`; the two controllers share one `FakeNet`, and the test switches which device it renders (two `ComposeTestRule`s are not needed: the controllers run headless, the UI only reflects one). Time comes from the controllable clock of the fixtures; no sleeps. The fixtures move from `platform/src/test` to a `testFixtures` source set (or a small `:test-fixtures` module) that `app` tests depend on. Pairing by pasted code uses the same path as the user; the camera is out of scope (decoder covered by `QrScanTest`).

### 2. Contract completeness by the compiler

Removing default bodies makes the compiler enforce what TypeScript already enforces on web. Fakes and previews implement the methods explicitly. The writer check is a JVM test over the platform sources: every `AppState`/`ParentMonitorState`/`BabyMonitorState` property must appear as a named argument or `copy(` field in `platform`; an allowlist with a reason covers fields that are intentionally constant.

### 3. Maestro pinned with mise, run in the gate

`maestro` via mise's `github:mobile-dev-inc/maestro` backend, exact version, with a doc-link comment; Java is already pinned. The gate runner container gets the mise install mounted read-only (or installs from the lock into the shared cache), so local and CI use the same binary. `maestro_assert` fails under `GS_STRICT=1` when Maestro is missing.

### 4. Code transfer between emulators

The baby flow taps "Show code as text"; the gate reads the text node by its test tag through `adb shell uiautomator dump` into a file under the gate's output directory, which is removed after the parent flow pastes it (Maestro `inputText` with an env value). The code is a fresh test PSK: the gate registers it with the log canary (`canary-scan.sh`), which then fails the run if it appears in any log. It is never printed by the gate.

### 5. When the heavy tier runs

Nightly, on dispatch, on the `gate` path filter (2 min Doze) and on the release commit, as the gate does today. It stays outside `ci-ok`; the release checklist requires it green.

## Discarded alternatives

- **adb taps only, no Maestro.** Pros: nothing to install. Cons: re-implements waits and selectors in shell, brittle; the existing flows stay dead. Rejected by the owner, 2026-11-14 (introduce Maestro properly).
- **Compose UI tests on emulators for every PR.** Pros: real devices. Cons: minutes per run and KVM on every PR; the Robolectric journeys catch the wiring bugs at a fraction of the cost.
- **Making `FakeAppController` stricter.** Cons: still a second implementation that can drift; the bug was the drift.

## Risks / Trade-offs

- Robolectric journeys cannot prove Doze, notifications over the lock screen or real audio; those stay in the gate.
- The two-emulator journeys need about 4 GB free RAM besides the build; they take the heavy slot.
