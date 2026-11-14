# Testing: which layer catches what

Behaviour is proven locally first, then on emulators, then on hardware ([`dev-toolchain`](../../openspec/specs/dev-toolchain/spec.md), "Test layers"). Write the cheapest test that can see the defect. `just test` runs everything except the emulator gate; [`ci-gates.md`](../ci-gates.md) lists what each CI job protects.

| Defect                                                                 | Caught by                                                              | Where                                                           |
| ---------------------------------------------------------------------- | ---------------------------------------------------------------------- | --------------------------------------------------------------- |
| Wrong protocol, pairing, link or escalation decision                   | Rust unit, property and snapshot tests, conformance vectors and traces | `crates/guardedsleep-core`                                      |
| Link recovery, timing, relay behaviour over many seeds                 | Host simulation scenarios (`gs-scenario`), the log canary              | `crates/guardedsleep-sim`                                       |
| A binding exposes the wrong shape                                      | `api.txt` snapshots, vectors replayed through UniFFI and WASM          | `crates/guardedsleep-{ffi,wasm}`, `packages/core-wasm`          |
| A screen renders a state wrongly, a layout breaks in a locale          | Roborazzi screenshots, Compose tests on `FakeAppController`            | `apps/android/app/src/test/.../ui`                              |
| A button does nothing, or a state field is never written               | **UI journeys** through the real controller, and the contract check    | `apps/android/app/src/test/.../journey`                         |
| The platform maps a core event wrongly, a controller rule is violated  | `LinkControllerTest` and the platform tests on the virtual scheduler   | `apps/android/platform/src/test`                                |
| A web flow breaks                                                      | Vitest (logic only) and Playwright with axe against the local stack    | `apps/web`, `apps/e2e`                                          |
| Doze, foreground service, lock-screen alerts, real audio, camera, TURN | The emulator gate, then hardware tests                                 | `just gate`, [`alpha-hardware-test.md`](alpha-hardware-test.md) |

## The Android UI journeys

`JourneyHost` shows two phones side by side in one Robolectric activity (`device_baby`, `device_parent`). Each is `GuardedSleepRoot` on its own real `LinkController` with the real core, built by `TestPhones` in `apps/android/test-fixtures`. Only signaling (an in-memory relay), media, audio, time and system services are fakes. Time is virtual: a `ManualScheduler` runs the cores, the relay and the writers on the test thread, and `step` advances it together with the Compose clock, so a link loss of a minute costs no wall time.

- Select nodes by stable test tag within one device, never by copy text. Sheets and dialogs are windows of their own; `tap`, `awaitShown` and the others find them.
- Journeys read only contract types (`AppController`, `UiContract.kt`); `app/src/test` imports no `uniffi.*` (`BindingSeamTest`).
- The pairing journeys read the code from the baby's code text node and paste it into the parent's field, as a user without a camera does. The camera is out of scope (`QrScanTest` covers the decoder).
- A journey that fails prints the tags on screen on both phones and both link states. Fix the controller when a button does nothing; fix the journey only when the product behaviour was changed on purpose.
- A new contract method needs an implementation in `LinkController` and `FakeAppController` (no default bodies), and a journey that taps it. A new state field needs a platform write that a journey reaches, or an entry in `WriterAllowlist` with the reason (and a backlog entry for a product gap).
- Journeys for behaviour the product does not have yet are written and `@Ignore`d with a pointer to the backlog entry.

What the journeys cannot prove stays with the emulator gate: launching `AlertActivity` from a notification (`showAlert` stands in for it), the foreground service, Doze, real audio, the keystore and libwebrtc.

## When to run the emulator gate

Run `just gate` (build with `just build-android` first; `DOZE_MINUTES=2 just gate` for a quick run) before a PR that touches the foreground service, notification and alert code, link recovery, signaling transports, the media engine or the manifest, and before a release tag. Do not run it for UI-only or core-only changes that the layers above cover. The gate needs KVM and two emulators; the machine runs one gate at a time (`GS_HEAVY_SLOTS`).
