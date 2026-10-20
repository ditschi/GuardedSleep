# Test data schemas: traces and device reports

`STATUS: UNDER DISCUSSION` — sketch accompanying [ADR-016](../decisions/ADR-016-test-strategy-local-virtual-physical.md) (PROPOSED). Field names and versions may change until ADR-016 is DECIDED. Nothing here is implemented yet.

Two artefacts tie the test stages together:

- **Trace** (`gs-trace/1`): the ordered inputs given to a core `Session` and the actions it returned. Used on the host (golden vectors, simulation output). A device-side recorder is `STATUS: DEFERRED` (ADR-016 §9).
- **Device report** (`gs-device-report/1`): the result of running capability-tagged scenarios on one device with the physical runner. Used as release evidence, in particular for the mandatory soak gate (ADR-016 §5).

Both are plain JSON, UTF-8, versioned by a `schema` field, and contain no media. Both use one privacy vocabulary: `privacy: "local" | "shareable"`. `local` artefacts may contain secrets, are written only to app-private or ignored build directories, and are never exported automatically. `shareable` artefacts are built by allowlist (below) and pass the lint step.

## 1. Trace (`gs-trace/1`)

### 1.1 Relation to the existing conformance vectors

`crates/guardedsleep-core/vectors/conformance.json` has `kind: "session"` cases: `config` plus `steps` of `{ now, input }`, evaluated by `conformance::evaluate`, with the expected actions as one case-level array (one `Vec<Action>` per step). A trace is a **superset with a converter**: the existing cases stay valid and unchanged; a converter turns a trace into a `session` case (dropping `n`, `note`, `final`) and back (adding defaults). This avoids migrating the existing cases and keeps one runner for vectors, simulation output and future recordings.

### 1.2 Shape

```json
{
  "schema": "gs-trace/1",
  "id": "sim-heartbeat-loss-0007",
  "class": "simulated",
  "privacy": "shareable",
  "origin": { "core_version": "0.3.0", "harness": "sim/1" },
  "seed": 7,
  "config": {
    "role": "parent",
    "room_id": "room-1",
    "psk_b64url": "<synthetic test psk>",
    "heartbeat_interval_ms": 10000,
    "heartbeat_stale_ms": 50000,
    "pairing_timeout_ms": 15000,
    "noise_threshold": 0.12,
    "noise_hold_ms": 800
  },
  "steps": [
    { "n": 0, "now": 1000, "input": { "type": "signaling_open" } },
    { "n": 1, "now": 61000, "input": { "type": "tick" } }
  ],
  "expect": [
    [{ "type": "send_signaling", "text": "…" }],
    [{ "type": "emit", "event": { "type": "link_lost", "reason": "heartbeat_stale" } }]
  ],
  "final": { "state": "verified" }
}
```

| Field     | Meaning                                                                                                                                                                                                                           |
| --------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `schema`  | Format version. Consumers reject unknown major versions.                                                                                                                                                                          |
| `class`   | `golden` (hand-written or blessed, committed), `simulated` (output of the host simulation, committed when it pins a regression). `recorded` is reserved for the deferred device recorder.                                         |
| `privacy` | `local` or `shareable` (see above). Committed traces are always `shareable`.                                                                                                                                                      |
| `seed`    | Simulation seed; `null` for golden traces.                                                                                                                                                                                        |
| `config`  | Exactly `SessionConfig` as serialised by the core today. The PSK is always a **synthetic test value** in any committed or shareable trace.                                                                                        |
| `steps`   | `n` is a monotonic step index so omissions are visible. `now` is the **monotonic** millisecond clock supplied by the platform (not wall-clock); the simulation may include time jumps (Doze/suspend). `input` is exactly `Input`. |
| `expect`  | One `Vec<Action>` per step, in the same layout as the existing `session` vectors.                                                                                                                                                 |
| `final`   | Optional end-state assertions (`state`, emitted-event counts).                                                                                                                                                                    |

Audio is never stored: `audio_level` inputs carry only the RMS float the core already uses.

### 1.3 Replay and why there is no "oracle"

