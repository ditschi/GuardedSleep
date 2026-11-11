# Proposal

## Why

Owner request (2026-11-11), following `alternate-roles-and-apk-size`:

- Parents alternate nights, and both want to monitor at the same time when both are at home: **one baby unit, two parent units**.
- One phone, many situations: an old phone as the fixed baby unit at home, the partner's phone as baby unit when travelling. Switching between saved connections should be one tap, and each connection should have a name the user chooses instead of the peer's device name.

Multi-parent is `DEFERRED` (Phase 2) only for alpha scope; no technical rejection is recorded (ADR-018, ADR-009, `migrate-pairing`). The owner moves it into this change with a cap of **2 parent units** per baby unit (owner, 2026-11-11). This changes a prior decision and the security model of the baby unit (several trusted parents), so the Critical Review Gate applies.

## What Changes

- **One pairing per parent device.** A baby unit keeps up to 2 saved parent pairings; each is an ordinary 1 baby + 1 parent pairing with its own PSK, room, pin and invite. The pairing protocol, the crypto and the relay stay unchanged (`SIGNALING_MAX_PARENTS` stays 1). Removing a parent device is forgetting its pairing; nobody else re-pairs.
- **Baby hub in the core.** A new sans-IO core type owns the baby's links (at most 2) and the rules between them, so no cross-link safety logic lives in a platform:
  - **Join window:** for 2 minutes after monitoring starts (and 2 minutes after the first parent verifies, whichever is later) every saved pairing waits for its parent. After the window, links whose parent has not joined close their mailboxes.
  - **Grace period:** when a joined parent drops while another parent is still connected, its link keeps recovering for 5 minutes, then closes. A deliberate stop closes it at once.
  - **Last parent never dropped:** while no other parent is connected, the baby keeps every link of this session retrying, as today.
  - **Rejoin:** an adult on the unlocked baby unit, or a connected parent, can reopen the join window for 2 minutes.
- **Microphone per parent.** The audio of each parent link is enabled separately; the core's rule "audio only for a verified attempt, only on demand" holds per link. Listening, snooze, escalation and alarms stay per parent.
- **Shared noise acknowledgment.** When one parent acknowledges a noise alert, the baby tells the other parent, which stops ringing and shows "Acknowledged on the other parent device". Connection alarms are never shared.
- **Other parent status.** The parent screen shows whether the other parent device is connected; the baby's touch-wake status names how many parents are connected.
- **Named connections and saved lists.** Every saved pairing has a local name (default: the peer's device name), kept only on the device, encrypted with the pairing. Parent role: up to 2 saved baby pairings with a default and a per-session choice; the list appears only when there is more than one.
- Out of scope (stay `DEFERRED` or `IDEA`): more than 2 parents, join requests without a QR scan, the F-1a "you are now primary" failover message, a web baby unit with two parents (the web client stays one pairing; as parent it shows the shared acknowledgment and other-parent status through the core).

Persona impact: the two-parent household gets simultaneous monitoring and one-tap switching; single-parent use is unchanged (one saved pairing shows today's screens).

## Capabilities

### New Capabilities

### Modified Capabilities

- `pairing`: several saved pairings per role (cap 2), local connection names, forget per pairing.
- `link-supervision`: baby hub with join window, grace period, rejoin; "never give up" scoped to the last connected parent.
- `parent-alerts`: shared noise acknowledgment.
- `connection-status`: other-parent status on the parent, parent count on the baby.
- `device-setup`: connection choice and default connection per role, "Add parent device" on the baby unit.

## Impact

`crates/guardedsleep-core` (hub type, additive protocol v3 messages and status field, vectors and traces), `crates/guardedsleep-ffi` and `crates/guardedsleep-wasm` (API snapshots), `crates/guardedsleep-sim` (scenarios), `apps/android` (multi-record pairing store, one media engine with one track per parent link, several link runners under one hub, LAN listener with one admission key per saved pairing, UI), `apps/web` (parent display only), `locales/*.json`, emulator gate (three-device scenario), `AGENTS.md` §9 row and `docs/backlog.md` (multi-parent item split). Depends on `alternate-roles-and-apk-size` being archived first; coordinate with `quiet-link-no-rtp` (per-sender detach) and `add-offline-lan` (listener admission keys).
