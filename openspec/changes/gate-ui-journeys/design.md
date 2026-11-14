# Design

## Context

`scripts/poc/local-gate.sh` runs two emulators, `gs-gate-signaling` and coturn as containers in one network namespace, and `gate.sh` in a runner container (`eclipse-temurin:21-jdk-jammy`, Java present) that mounts only the repository and `/root/.android`. CI uploads `target/poc-gate` as an artifact. `canary-scan.sh` knows only the fixed `GATE_PSK`/`GATE_PSK_OTHER` and runs on the host after the runner exits. `MainActivity` exposes test tags as resource ids.

## Decisions

### 1. Maestro via mise, mounted into the runner (review I7, M4)

- `maestro` from mise's `github:mobile-dev-inc/maestro` backend, exact version, doc-link comment; task 1 verifies the `cli-` tag prefix handling and that `mise.lock` holds entries for every CI platform (`MISE_LOCKED=1`).
- `mise.toml [env]`: `MAESTRO_CLI_NO_ANALYTICS=1`, `MAESTRO_CLI_ANALYTICS_NOTIFICATION_DISABLED=true` and the update-check opt-out (AGENTS.md §7.5 spirit: no phoning home), `JAVA_OPTS=-Xmx1g` for the gate run.
- `local-gate.sh` mounts `$(mise where maestro)` read-only at `/opt/maestro`, puts it on `PATH` and forwards the env with `docker run -e`. Maestro's debug output stays inside the `--rm` container; `--debug-output` never points into `target/poc-gate`.
- CI installs Maestro only in the gate job (`setup-mise` with `install_args`), not in every job.

### 2. Hermetic environment through the UI (review C1)

Before the baby shows its code, the flows open Settings → Advanced on both devices: own relay `ws://10.0.2.2:$SIGNAL_PORT`, project server off, TURN `turn:10.0.2.2:3478` with the gate's test credentials. This also covers that UI. `PairingPayload.mailboxesFor` accepts the private `ws://` relay, so the invite carries only the gate server. The gate asserts from the `gs-gate-signaling` log that two members joined a room, so a silent fallback to the project server would fail. Rejected: a debug broadcast setting URLs (shortcut past the UI under test).

### 3. Code transfer without a published file (review C3)

- The baby flow taps "Show code as text"; the gate reads the node by its tag with `adb exec-out uiautomator dump /dev/tty` into a shell variable (no `/sdcard/window_dump.xml`, nothing in `target/poc-gate`).
- The value goes to the parent flow as a Maestro env value for `inputText` into `parent_pair_paste_field`.
- For the canary it is written to `target/poc-gate-secret/code` (directory 0700, file 0600), outside the uploaded path; `canary-scan.sh` gains `--extra-canary-file`, deriving raw, b64url and hex forms of the PSK from the invite as for the fixed PSKs; `local-gate.sh` deletes the directory in its EXIT trap after the canary.
- If the JUnit XML echoes the `inputText` value on failure, the canary fails the run: safe; the flow avoids echoing by not logging steps with values.
- An ephemeral test PSK on throwaway emulators is not user pairing material (§7.2); a short-lived file outside the artifact is acceptable, a file in the artifact is not. Rejected: a debug seam forcing a known PSK (would ship in the tester `dev-apk`).

### 4. System UI and emulator quirks (review I8)

Before the flows: `install -g`, `dumpsys deviceidle whitelist +pkg`, the existing appops; system dialogs only through `android:id/button1`. The emulator microphone delivers digital silence, so the baby's device check shows "not recommended" there; the flow asserts a result exists, not its value.

### 5. Placement and reporting (review M2, M3)

A new phase `AC-12 UI journey` after AC-4 with `pm clear` on both devices before and after, so its saved pairing cannot leak into the debug-launch scenarios. A PASS/FAIL line in `summary.txt`; `GATE_ONLY=ui` for fast loops. Without Maestro: `SKIPPED: gate-ui needs maestro`, failure under `GS_STRICT=1`.

### 6. Budget (review I10, I11)

Task 3 measures one local run (peak RSS, duration) and records it here. A 14 GB machine runs the gate with `GS_MEDIUM_SLOTS=1` (note in `development.md`). Scope stays at one journey plus the existing asserts; Renovate bumps Maestro monthly; the gate row in `docs/ci-gates.md` gets a separate last-catch date for the UI journey so the six-month rule can demote it alone.

## Discarded alternatives

- **adb taps only, no Maestro.** Pros: nothing to install. Cons: waits and selectors re-implemented in shell; the existing flows stay dead. Rejected by the owner, 2026-11-14.
- **Many journeys on emulators.** Cons: the most flake-prone gate class against a 12 h/month budget; breadth lives in the Robolectric journeys of `ui-journey-tests`.
- **Debug broadcast for the test environment.** Cons: bypasses the Settings UI the journey should cover.
- **Known-PSK debug seam.** Cons: ships in the debug `dev-apk` testers install.

## Risks / Trade-offs

- Emulator UI flows flake more than JVM tests; first contact will expose wrong ids in the never-run assertion flows (task 2 fixes them).
- Maestro's JVM adds about 1 GB on top of two emulators (about 6 GB).
