import type { DeviceStatus, LinkEvent, LinkState, LinkStatus } from "@guardedsleep/core-wasm";
import { beforeEach, describe, expect, it } from "vitest";
import type { ParentMonitorState } from "../contract";
import type { AlertSink, LinkSnapshot, NoiseKind, Notice } from "./alert-sink";
import { ParentModel, type ParentEffects } from "./parent-model";
import { FakeTime } from "./test-support";

const status = (state: LinkState, over: Partial<LinkStatus> = {}): LinkStatus => ({
  state,
  level: 0,
  key: null,
  cause: null,
  acknowledged_until: null,
  last_alive_ms: null,
  ...over,
});

class Sink implements AlertSink {
  log: string[] = [];
  snapshots: LinkSnapshot[] = [];
  notices: Notice[] = [];
  noise: Array<[NoiseKind, boolean, boolean]> = [];
  linkState(s: LinkSnapshot): void {
    this.snapshots.push(s);
    this.log.push("linkState");
  }
  linkLost(downMs: number): void {
    this.log.push(`linkLost:${downMs}`);
  }
  noiseAlert(kind: NoiseKind, probe: boolean, snoozed: boolean): void {
    this.noise.push([kind, probe, snoozed]);
    this.log.push("noiseAlert");
  }
  noiseEnded(): void {
    this.log.push("noiseEnded");
  }
  stopNoiseSound(): void {
    this.log.push("stopNoiseSound");
  }
  notice(n: Notice): void {
    this.notices.push(n);
    this.log.push(`notice:${n.type}`);
  }
  clear(): void {
    this.log.push("clear");
  }
}

class Effects implements ParentEffects {
  log: string[] = [];
  muteRemote(muted: boolean): void {
    this.log.push(`mute:${muted}`);
  }
  listenStopped(): void {
    this.log.push("listenStopped");
  }
  testAlertArrived(wallMs: number): void {
    this.log.push(`test:${wallMs}`);
  }
  snoozeEnded(): void {
    this.log.push("snoozeEnded");
  }
}

function setup(threshold = 0.12) {
  const time = new FakeTime();
  const sink = new Sink();
  const effects = new Effects();
  let state: ParentMonitorState | null = null;
  const model = new ParentModel(
    time,
    sink,
    effects,
    () => threshold,
    (s) => (state = s),
  );
  /** Moves the clock and delivers the elapsed ticks the controller would deliver. */
  const advance = (ms: number): void => {
    for (let left = ms; left > 0; left -= 250) {
      time.advance(Math.min(250, left));
      model.onTick(time.elapsedMs());
    }
  };
  const connect = (): void => model.onStatus(status("connected"), time.elapsedMs());
  const event = (e: LinkEvent): void => model.onEvent(e, time.elapsedMs());
  return {
    time,
    sink,
    effects,
    model,
    advance,
    connect,
    event,
    get state(): ParentMonitorState {
      if (!state) throw new Error("nothing published yet");
      return state;
    },
  };
}

