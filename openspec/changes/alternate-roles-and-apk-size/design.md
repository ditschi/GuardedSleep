# Design

## 1. Keeping pairings across role switches

**Option A — one pairing per role (recommended).** The phone stores up to two independent pairings, keyed by role. Night 1 phone A (baby) and phone B (parent) use pairing P1 shown by A; night 2 B shows its own invite once and A scans it (pairing P2). After that both swaps are one tap.

- Pros: no crypto or protocol change; each pairing keeps its direction-bound proof, parent-secret pin and 1 + 1 cap exactly as specified; a forgotten or compromised pairing affects one direction only.
- Cons: the second direction needs one extra QR scan; two PSKs on disk (both Keystore-encrypted).

**Option B — one role-agnostic pairing used in both directions.**

- Pros: one scan total.
- Cons: the parent-secret pin and the baby's pinned parent are directional; reusing one PSK with swapped roles changes the pairing model and needs a security review and core work. Two phones that both pick the same role share a room with no peer.

Recommendation: A.

## 2. What is per role and what is shared

Per role: the pairing record, peer name, snooze end, last test time. Shared: all `SettingsState` fields (already persisted; role-specific ones such as silencing or alert output only apply in their role). The persisted `role` stays as the _session_ role so a process restart during monitoring resumes the running role; a new `defaultRole` (nullable) decides the role at a cold start when monitoring is not active.

## 3. Start and switch flow

- Cold start, monitoring active: resume the session role (unchanged).
- Cold start, monitoring off: role = `defaultRole`; null shows the role screen.
- Role screen: two role cards plus a "Use as default" toggle (off by default). Choosing a card sets the session role and, when toggled, the default.
- "Switch mode" on the home and pairing screens (not on monitor screens, not in the kiosk): clears the session role and shows the role screen; an open invite is cancelled. It never changes the default.
- Settings, Basic: "Start mode" radio group — Ask every time / Baby unit / Parent unit.
- Setup checklist completion stays shared (`onboardingDone`); the checklist rows of the other role stay reachable from Settings.

## 4. Migration

On the first read after the update: if `role` is set and `defaultRole` absent, set `defaultRole = role`, and move the legacy single pairing file to the slot of that role. A failed move keeps the legacy file so the next start retries. New installs start with no default.

## 5. Launcher icon

`res/mipmap-anydpi-v26/ic_launcher.xml` (adaptive: `@color` background, vector foreground inside the 72 dp safe zone, the same vector as `monochrome`) and `ic_launcher_round.xml`; manifest `android:icon="@mipmap/ic_launcher"`, `android:roundIcon="@mipmap/ic_launcher_round"`. minSdk is 26, so no PNG fallbacks are needed. Verify with `aapt2 dump badging`.

## 6. APK size

Measured 2026-11-11 (debug APK, 53.5 MB): native libraries 34.8 MB stored uncompressed (`x86_64` 18.4 MB, `arm64-v8a` 16.4 MB), dex 17.6 MB compressed.

**Option A — compressed native libs + drop `x86_64` from the published dev APK (recommended).** `packaging.jniLibs.useLegacyPackaging = true` in `app` (libraries compress to about 40 %; the installer extracts them, so the installed size grows by the library size). The `dev-apk` job deletes `lib/x86_64/*` from the gate-tested APK before `zipalign` and `apksigner`. Expected dev APK about 25 MB. The gate keeps the universal APK.

- Pros: no gate or path changes; the phone runs the exact tested code.
- Cons: larger installed footprint; the published APK differs from the tested one by an unused ABI.

**Option B — Gradle ABI splits.** Pros: clean per-ABI outputs. Cons: renames the APK outputs used by gate, size and release scripts and the workflows.

**Option C — R8 on debug.** Pros: dex shrinks by more than half. Cons: slower builds, obfuscated stacks in the test build, keep rules then matter for the gate too.

Recommendation: A now; B with the release pipeline (`add-android-release`), C not planned.

## Discarded alternatives

- Option 1B (role-agnostic pairing) and options 6B/6C above.