A binding replays a trace by constructing `Session::new(config)`, calling `handle(now, input)` for each step and comparing the result with `expect` (JSON equality). Kotlin (UniFFI), Swift and WASM must produce identical results on all committed traces (this mainly proves marshalling; independent cross-checks come from the TypeScript characterization vectors).

The core computes `pair_proof` itself (`compute_pair_proof_mac(psk, room_id, local_sdp, remote_sdp)`), and its inputs are embedded as JSON strings inside `SignalingMessage.text` and `DataChannelMessage.text`. Therefore **a trace from a real pairing cannot be both shareable and replayable**: redacting the room id, SDP or PSK changes the MAC input. The earlier idea of a core path that accepts a recorded MAC outcome ("oracle") is **rejected**: it would be an authentication bypass if it ever shipped. Instead, traces that must be shareable are produced with a **synthetic PSK, synthetic room id and synthetic SDP fingerprints from the start** (the simulation does this by construction; a future device recorder would have to run in a mode that does the same), and the replay driver recomputes every MAC. Real pairing material is never recorded.

### 1.4 Redaction: allowlist, not blacklist (for any `shareable` artefact)

The exporter serialises **only known fields** and drops everything else; unknown fields are an error, not a pass-through. In addition, `gs-trace lint` (run in CI and before attaching a trace to an issue) rejects any shareable trace matching: IPv4/IPv6 literals, `a=fingerprint` with a non-synthetic value, `a=candidate`, `ice-ufrag`/`ice-pwd`, TURN `credential`/`username`, URLs of signaling hosts, long base64url strings that are not the declared synthetic PSK, and absolute timestamps. Items the exporter must never emit: real PSKs, real room/client/peer ids, real SDP bodies or ICE candidates (including mDNS names), `pair_proof` MACs and nonces from real sessions, signaling URL/host, server free-text error messages, absolute wall-clock times (relative `now` only), device names, IPs, SSIDs.

Constraint on consistency: pseudonymised ids (`room-1`, `c1`, `c2`) must be re-encoded consistently inside every embedded JSON string, because the core cross-checks `welcome`/`hello` against the configured room id.

## 2. Simulation scenario (`gs-scenario/1`, host only) — minimal

`STATUS: DEFERRED` for anything beyond the AC-9 slice and the AC-10 link-recovery scenarios (ADR-016 §9). The slice needs only: two peers, a seed and a short timeline.

```json
{
  "schema": "gs-scenario/1",
  "id": "heartbeat-loss-parent",
  "seed": 7,
  "peers": [
    { "id": "baby", "role": "baby" },
    { "id": "parent", "role": "parent" }
  ],
  "timeline": [
    { "at": 0, "do": "start" },
    { "at": 30000, "do": "drop_data", "from": "baby" },
    { "at": 90000, "do": "end" }
  ],
  "assert": [{ "peer": "parent", "event": "link_lost", "within_ms": [60000, 90000] }]
}
```

