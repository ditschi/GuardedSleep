import { describe, expect, it } from "vitest";
import {
  defaultLinkUi,
  defaultParentMonitor,
  type LinkUi,
  type ParentMonitorState,
} from "../contract";
import {
  AMBIENT_LOOK,
  ambientKind,
  canRequestSilence,
  downForMs,
  extraChips,
  fixedChips,
  fullScreenOf,
  headlineOf,
  pruneDismissed,
  type FullScreenKind,
} from "./parent-view";

const link = (over: Partial<LinkUi>): LinkUi => ({ ...defaultLinkUi(), ...over });
const state = (over: Partial<ParentMonitorState> = {}): ParentMonitorState => ({
  ...defaultParentMonitor(),
  ...over,
});
const connected = (over: Partial<ParentMonitorState> = {}) =>
  state({ link: link({ phase: "connected" }), ...over });
const none = new Set<FullScreenKind>();

describe("ambientKind", () => {
  const table: Array<[Partial<LinkUi>, string]> = [
    [{ phase: "connected" }, "connected"],
    [{ phase: "connecting", level: 0 }, "connecting"],
    [{ phase: "connecting", level: 1 }, "reconnecting"],
    [{ phase: "reconnecting", level: 0 }, "reconnecting"],
    [{ phase: "reconnecting", level: 1 }, "reconnecting"],
    [{ phase: "reconnecting", level: 2 }, "cant_reach"],
    [{ phase: "reconnecting", level: 3 }, "lost_for"],
    [{ phase: "peer_stopped" }, "baby_stopped"],
    [{ phase: "failed_needs_user", failureKey: "pairing_key_mismatch" }, "needs_pairing"],
    [{ phase: "idle" }, "idle"],
    [{ phase: "stopped" }, "idle"],
  ];
  it.each(table)("%j is %s", (over, kind) => {
    expect(ambientKind(link(over))).toBe(kind);
  });

  it("gives every state a tone, an icon and (from level 2) an assertive announcement", () => {
    for (const look of Object.values(AMBIENT_LOOK)) {
      expect(look.icon).toBeTruthy();
      expect(look.tone).toBeTruthy();
    }
    expect(AMBIENT_LOOK.connected.tone).toBe("ok");
    expect(AMBIENT_LOOK.cant_reach.assertive).toBe(true);
    expect(AMBIENT_LOOK.lost_for.assertive).toBe(true);
    expect(AMBIENT_LOOK.reconnecting.assertive).toBe(false);
  });
});

describe("downForMs", () => {
  it("counts from the last contact", () => {
    expect(downForMs(link({ lastAliveElapsedMs: 1_000 }), 61_000)).toBe(60_000);
    expect(downForMs(link({ lastAliveElapsedMs: null }), 61_000)).toBe(0);
  });
});

describe("headlineOf", () => {
  const now = { elapsed: 100_000, wall: 1_800_000_000_000 };
  const head = (p: ParentMonitorState) => headlineOf(p, now.elapsed, now.wall);

  it("is 'all quiet since' only while connected and quiet", () => {
    expect(head(connected({ quietSinceWallMs: now.wall - 60_000 }))).toEqual({
      kind: "all_quiet",
      sinceWallMs: now.wall - 60_000,
    });
  });

  it("never says 'all quiet' while the link is down; it says when it last heard", () => {
    const down = state({
      link: link({ phase: "reconnecting", level: 2, lastAliveElapsedMs: 40_000 }),
      quietSinceWallMs: now.wall - 600_000,
    });
    expect(head(down)).toEqual({ kind: "last_heard", atWallMs: now.wall - 60_000 });
  });

  it("admits that nothing was heard yet", () => {
    expect(head(state({ link: link({ phase: "connecting" }) }))).toEqual({ kind: "no_contact" });
  });

  it("names a sound that is going on", () => {
    expect(head(connected({ noiseAlertActive: true, noiseSinceWallMs: now.wall - 5_000 }))).toEqual(
      { kind: "sound_since", sinceWallMs: now.wall - 5_000 },
    );
  });

  it("names the last sound and its length until quiet is earned again", () => {
    const lastSound = { atWallMs: now.wall - 90_000, durationMs: 30_000 };
    expect(head(connected({ lastSound }))).toEqual({
      kind: "sound_at",
      atWallMs: lastSound.atWallMs,
      durationMs: 30_000,
    });
  });

  it("says alerts are snoozed when they are", () => {
    const lastSound = { atWallMs: now.wall - 90_000, durationMs: 30_000 };
    expect(head(connected({ lastSound, snoozedUntilWallMs: now.wall + 60_000 }))).toEqual({
      kind: "sound_snoozed",
      atWallMs: lastSound.atWallMs,
    });
  });

  it("falls back to plain 'monitoring' right after a reconnect", () => {
    expect(head(connected())).toEqual({ kind: "monitoring" });
  });
});