describe("status mapping onto the link ui", () => {
  const cases: Array<[string, LinkStatus, Record<string, unknown>]> = [
    ["connecting escalates", status("connecting", { level: 1 }), { phase: "connecting", level: 1 }],
    [
      "reconnecting keeps its level and cause",
      status("reconnecting", { level: 2, cause: "server_unreachable", last_alive_ms: 1500 }),
      {
        phase: "reconnecting",
        level: 2,
        causeHint: "server_unreachable",
        lastAliveElapsedMs: 1500,
      },
    ],
    [
      "peer_absent cause",
      status("connecting", { level: 2, cause: "peer_absent" }),
      { causeHint: "peer_absent" },
    ],
    [
      "media_blocked cause",
      status("reconnecting", { level: 2, cause: "media_blocked" }),
      { causeHint: "media_blocked" },
    ],
    ["connected has level 0", status("connected", { level: 3 }), { phase: "connected", level: 0 }],
    [
      "peer_stopped",
      status("peer_stopped", { last_alive_ms: 9 }),
      { phase: "peer_stopped", level: 0, lastAliveElapsedMs: 9 },
    ],
    ["stopped", status("stopped"), { phase: "stopped" }],
    [
      "failed carries its message key",
      status("failed_needs_user", { key: "pairing_key_mismatch" }),
      { phase: "failed_needs_user", failureKey: "pairing_key_mismatch" },
    ],
    [
      "update_required as a sticky cause is a failure key while reconnecting",
      status("reconnecting", { level: 1, cause: "update_required" }),
      { failureKey: "update_required", causeHint: null },
    ],
    [
      "acknowledged until",
      status("reconnecting", { level: 2, acknowledged_until: 5_000 }),
      { acknowledgedUntilElapsedMs: 5_000 },
    ],
  ];
  it.each(cases)("%s", (_name, input, expected) => {
    const t = setup();
    t.model.onStatus(input, 0);
    expect(t.state.link).toMatchObject(expected);
  });

  it("tells the alert sink the snapshot only when it changed", () => {
    const t = setup();
    t.model.onStatus(status("reconnecting", { level: 2 }), 0);
    t.model.onStatus(status("reconnecting", { level: 2 }), 100);
    t.model.onStatus(status("reconnecting", { level: 3 }), 200);
    expect(t.sink.snapshots.map((s) => s.level)).toEqual([2, 3]);
    expect(t.sink.snapshots[0]).toMatchObject({ phase: "reconnecting", wasConnected: false });
  });

  it("remembers that the link worked once", () => {
    const t = setup();
    t.connect();
    t.model.onStatus(status("failed_needs_user", { key: "pairing_key_mismatch" }), 10);
    expect(t.sink.snapshots.at(-1)).toMatchObject({
      phase: "failed_needs_user",
      wasConnected: true,
    });
  });

  it("the sink sees a running acknowledgement as acknowledged", () => {
    const t = setup();
    t.model.onStatus(status("reconnecting", { level: 2, acknowledged_until: 5_000 }), 1_000);
    expect(t.sink.snapshots.at(-1)?.acknowledged).toBe(true);
    t.model.onStatus(status("reconnecting", { level: 2, acknowledged_until: 5_000 }), 6_000);
    expect(t.sink.snapshots.at(-1)?.acknowledged).toBe(false);
  });

  it("stale pairing hint after 10 minutes of an absent peer", () => {
    const t = setup();
    t.model.onStatus(status("connecting", { level: 2, cause: "peer_absent" }), 0);
    t.advance(9 * 60_000);
    expect(t.state.link.stalePairingHint).toBe(false);
    t.advance(61_000);
    expect(t.state.link.stalePairingHint).toBe(true);
    t.model.onStatus(status("connecting", { level: 2, cause: "server_unreachable" }), t.time.now);
    expect(t.state.link.stalePairingHint).toBe(false);
  });
});