Initial verbs: `start`, `end`, `drop_data`, `delay`, `duplicate`, `reorder`, `time_jump`. Link-recovery verbs (needed for AC-10): `network_down` / `network_up` per peer, `signaling_down` / `signaling_up`, `path_switch`, `restart_process`. They are specified now and run test-first (expected to fail against today's core, which ends every session at `Failed` on peer-connection failure) until the link-recovery design is implemented. Because 8 h of 10 Hz audio levels and 500 ms ticks would be ~350k steps, simulated long runs skip no-op ticks and keep periodic time anchors.

## 3. Device report (`gs-device-report/1`)

Produced by `scripts/device-test.sh` on the machine running adb. It is local; the tool never uploads.

```json
{
  "schema": "gs-device-report/1",
  "id": "20261019-7f3a",
  "privacy": "local",
  "date": "2026-10-19",
  "tool": { "name": "device-test", "version": "0.1.0" },
  "app": { "version": "0.3.0+142", "build": "debug", "core_version": "0.3.0" },
  "device": {
    "kind": "physical",
    "manufacturer": "motorola",
    "model": "moto g(7) plus",
    "api": 29,
    "android": "10",
    "rom_family": "stock",
    "rooted": false,
    "abi": "arm64-v8a",
    "ram_mb_bucket": "4096"
  },
  "environment": {
    "network": "wifi",
    "charging_at_start": false,
    "second_device_role": "parent-web"
  },
  "scenarios": [
    {
      "id": "S-pair-qr",
      "requires": ["needs-second-device"],
      "status": "pass",
      "duration_ms": 8210
    },
    {
      "id": "S-listen-audible",
      "requires": ["needs-real-mic", "needs-second-device"],
      "status": "skipped",
      "reason": "no microphone input available"
    },
    {
      "id": "S-soak-screen-off",
      "requires": ["soak", "needs-second-device"],
      "status": "pass",
      "metrics": {
        "duration_ms": 28800000,
        "heartbeat": {
          "expected": 2880,
          "received": 2873,
          "max_gap_ms": 41000,
          "gaps_over_stale_ms": 0
        },
        "battery_pct": { "start": 100, "end": 74 },
        "fgs_alive_samples": { "total": 480, "alive": 480 }
      }
    }
  ],
  "summary": { "pass": 2, "fail": 0, "skipped": 1, "error": 0 }
}
```

| Field            | Notes                                                                                                                                                                                                                         |
| ---------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `privacy`        | A runner report is `local` (maintainer's own devices, includes `model`). Any future shareable/community variant uses the allowlist model of §1.4, rounds nothing finer than the date, and makes the model name opt-in.        |
| `date`           | Date only, no time of day.                                                                                                                                                                                                    |
| `device.kind`    | `physical`, `emulator`, `simulator`, `cloud-farm` (optional per ADR-016 §7).                                                                                                                                                  |
| `rom_family`     | `stock`, `oem-skin` (MIUI/One UI/ColorOS…), `aosp-like`, `custom`. Rooted/custom devices are labelled because they under-represent OEM battery managers; custom build strings can be unique, so shareable variants omit them. |
| `scenarios[].id` | Stable ids shared by the checklist and the runner, e.g. `S-pair-qr`, `S-listen-audible`, `S-alert-heads-up`, `S-soak-screen-off`, `S-call-interrupt`, `S-nsd-lan`.                                                            |
| `requires`       | Capability tags: `needs-second-device`, `needs-real-mic`, `needs-sim`, `needs-wifi-lan`, `soak`, `needs-bluetooth`. Availability is derived from the device profile.                                                          |
| `status`         | `pass`, `fail`, `skipped` (precondition missing, with `reason`), `error` (runner problem, not an app verdict). `skipped` never counts as pass or as failure.                                                                  |
| soak metrics     | Fixed names so reports are comparable. `heartbeat.*` is measured from the `Heartbeat` events seen on the parent side; `fgs_alive_samples` from periodic `dumpsys activity services` checks.                                   |
| `dumpsys` data   | Only through a per-service allowlist plus a scrub pass; never raw excerpts (package lists, account names, SSIDs, IPs and identifiers can appear in dumps).                                                                    |

The in-app self-test (ADR-010) emitting this schema and any community-report flow are `STATUS: DEFERRED` (ADR-016 §9). When built, they need: a human-readable summary shown before any raw data, an exact preview of what is shared, export only via clipboard or an explicit share sheet (never an upload), the privacy-policy entry, English-only field names (report keys for maintainers) with translated UI copy, and Security/Privacy sign-off on the model-name default.

## 4. Open questions

- `STATUS: UNDER DISCUSSION` **Synthetic-PSK recorder mode.** Design a device-side recorder that only runs with synthetic pairing material, or drop device recording altogether in favour of host simulation plus soak reports. Decide when the first field bug needs it.
- `STATUS: IDEA` Generate JSON Schema from the Rust types (e.g. via `schemars`) so Kotlin, Swift, TypeScript and the runner validate against one definition.
- `STATUS: IDEA` Known-device list under `docs/` generated from accepted reports.
- `STATUS: IDEA` Trace compression and size caps for long runs.