describe("fullScreenOf", () => {
  const lost = state({ link: link({ phase: "reconnecting", level: 2 }) });

  it("shows nothing for a healthy link", () => {
    expect(fullScreenOf(connected(), 0, none)).toBeNull();
  });

  it("shows the link alarm from level 2 and lets level 1 pass", () => {
    expect(fullScreenOf(lost, 0, none)).toBe("link_lost");
    expect(fullScreenOf(state({ link: link({ phase: "reconnecting", level: 1 }) }), 0, none)).toBe(
      null,
    );
  });

  it("hides the link alarm while 'I'm checking' mutes it, and brings it back after", () => {
    const acked = state({
      link: link({ phase: "reconnecting", level: 2, acknowledgedUntilElapsedMs: 300_000 }),
    });
    expect(fullScreenOf(acked, 100_000, none)).toBeNull();
    expect(fullScreenOf(acked, 300_001, none)).toBe("link_lost");
  });

  it("shows the noise alert, and the link alarm before it", () => {
    expect(fullScreenOf(connected({ noiseAlertActive: true }), 0, none)).toBe("noise");
    expect(fullScreenOf({ ...lost, noiseAlertActive: true }, 0, none)).toBe("link_lost");
  });

  it("lets 'Stop sound' close the noise alert for the rest of the event", () => {
    expect(fullScreenOf(connected({ noiseAlertActive: true }), 0, new Set(["noise"]))).toBeNull();
  });

  it("shows the stopped baby device until OK, then the battery", () => {
    const stopped = state({ link: link({ phase: "peer_stopped" }) });
    expect(fullScreenOf(stopped, 0, none)).toBe("baby_stopped");
    expect(fullScreenOf(stopped, 0, new Set(["baby_stopped"]))).toBeNull();
    const dying = connected({
      peer: { ...defaultParentMonitor().peer, batteryPct: 3, charging: false },
    });
    expect(fullScreenOf(dying, 0, none)).toBe("battery_critical");
    expect(fullScreenOf(dying, 0, new Set(["battery_critical"]))).toBeNull();
  });

  it("ignores a low battery that is charging", () => {
    const charging = connected({
      peer: { ...defaultParentMonitor().peer, batteryPct: 3, charging: true },
    });
    expect(fullScreenOf(charging, 0, none)).toBeNull();
  });

  it("asks to pair again when the key no longer matches", () => {
    expect(fullScreenOf(state({ link: link({ phase: "failed_needs_user" }) }), 0, none)).toBe(
      "needs_pairing",
    );
  });

  it("orders connection lost, noise, baby stopped, battery", () => {
    const everything = state({
      link: link({ phase: "peer_stopped" }),
      noiseAlertActive: true,
      peer: { ...defaultParentMonitor().peer, batteryPct: 3 },
    });
    expect(fullScreenOf(everything, 0, none)).toBe("noise");
    expect(fullScreenOf(everything, 0, new Set(["noise"]))).toBe("baby_stopped");
    expect(fullScreenOf(everything, 0, new Set(["noise", "baby_stopped"]))).toBeNull();
  });
});

