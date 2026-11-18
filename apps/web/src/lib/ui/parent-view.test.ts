import { describe, expect, it } from "vitest";
import { createTranslator } from "../i18n/translate";
import {
  defaultLinkUi,
  defaultParentMonitor,
  type LinkUi,
  type MissedEventsUi,
  type ParentMonitorState,
} from "../contract";
import {
  STATUS_LOOK,
  connectionName,
  statusHeadline,
  alertStageOf,
  canRequestSilence,
  downForMs,
  elapsedToWall,
  fullScreenOf,
  headlineOf,
  missedResultOf,
  outageStartElapsedMs,
  pruneDismissed,
  statusKindOf,
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

describe("statusKindOf", () => {
  const table: Array<[Partial<LinkUi>, string]> = [
    [{ phase: "connected" }, "connected"],
    [{ phase: "connecting", level: 0 }, "connecting"],
    [{ phase: "reconnecting", level: 0 }, "reconnecting"],
    // First connection not answered: orange at level 1 while `connecting`, never blue.
    [{ phase: "connecting", level: 1 }, "not_connected_yet"],
    [{ phase: "reconnecting", level: 1 }, "cant_reach"],
    [{ phase: "connecting", level: 2 }, "couldnt_connect"],
    [{ phase: "reconnecting", level: 2 }, "no_connection"],
    [{ phase: "reconnecting", level: 3 }, "still_no_connection"],
    [{ phase: "connecting", level: 3 }, "still_no_connection"],
    [{ phase: "peer_stopped" }, "baby_stopped"],
    [{ phase: "failed_needs_user", failureKey: "pairing_key_mismatch" }, "needs_pairing"],
    [{ phase: "idle" }, "idle"],
    [{ phase: "stopped" }, "idle"],
  ];
  it.each(table)("%j is %s", (over, kind) => {
    expect(statusKindOf(link(over))).toBe(kind);
  });

  it("colours by stage: neutral, blue, orange, red, grey", () => {
    expect(STATUS_LOOK.connected.tone).toBe("ok");
    expect(STATUS_LOOK.connecting.tone).toBe("info");
    expect(STATUS_LOOK.reconnecting.tone).toBe("info");
    expect(STATUS_LOOK.not_connected_yet.tone).toBe("orange");
    expect(STATUS_LOOK.cant_reach.tone).toBe("orange");
    expect(STATUS_LOOK.no_connection.tone).toBe("bad");
    expect(STATUS_LOOK.baby_stopped.tone).toBe("neutral");
  });

  it("announces politely for levels 0 and 1 and assertively from level 2", () => {
    for (const kind of ["connected", "connecting", "reconnecting", "cant_reach"] as const) {
      expect(STATUS_LOOK[kind].assertive).toBe(false);
    }
    for (const kind of [
      "couldnt_connect",
      "no_connection",
      "still_no_connection",
      "baby_stopped",
      "needs_pairing",
    ] as const) {
      expect(STATUS_LOOK[kind].assertive).toBe(true);
    }
  });

  it("gives every stage an icon so colour is never the only signal", () => {
    for (const look of Object.values(STATUS_LOOK)) expect(look.icon).toBeTruthy();
  });
});

describe("downForMs and the outage timing fields", () => {
  it("counts from the outage start the core reports", () => {
    expect(downForMs(link({ downSinceElapsedMs: 1_000 }), 61_000)).toBe(60_000);
    expect(downForMs(link({ lastAliveElapsedMs: 1_000 }), 61_000)).toBe(60_000);
    expect(downForMs(link({}), 61_000)).toBe(0);
  });

  it("the start before any contact is the run start the core sent", () => {
    expect(outageStartElapsedMs(link({ downSinceElapsedMs: 500, lastAliveElapsedMs: null }))).toBe(
      500,
    );
  });
});

describe("alertStageOf", () => {
  it("is vibrating at level 2, loud at level 3 and paused while acknowledged", () => {
    expect(alertStageOf(link({ phase: "reconnecting", level: 2 }), 0)).toBe("vibrating");
    expect(alertStageOf(link({ phase: "reconnecting", level: 3 }), 0)).toBe("loud");
    const acked = link({ phase: "reconnecting", level: 2, acknowledgedUntilElapsedMs: 300_000 });
    expect(alertStageOf(acked, 100_000)).toBe("paused");
    expect(alertStageOf(acked, 300_001)).toBe("vibrating");
  });
});

describe("elapsedToWall", () => {
  it("shows next_level_at_ms with the platform's reading of the core clock", () => {
    // The core clock is the clamped Date.now(): a time 90 s ahead of now is 90 s ahead on the wall.
    const wall = 1_800_000_000_000;
    expect(elapsedToWall(160_000, 70_000, wall)).toBe(wall + 90_000);
    expect(elapsedToWall(70_000, 70_000, wall)).toBe(wall);
  });
});

describe("missedResultOf", () => {
  const m = (over: Partial<MissedEventsUi>): MissedEventsUi => ({
    downMs: 60_000,
    count: 0,
    complete: true,
    endWallMs: null,
    loudest: null,
    ...over,
  });
  it("tells no sound, some sounds and unknown apart", () => {
    expect(missedResultOf(m({ count: 0 }))).toBe("none");
    expect(missedResultOf(m({ count: 2 }))).toBe("some");
    expect(missedResultOf(m({ complete: false, count: 2 }))).toBe("unknown");
    expect(missedResultOf(m({ count: null }))).toBe("unknown");
    expect(missedResultOf(null)).toBe("unknown");
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

  it("names a running sound as paused while noise alerts are paused", () => {
    expect(
      head(
        connected({
          noiseAlertActive: true,
          noiseSinceWallMs: now.wall - 5_000,
          snoozedUntilWallMs: now.wall + 60_000,
        }),
      ),
    ).toEqual({ kind: "sound_snoozed", atWallMs: now.wall - 5_000 });
  });

  it("names the last sound even while noise alerts are paused and no event runs", () => {
    const lastSound = { atWallMs: now.wall - 90_000, durationMs: 30_000 };
    expect(head(connected({ lastSound, snoozedUntilWallMs: now.wall + 60_000 }))).toEqual({
      kind: "sound_at",
      atWallMs: lastSound.atWallMs,
      durationMs: 30_000,
    });
  });

  it("never says 'all quiet' while the baby device's microphone is silent", () => {
    const peer = { ...defaultParentMonitor().peer, micSilent: true };
    expect(head(connected({ peer, quietSinceWallMs: now.wall - 60_000 }))).toEqual({
      kind: "cant_hear",
    });
    // The order is Android's: a silent microphone beats a running event.
    expect(
      head(
        connected({
          peer,
          noiseAlertActive: true,
          noiseSinceWallMs: now.wall - 5_000,
        }),
      ).kind,
    ).toBe("cant_hear");
  });

  it("keeps the whole order: down, silent microphone, paused sound, sound, quiet, last sound", () => {
    const lastSound = { atWallMs: now.wall - 90_000, durationMs: 30_000 };
    const quiet = now.wall - 60_000;
    expect(head(connected({ lastSound, quietSinceWallMs: quiet })).kind).toBe("all_quiet");
    expect(head(connected({ lastSound })).kind).toBe("sound_at");
    expect(head(connected()).kind).toBe("monitoring");
  });

  it("falls back to plain 'monitoring' right after a reconnect", () => {
    expect(head(connected())).toEqual({ kind: "monitoring" });
  });
});

describe("fullScreenOf", () => {
  const lost = state({ link: link({ phase: "reconnecting", level: 2 }) });

  it("shows nothing for a healthy link", () => {
    expect(fullScreenOf(connected(), none)).toBeNull();
  });

  it("shows the link alarm from level 2 and lets level 1 pass", () => {
    expect(fullScreenOf(lost, none)).toBe("link_lost");
    expect(fullScreenOf(state({ link: link({ phase: "reconnecting", level: 1 }) }), none)).toBe(
      null,
    );
  });

  it("keeps the link alarm up after 'I'm checking': it stays red as 'Alarm paused'", () => {
    const acked = state({
      link: link({ phase: "reconnecting", level: 2, acknowledgedUntilElapsedMs: 300_000 }),
    });
    expect(fullScreenOf(acked, none)).toBe("link_lost");
  });

  it("shows the noise alert, and the link alarm before it", () => {
    expect(fullScreenOf(connected({ noiseAlertActive: true }), none)).toBe("noise");
    expect(fullScreenOf({ ...lost, noiseAlertActive: true }, none)).toBe("link_lost");
  });

  it("lets 'Stop sound' close the noise alert for the rest of the event", () => {
    expect(fullScreenOf(connected({ noiseAlertActive: true }), new Set(["noise"]))).toBeNull();
  });

  it("shows the stopped baby device until OK, then the battery", () => {
    const stopped = state({ link: link({ phase: "peer_stopped" }) });
    expect(fullScreenOf(stopped, none)).toBe("baby_stopped");
    expect(fullScreenOf(stopped, new Set(["baby_stopped"]))).toBeNull();
    const dying = connected({
      peer: { ...defaultParentMonitor().peer, batteryPct: 3, charging: false },
    });
    expect(fullScreenOf(dying, none)).toBe("battery_critical");
    expect(fullScreenOf(dying, new Set(["battery_critical"]))).toBeNull();
  });

  it("ignores a low battery that is charging", () => {
    const charging = connected({
      peer: { ...defaultParentMonitor().peer, batteryPct: 3, charging: true },
    });
    expect(fullScreenOf(charging, none)).toBeNull();
  });

  it("asks to pair again when the key no longer matches", () => {
    expect(fullScreenOf(state({ link: link({ phase: "failed_needs_user" }) }), none)).toBe(
      "needs_pairing",
    );
  });

  it("orders connection lost, noise, baby stopped, battery", () => {
    const everything = state({
      link: link({ phase: "peer_stopped" }),
      noiseAlertActive: true,
      peer: { ...defaultParentMonitor().peer, batteryPct: 3 },
    });
    expect(fullScreenOf(everything, none)).toBe("noise");
    expect(fullScreenOf(everything, new Set(["noise"]))).toBe("baby_stopped");
    expect(fullScreenOf(everything, new Set(["noise", "baby_stopped"]))).toBeNull();
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

describe("connectionName", () => {
  it("is the trimmed name, or null for none and blank", () => {
    expect(connectionName("  Nursery ")).toBe("Nursery");
    expect(connectionName("   ")).toBeNull();
    expect(connectionName("")).toBeNull();
    expect(connectionName(null)).toBeNull();
  });
});

describe("statusHeadline", () => {
  const t = createTranslator("en");
  const l = link({});
  it("composes 'State · Name' and leaves the bare state without a name", () => {
    expect(statusHeadline(t, "connected", l, "Nursery")).toBe("Connected · Nursery");
    expect(statusHeadline(t, "connected", l, null)).toBe("Connected");
    expect(statusHeadline(t, "still_no_connection", l, "Nursery")).toBe(
      "Still no connection · Nursery",
    );
  });
  it("never names an idle bar, and a link that needs the user says the core's message", () => {
    expect(statusHeadline(t, "idle", l, "Nursery")).toBe("Not monitoring");
    expect(
      statusHeadline(
        t,
        "needs_pairing",
        link({ phase: "failed_needs_user", failureKey: "update_required" }),
        "Nursery",
      ),
    ).toContain("update");
  });
});
