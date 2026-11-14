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

## Implementation findings (2026-11-14)

Recorded while implementing tasks 1 to 5; they change what the change delivers today.

### 7. Blocker: the UI journey cannot pair on two emulators without relay-only media

- **What works through the real UI** (Maestro 2.12.1, local runs on two emulators): fresh app, role choice, onboarding, Settings -> Advanced (the project server entry is removed with its Remove button, there is no "off" switch in this build; own relay `ws://10.0.2.2:$SIGNAL_PORT` added; TURN `turn:10.0.2.2:3478` poc/poc saved), the baby's code shown as text and read with `uiautomator` into a shell variable, pasted into the parent. The `gs-gate-signaling` log showed `joined as baby` and `joined as parent`, so signaling met on the gate's own server (hermetic, no project server).
- **What fails:** the peer connection. Both emulators have the address 10.0.2.15, so ICE cannot use host candidates, and the pairing ends in `peer_connection_failed`/`attempt_timeout`. Control experiment: `DebugLaunchActivity` with `relay_only=false` and the same TURN fails the same way, so this is the emulator topology, not the journey. The existing gate scenarios pass only because `DebugLaunchActivity` sets `relay_only=true`.
- **A shortcut was tried and dropped.** A debug-only extra on `DebugLaunchActivity` that sets only `relay_only` (patch kept out of the tree) made the journey get through pairing and verification, so the rest of the plan is sound. It is a debug seam of the kind §2 and §3 reject, so it is not in this change. The planner decides: (a) accept that one narrow seam (relay-only only, no URLs, TURN or PSK; debug source set), (b) add a user-visible "relay only" setting (also useful on real networks; needs its own change), or (c) leave the journey disabled.
- **State of the gate:** the AC-12 phase and its flows are in place but disabled by default (summary line `NOTE AC-12 — UI journey disabled`); `GATE_UI_JOURNEY=1` or `GATE_ONLY=ui` runs them. Maestro runs the existing assertion flows (AC-1, AC-3, AC-4), whose ids were wrong and are fixed. The "project server off" step is a removal of the list entry because this build has no off switch yet (`alpha-connection-model` task 5.3).
- **Emulator quirks found:** an "isn't responding" dialog covers the app when two emulators and Maestro share a busy 8-core host (`hide_error_dialogs=1` plus `dismiss-system-dialogs.yaml`); `adb install` can fail with Broken pipe right after boot (retried); uiautomator quotes a text attribute containing `"` with single quotes (the code reader handles both); scrolling lists need `visibilityPercentage: 50` and a long, fast scroll to the top on the parent's longer Settings screen.