describe("pruneDismissed", () => {
  it("forgets a dismissal when its situation is over", () => {
    const all = new Set<FullScreenKind>(["noise", "baby_stopped", "battery_critical"]);
    expect([...pruneDismissed(connected(), all)]).toEqual([]);
    expect([...pruneDismissed(connected({ noiseAlertActive: true }), all)]).toEqual(["noise"]);
  });

  it("returns the same set when nothing changes", () => {
    const set = new Set<FullScreenKind>(["noise"]);
    expect(pruneDismissed(connected({ noiseAlertActive: true }), set)).toBe(set);
  });
});

describe("chips", () => {
  const peer = defaultParentMonitor().peer;

  it("shows the battery and a grey 'unknown, turn it on at the baby device' for a browser baby", () => {
    const chips = fixedChips(connected({ peer: { ...peer, batteryPct: 80, isWebClient: true } }));
    expect(chips.map((c) => c.id)).toEqual(["parent_baby_battery_chip", "parent_silence_chip"]);
    expect(chips[0]).toMatchObject({ key: "native.ui.battery.baby", tone: "neutral" });
    expect(chips[1]).toMatchObject({ key: "webScreens.parent.quietUnknownWeb", tone: "neutral" });
  });

  it("asks for the charger when the battery is low and not charging", () => {
    const low = fixedChips(connected({ peer: { ...peer, batteryPct: 15, charging: false } }));
    expect(low[0]).toMatchObject({ key: "native.ui.battery.babyLow", tone: "warn" });
    const charging = fixedChips(connected({ peer: { ...peer, batteryPct: 15, charging: true } }));
    expect(charging[0]).toMatchObject({ key: "native.ui.battery.baby", tone: "neutral" });
  });

  it("maps the silence status of a native baby device", () => {
    const silent = fixedChips(connected({ peer: { ...peer, silence: "silent" } }));
    expect(silent[0]).toMatchObject({ key: "native.ui.silence.silent", tone: "ok" });
    const ring = fixedChips(connected({ peer: { ...peer, silence: "may_ring" } }));
    expect(ring[0]).toMatchObject({ key: "native.ui.silence.mayRing", tone: "warn" });
  });

  it("lists warnings in priority order", () => {
    const wall = 1_800_000_000_000;
    const chips = extraChips(
      connected({
        ownBatteryPct: 10,
        peer: { ...peer, unitWarnings: ["page_hidden", "not_charging", "unknown_future"] },
        snoozedUntilWallMs: wall + 5 * 60_000,
        link: link({ phase: "connected", quality: "poor" }),
      }),
      wall,
    );
    expect(chips.map((c) => c.id)).toEqual([
      "parent_own_battery_chip",
      "parent_unit_warning_page_hidden",
      "parent_unit_warning_not_charging",
      "parent_snooze_chip",
      "parent_quality_chip",
    ]);
    expect(chips.find((c) => c.id === "parent_snooze_chip")?.args).toEqual({ minutes: 5 });
  });

  it("shows no warning chips for a healthy link", () => {
    expect(extraChips(connected(), 0)).toEqual([]);
  });
});

describe("canRequestSilence", () => {
  const peer = defaultParentMonitor().peer;

  it("is never offered for a browser baby device", () => {
    expect(
      canRequestSilence(connected({ peer: { ...peer, isWebClient: true, silence: "may_ring" } })),
    ).toBe(false);
  });

  it("is offered for a native baby device that may make noise", () => {
    expect(canRequestSilence(connected({ peer: { ...peer, silence: "may_ring" } }))).toBe(true);
    expect(canRequestSilence(connected({ peer: { ...peer, silence: "silent" } }))).toBe(false);
  });

  it("needs a connected link", () => {
    expect(
      canRequestSilence(
        state({ link: link({ phase: "reconnecting" }), peer: { ...peer, silence: "may_ring" } }),
      ),
    ).toBe(false);
  });
});