describe("events", () => {
  let t: ReturnType<typeof setup>;
  beforeEach(() => {
    t = setup();
    t.connect();
  });

  it("link_lost goes to the sink", () => {
    t.event({ type: "link_lost", down_ms: 61_000 });
    expect(t.sink.log).toContain("linkLost:61000");
  });

  it("link_restored shows the restored notice for 10 s", () => {
    t.event({ type: "link_restored", down_ms: 75_000 });
    expect(t.state.restoredAfterMs).toBe(75_000);
    expect(t.sink.notices.at(-1)).toEqual({ type: "link_restored", downMs: 75_000 });
    t.advance(9_500);
    expect(t.state.restoredAfterMs).toBe(75_000);
    t.advance(1_000);
    expect(t.state.restoredAfterMs).toBeNull();
  });

  it("security_warning sets the flag and notifies", () => {
    t.event({ type: "security_warning", key: "wrong_key_attempt" });
    expect(t.state.securityWarning).toBe(true);
    expect(t.sink.notices.at(-1)).toEqual({ type: "security_warning" });
  });

  it("peer_stopped notifies once", () => {
    t.event({ type: "peer_stopped" });
    expect(t.sink.notices).toEqual([{ type: "peer_stopped" }]);
  });

  it("alarm_acknowledged re-publishes the link snapshot", () => {
    t.model.onStatus(status("reconnecting", { level: 2, acknowledged_until: 400_000 }), 0);
    const before = t.sink.snapshots.length;
    t.event({ type: "alarm_acknowledged", until_ms: 400_000 });
    expect(t.sink.snapshots.length).toBe(before); // unchanged snapshot is not repeated
    expect(t.state.link.acknowledgedUntilElapsedMs).toBe(400_000);
  });

  it("low_battery and charger_unplugged notify with their data", () => {
    t.event({ type: "low_battery", pct: 15, critical: false });
    t.event({ type: "low_battery", pct: 5, critical: true });
    t.event({ type: "charger_unplugged" });
    expect(t.sink.notices).toEqual([
      { type: "low_battery", pct: 15, critical: false },
      { type: "low_battery", pct: 5, critical: true },
      { type: "charger_unplugged" },
    ]);
  });

  it("noise_gate_changed, status and failed leave the parent state alone", () => {
    const before = t.state;
    t.event({ type: "noise_gate_changed", threshold: 0.3, hold_ms: 2000 });
    t.event({ type: "failed", reason: "x" });
    t.event({ type: "status", status: status("connected") });
    expect(t.state).toEqual(before);
    expect(t.sink.notices).toEqual([]);
  });

  describe("noise_alert", () => {
    it("starts an event: active, since, sink gets a new full alert, audio is not muted", () => {
      t.advance(1_000);
      t.event({ type: "noise_alert", rms: 0.4, probe: false, snoozed: false });
      expect(t.state.noiseAlertActive).toBe(true);
      expect(t.state.noiseSinceWallMs).toBe(t.time.wallMs());
      expect(t.state.quietSinceWallMs).toBeNull();
      expect(t.state.noiseIsTest).toBe(false);
      expect(t.sink.noise).toEqual([["new", false, false]]);
      expect(t.effects.log.at(-1)).toBe("mute:false");
    });

    it("a probe is a test alert and records the test time", () => {
      t.event({ type: "noise_alert", rms: 0.5, probe: true, snoozed: false });
      expect(t.state.noiseIsTest).toBe(true);
      expect(t.effects.log).toContain(`test:${t.time.wallMs()}`);
    });

    it("snoozed noise stays visual: the playout is muted unless the parent listens", () => {
      t.event({ type: "noise_alert", rms: 0.5, probe: false, snoozed: true });
      expect(t.effects.log.at(-1)).toBe("mute:true");
      expect(t.state.noiseAlertActive).toBe(true);
      t.model.setListen("hold", t.time.now);
      t.event({ type: "noise_alert", rms: 0.5, probe: false, snoozed: true });
      expect(t.effects.log.at(-1)).toBe("mute:false");
    });

    it("noise within 2 minutes of the event continues it; later noise is a new event", () => {
      t.event({ type: "noise_alert", rms: 0.5, probe: false, snoozed: false });
      t.advance(30_000);
      t.event({ type: "noise_alert", rms: 0.5, probe: false, snoozed: false });
      expect(t.sink.noise.map((n) => n[0])).toEqual(["new", "continue"]);
      // The event ends 2 minutes after the last loud moment ...
      t.advance(121_000);
      expect(t.state.noiseAlertActive).toBe(false);
      expect(t.sink.log).toContain("noiseEnded");
      expect(t.state.lastSound).not.toBeNull();
      // ... and noise right after it still continues it, noise much later starts a new one.
      t.event({ type: "noise_alert", rms: 0.5, probe: false, snoozed: false });
      expect(t.sink.noise.at(-1)?.[0]).toBe("continue");
      t.advance(121_000 + 121_000);
      t.event({ type: "noise_alert", rms: 0.5, probe: false, snoozed: false });
      expect(t.sink.noise.at(-1)?.[0]).toBe("new");
    });

    it("loud received audio keeps the event alive and extends its duration", () => {
      t.event({ type: "noise_alert", rms: 0.5, probe: false, snoozed: false });
      t.advance(60_000);
      t.model.onRemoteLevel(0.5, t.time.now);
      t.advance(100_000);
      expect(t.state.noiseAlertActive).toBe(true); // 100 s since the last loud level
      expect(t.state.lastSound?.durationMs).toBe(60_000);
    });

    it("quiet since restarts at the last loud moment when the event ends", () => {
      t.event({ type: "noise_alert", rms: 0.5, probe: false, snoozed: false });
      const heardAt = t.time.now;
      t.advance(125_000);
      expect(t.state.quietSinceWallMs).toBe(t.time.wallMs() - (t.time.now - heardAt));
    });
  });

  describe("peer_heartbeat and the meter", () => {
    it("shows the heartbeat level for 12 s and the remote level with a decay", () => {
      t.event({ type: "peer_heartbeat", seq: 1, rms: 0.3 });
      expect(t.state.level).toBeCloseTo(0.3);
      t.advance(12_500);
      expect(t.state.level).toBeLessThan(0.01);
      t.model.onRemoteLevel(0.6, t.time.now);
      expect(t.state.level).toBeCloseTo(0.6);
      t.advance(1_000);
      expect(t.state.level).toBeLessThan(0.5);
      t.advance(10_000);
      expect(t.state.level).toBeLessThan(0.01);
    });

    it("is zero while not connected", () => {
      t.model.onRemoteLevel(0.6, t.time.now);
      t.model.onStatus(status("reconnecting", { level: 1 }), t.time.now);
      expect(t.state.level).toBe(0);
    });
  });

  describe("peer_status", () => {
    const base: DeviceStatus = {
      battery_pct: 80,
      charging: true,
      ringer: "normal",
      dnd: "off",
      dnd_access: true,
      alarms_allowed: true,
      next_alarm_min: 45,
      noise_threshold: 0.2,
      unit_warnings: ["page_hidden"],
    };
    const cases: Array<[string, DeviceStatus, Record<string, unknown>]> = [
      ["normal ringer may ring", base, { silence: "may_ring", isWebClient: false }],
      ["vibrate may buzz", { ...base, ringer: "vibrate" }, { silence: "may_buzz" }],
      [
        "ringer silent: screen may light",
        { ...base, ringer: "silent" },
        { silence: "screen_may_light" },
      ],
      [
        "dnd alarms only: screen may light",
        { ...base, dnd: "alarms_only" },
        { silence: "screen_may_light" },
      ],
      ["dnd total: screen may light", { ...base, dnd: "total" }, { silence: "screen_may_light" }],
      ["can make noise without access", { ...base, dnd_access: false }, { silence: "no_access" }],
      ["our own rule is silent", { ...base, quiet_active: true }, { silence: "silent" }],
      ["unknown ringer is unknown", { ...base, ringer: "unknown" }, { silence: "unknown" }],
      [
        "a browser baby reports no ringer, dnd or access: unknown and flagged",
        { battery_pct: 60, unit_warnings: ["wake_lock_off"] },
        { silence: "unknown", isWebClient: true, unitWarnings: ["wake_lock_off"], batteryPct: 60 },
      ],
    ];
    it.each(cases)("%s", (_name, input, expected) => {
      t.event({ type: "peer_status", status: input });
      expect(t.state.peer).toMatchObject(expected);
    });

    it("carries the battery, charging, alarm and microphone fields and the baby's threshold", () => {
      t.event({ type: "peer_status", status: { ...base, mic_silent: true } });
      expect(t.state.peer).toMatchObject({
        batteryPct: 80,
        charging: true,
        alarmsAllowed: true,
        nextAlarmMinutes: 45,
        micSilent: true,
        unitWarnings: ["page_hidden"],
      });
      expect(t.state.babyThreshold).toBe(0.2);
    });

    it("applies a pending silence request when the baby reports the rule", () => {
      t.model.requestSilence(t.time.now);
      expect(t.state.silenceRequest).toBe("pending");
      t.event({ type: "peer_status", status: { ...base, quiet_active: true } });
      expect(t.state.silenceRequest).toBe("applied");
      t.advance(6_500);
      expect(t.state.silenceRequest).toBe("idle");
    });

    it("fails a silence request that is not answered within 10 s", () => {
      t.model.requestSilence(t.time.now);
      t.advance(10_500);
      expect(t.state.silenceRequest).toBe("failed");
    });
  });

  describe("missed_events and quiet since", () => {
    function outage(): void {
      t.advance(5_000);
      t.model.onStatus(status("reconnecting", { level: 1 }), t.time.now);
      t.advance(20_000);
      t.model.onStatus(status("connected"), t.time.now);
    }

    it("a complete empty summary restores the quiet time from before the outage", () => {
      const quietBefore = t.state.quietSinceWallMs;
      outage();
      expect(t.state.quietSinceWallMs).toBeNull(); // earned, not assumed
      t.event({ type: "missed_events", count: 0, max_rms: 0, complete: true, down_ms: 20_000 });
      expect(t.state.quietSinceWallMs).toBe(quietBefore);
      expect(t.state.missed).toEqual({
        downMs: 20_000,
        count: 0,
        complete: true,
        endWallMs: null,
        loudest: 0,
      });
      expect(t.sink.notices.at(-1)).toEqual({ type: "missed_events", count: 0, downMs: 20_000 });
    });

    it("a summary with events restarts quiet at the reconnect", () => {
      outage();
      const reconnectWall = t.time.wallMs();
      t.event({ type: "missed_events", count: 3, max_rms: 0.7, complete: true, down_ms: 20_000 });
      expect(t.state.quietSinceWallMs).toBe(reconnectWall);
      expect(t.state.missed).toMatchObject({ count: 3, loudest: 0.7 });
    });

    it("an incomplete summary hides the count and the loudest level", () => {
      outage();
      t.event({ type: "missed_events", count: 2, max_rms: 0.7, complete: false, down_ms: 20_000 });
      expect(t.state.missed).toMatchObject({ count: null, complete: false, loudest: null });
      expect(t.sink.notices.at(-1)).toEqual({ type: "missed_events", count: null, downMs: 20_000 });
    });

    it("no summary within 5 s falls back to the reconnect time", () => {
      outage();
      const reconnectWall = t.time.wallMs();
      expect(t.state.quietSinceWallMs).toBeNull();
      t.advance(5_500);
      expect(t.state.quietSinceWallMs).toBe(reconnectWall);
    });

    it("dismiss clears the summary", () => {
      t.event({ type: "missed_events", count: 1, max_rms: 0.3, complete: true, down_ms: 1 });
      t.model.dismissMissed();
      expect(t.state.missed).toBeNull();
    });

    it("a first connection is quiet from the start", () => {
      const fresh = setup();
      fresh.connect();
      expect(fresh.state.quietSinceWallMs).toBe(fresh.time.wallMs());
    });
  });

  describe("listen", () => {
    it("is refused while not connected", () => {
      const off = setup();
      off.model.onStatus(status("reconnecting", { level: 1 }), 0);
      expect(off.model.setListen("hold", 0)).toBe(false);
      expect(off.state.listen).toBe("off");
    });

    it("unmutes the playout and follows the mode", () => {
      expect(t.model.setListen("locked", t.time.now)).toBe(true);
      expect(t.state.listen).toBe("locked");
      expect(t.effects.log).toContain("mute:false");
    });

    it("the baby ending listen resets the mode", () => {
      t.model.setListen("hold", t.time.now);
      t.event({ type: "listen_changed", enabled: false });
      expect(t.state.listen).toBe("off");
      expect(t.effects.log).toContain("listenStopped");
    });

    it("locked listening stops by itself after 60 minutes", () => {
      t.model.setListen("locked", t.time.now);
      t.advance(59 * 60_000);
      expect(t.state.listen).toBe("locked");
      t.advance(61_000);
      expect(t.state.listen).toBe("off");
      expect(t.effects.log).toContain("listenStopped");
    });
  });

  describe("snooze", () => {
    it("stores the end on both clocks and ends by itself", () => {
      t.model.snooze(10);
      expect(t.state.snoozedUntilWallMs).toBe(t.time.wallMs() + 600_000);
      expect(t.model.snoozeUntilElapsedMs()).toBe(t.time.now + 600_000);
      t.advance(601_000);
      expect(t.state.snoozedUntilWallMs).toBeNull();
      expect(t.effects.log).toContain("snoozeEnded");
    });

    it("cancel ends it at once", () => {
      t.model.snooze(30);
      t.model.cancelSnooze();
      expect(t.state.snoozedUntilWallMs).toBeNull();
      expect(t.model.snoozeUntilElapsedMs()).toBeNull();
    });

    it("noise that is still going when the snooze ends alerts immediately", () => {
      t.model.snooze(10);
      t.advance(598_000);
      t.event({ type: "noise_alert", rms: 0.5, probe: false, snoozed: true });
      t.model.onRemoteLevel(0.5, t.time.now);
      t.sink.noise.length = 0;
      t.advance(3_000);
      expect(t.sink.noise).toEqual([["new", false, false]]);
    });
  });

  it("link quality follows the packet loss of the last stats interval", () => {
    t.model.onInboundStats({ packetsReceived: 100, packetsLost: 0 }, t.time.now);
    t.model.onInboundStats({ packetsReceived: 300, packetsLost: 2 }, t.time.now);
    expect(t.state.link.quality).toBe("good");
    t.model.onInboundStats({ packetsReceived: 500, packetsLost: 22 }, t.time.now);
    expect(t.state.link.quality).toBe("medium");
    t.model.onInboundStats({ packetsReceived: 700, packetsLost: 122 }, t.time.now);
    expect(t.state.link.quality).toBe("poor");
    // Too few packets to judge keeps the last value; leaving connected forgets it.
    t.model.onInboundStats({ packetsReceived: 705, packetsLost: 122 }, t.time.now);
    expect(t.state.link.quality).toBe("poor");
    t.model.onStatus(status("reconnecting", { level: 1 }), t.time.now);
    t.connect();
    expect(t.state.link.quality).toBe("unknown");
  });

  it("stop sound mutes the playout and tells the sink", () => {
    t.model.stopAlertSound();
    expect(t.effects.log).toContain("mute:true");
    expect(t.sink.log).toContain("stopNoiseSound");
  });

  it("readiness is passed through", () => {
    t.model.setReadiness(["headphones_connected"], 40);
    expect(t.state.readiness).toEqual(["headphones_connected"]);
    expect(t.state.ownBatteryPct).toBe(40);
  });

  it("reset forgets the run but keeps readiness and the snooze", () => {
    t.model.setReadiness(["never_tested"]);
    t.model.snooze(10);
    t.event({ type: "noise_alert", rms: 0.5, probe: false, snoozed: false });
    t.model.reset();
    expect(t.state.noiseAlertActive).toBe(false);
    expect(t.state.link.phase).toBe("idle");
    expect(t.state.readiness).toEqual(["never_tested"]);
    expect(t.state.snoozedUntilWallMs).not.toBeNull();
  });
});
