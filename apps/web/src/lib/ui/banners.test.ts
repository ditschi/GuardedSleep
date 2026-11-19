import { describe, expect, it } from "vitest";
import {
  defaultLinkUi,
  defaultParentMonitor,
  defaultPeerStatus,
  type LinkUi,
  type PageGuardState,
  type ParentMonitorState,
  type PeerStatusUi,
  type ReadinessIssue,
} from "../contract";
import kotlin from "../../../../android/ui-contract/src/main/kotlin/org/guardedsleep/app/ui/model/Banners.kt?raw";
import contract from "../../../../android/ui-contract/src/main/kotlin/org/guardedsleep/app/ui/model/UiContract.kt?raw";
import { BABY_BATTERY_LOW_PCT, BATTERY_CRITICAL_PCT } from "./parent-view";
import {
  BANNER_LIMIT,
  BANNER_ORDER,
  bannerLevel,
  browserBanners,
  parentBanners,
  rankBanners,
  type Banner,
  type BannerKind,
} from "./banners";

const b = (kind: BannerKind, id: string = kind): Banner => ({ kind, id });
const kinds = (l: readonly Banner[]): BannerKind[] => l.map((x) => x.kind);

describe("rankBanners", () => {
  it("shows two of four notices and folds the rest", () => {
    const s = rankBanners([
      b("mic_silent"),
      b("readiness"),
      b("baby_battery_low"),
      b("vibrate_only"),
    ]);
    expect(kinds(s.shown)).toEqual(["mic_silent", "readiness"]);
    expect(kinds(s.folded)).toEqual(["baby_battery_low", "vibrate_only"]);
  });

  it("ranks the same whatever the input order", () => {
    const s = rankBanners([
      b("vibrate_only"),
      b("baby_battery_low"),
      b("readiness"),
      b("mic_silent"),
    ]);
    expect(kinds(s.shown)).toEqual(["mic_silent", "readiness"]);
    expect(kinds(s.folded)).toEqual(["baby_battery_low", "vibrate_only"]);
  });

  it("folds only beyond the limit", () => {
    expect(BANNER_LIMIT).toBe(2);
    expect(rankBanners([b("paused")])).toEqual({ shown: [b("paused")], folded: [] });
    const two = rankBanners([b("paused"), b("vibrate_only")]);
    expect(two.shown).toHaveLength(2);
    expect(two.folded).toHaveLength(0);
    const three = rankBanners([b("paused"), b("vibrate_only"), b("security")]);
    expect(kinds(three.shown)).toEqual(["security", "paused"]);
    expect(kinds(three.folded)).toEqual(["vibrate_only"]);
  });

  it("puts weak connection above pause above vibrate only", () => {
    const s = rankBanners([b("vibrate_only"), b("paused"), b("weak_connection")], 9);
    expect(kinds(s.shown)).toEqual(["weak_connection", "paused", "vibrate_only"]);
  });

  it("ranks security before everything", () => {
    // The plain keep-open note ranks last by design (its own test below); use a warning variant here.
    const all = [...BANNER_ORDER]
      .reverse()
      .map((k) => (k === "browser_keep_open" ? b(k, "browser_keep_open_lost") : b(k)));
    expect(rankBanners(all).shown[0]?.kind).toBe("security");
    expect(kinds(rankBanners(all, 99).shown)).toEqual([...BANNER_ORDER]);
  });

  it("puts the browser kinds between readiness and the baby battery", () => {
    const s = rankBanners(
      [
        b("baby_battery_low"),
        b("browser_away"),
        b("browser_keep_open", "browser_keep_open_lost"),
        b("browser_sound_gate"),
        b("readiness"),
      ],
      9,
    );
    expect(kinds(s.shown)).toEqual([
      "readiness",
      "browser_sound_gate",
      "browser_keep_open",
      "browser_away",
      "baby_battery_low",
    ]);
  });

  it("is stable for banners of one kind", () => {
    const s = rankBanners(
      [
        b("unit_warning", "unit_warning_wake_lock_off"),
        b("unit_warning", "unit_warning_page_hidden"),
      ],
      9,
    );
    expect(s.shown.map((x) => x.id)).toEqual([
      "unit_warning_wake_lock_off",
      "unit_warning_page_hidden",
    ]);
  });
});

describe("bannerLevel", () => {
  it("announces warn and alert, not info and note", () => {
    expect(bannerLevel("security")).toBe("alert");
    expect(bannerLevel("mic_silent")).toBe("warn");
    expect(bannerLevel("readiness")).toBe("warn");
    expect(bannerLevel("browser_sound_gate")).toBe("warn");
    expect(bannerLevel("baby_unplugged")).toBe("info");
    expect(bannerLevel("paused")).toBe("info");
    expect(bannerLevel("browser_away")).toBe("info");
    expect(bannerLevel("vibrate_only")).toBe("note");
    expect(bannerLevel("browser_keep_open")).toBe("note");
  });
});

describe("parentBanners", () => {
  const connected: LinkUi = { ...defaultLinkUi(), phase: "connected" };
  function state(
    over: {
      peer?: Partial<PeerStatusUi>;
      link?: Partial<LinkUi>;
      readiness?: ReadinessIssue[];
      snoozedUntil?: number | null;
      security?: boolean;
    } = {},
  ): ParentMonitorState {
    return {
      ...defaultParentMonitor(),
      link: { ...connected, ...over.link },
      peer: { ...defaultPeerStatus(), ...over.peer },
      readiness: over.readiness ?? [],
      snoozedUntilWallMs: over.snoozedUntil ?? null,
      securityWarning: over.security ?? false,
    };
  }
  const ids = (
    p: ParentMonitorState,
    style: "sound" | "discreet" = "sound",
    now = 1_000,
  ): string[] => parentBanners(p, style, now).map((x) => x.id);

  it("has no banner for a quiet connected monitor", () => {
    expect(ids(state({ peer: { batteryPct: 80, charging: false } }))).toEqual([]);
  });

  it("reports the silent microphone only while connected, and the security warning", () => {
    expect(ids(state({ peer: { micSilent: true } }))).toEqual(["mic_silent"]);
    expect(ids(state({ peer: { micSilent: true }, link: { phase: "reconnecting" } }))).toEqual([]);
    expect(ids(state({ security: true }))).toEqual(["security"]);
    expect(ids(state({ peer: { unitWarnings: ["mic_permission_lost"] } }))).toEqual([
      "mic_permission_lost",
    ]);
  });

  it("shows only the worst readiness item", () => {
    expect(ids(state({ readiness: ["notifications_blocked", "parent_battery_low"] }))).toEqual([
      "notifications_blocked",
    ]);
    expect(ids(state({ readiness: ["alarm_volume_low"] }))).toEqual(["alarm_volume_low"]);
  });

  it("warns about the baby battery between 5 and 15 percent unless it charges", () => {
    const low = (pct: number, charging = false): boolean =>
      ids(state({ peer: { batteryPct: pct, charging } })).includes("baby_battery_low");
    expect(BABY_BATTERY_LOW_PCT).toBe(15);
    expect(BATTERY_CRITICAL_PCT).toBe(5);
    expect(low(4)).toBe(false);
    expect(low(5)).toBe(true);
    expect(low(14)).toBe(true);
    expect(low(15)).toBe(false);
    expect(low(14, true)).toBe(false);
  });

  it("covers unplugged, silence, unit warnings and weak connection", () => {
    expect(ids(state({ peer: { unitWarnings: ["not_charging"] } }))).toEqual(["baby_unplugged"]);
    expect(ids(state({ peer: { silence: "may_ring" } }))).toEqual(["silence"]);
    expect(ids(state({ peer: { silence: "may_ring", isWebClient: true } }))).toEqual([]);
    expect(ids(state({ peer: { silence: "silent" } }))).toEqual([]);
    expect(ids(state({ peer: { unitWarnings: ["page_hidden", "wake_lock_off"] } }))).toEqual([
      "unit_warning_page_hidden",
      "unit_warning_wake_lock_off",
    ]);
    expect(ids(state({ link: { quality: "poor" } }))).toEqual(["weak_connection"]);
  });

  it("reports a pause until it ends and vibrate-only for the discreet style", () => {
    expect(ids(state({ snoozedUntil: 5_000 }), "sound", 1_000)).toEqual(["paused"]);
    expect(ids(state({ snoozedUntil: 5_000 }), "sound", 5_000)).toEqual([]);
    expect(ids(state(), "discreet")).toEqual(["vibrate_only"]);
  });
});

describe("browserBanners", () => {
  const guard = (over: Partial<PageGuardState> = {}): PageGuardState => ({
    wakeLock: "held",
    hidden: false,
    away: null,
    ...over,
  });

  it("always has the keep-open note, and no card for the sound gate (its panel is the surface)", () => {
    expect(browserBanners(guard()).map((x) => x.id)).toEqual(["browser_keep_open"]);
  });

  it("turns keep-open into a warning when the wake lock is lost or missing", () => {
    for (const wakeLock of ["lost", "unsupported"] as const) {
      const [only] = browserBanners(guard({ wakeLock }));
      expect(only?.id).toBe(`browser_keep_open_${wakeLock}`);
      expect(bannerLevel("browser_keep_open", only?.id)).toBe("warn");
    }
    expect(bannerLevel("browser_keep_open")).toBe("note");
  });

  it("adds the away report until it is dismissed", () => {
    const away = { kind: "background", awayMs: 60_000, endWallMs: 1 } as const;
    expect(browserBanners(guard({ away })).map((x) => x.kind)).toContain("browser_away");
  });

  it("ranks the browser kinds below readiness and above the baby device's warnings", () => {
    const ranked = rankBanners(
      [b("baby_battery_low"), b("browser_away"), b("readiness"), b("browser_sound_gate")],
      10,
    );
    expect(kinds(ranked.shown)).toEqual([
      "readiness",
      "browser_sound_gate",
      "browser_away",
      "baby_battery_low",
    ]);
  });
});

describe("the plain keep-open note folds first", () => {
  const plain = b("browser_keep_open");
  const lost = b("browser_keep_open", "browser_keep_open_lost");

  it("never hides a baby device safety notice behind it", () => {
    for (const safety of [
      b("mic_silent"),
      b("baby_battery_low"),
      b("baby_unplugged"),
      b("unit_warning", "unit_warning_page_hidden"),
      b("weak_connection"),
    ]) {
      const ranked = rankBanners([plain, safety, b("readiness", "never_tested")]);
      expect(
        ranked.shown.map((x) => x.id),
        safety.id,
      ).toContain(safety.id);
      expect(ranked.folded).toEqual([plain]);
    }
  });

  it("is shown when there is room, and ranks last even below Vibrate only", () => {
    expect(kinds(rankBanners([plain, b("vibrate_only")]).shown)).toEqual([
      "vibrate_only",
      "browser_keep_open",
    ]);
    expect(rankBanners([b("vibrate_only"), b("paused"), plain]).folded).toEqual([plain]);
  });

  it("keeps the rank of the wake lock warning variants, below readiness and above the rest", () => {
    const ranked = rankBanners([b("baby_battery_low"), lost, b("readiness", "never_tested")]);
    expect(ranked.shown.map((x) => x.id)).toEqual(["never_tested", "browser_keep_open_lost"]);
    expect(ranked.folded.map((x) => x.kind)).toEqual(["baby_battery_low"]);
  });
});

describe("mirror of Banners.kt", () => {
  it("has the web order without the browser kinds", () => {
    const at = kotlin.indexOf("enum class BannerKind {");
    const end = kotlin.indexOf("}", at);
    const names = [...kotlin.slice(at, end).matchAll(/^\s+([A-Z][A-Z_]*),?\s*$/gm)].map((m) =>
      (m[1] ?? "").toLowerCase(),
    );
    expect(names).toEqual(BANNER_ORDER.filter((k) => !k.startsWith("browser_")));
  });

  it("has the same constants", () => {
    const num = (name: string): number =>
      Number(new RegExp(`const val ${name} = (\\d+)`).exec(kotlin)?.[1]);
    // The battery levels live in `BabyBattery` of UiContract.kt (one constant per concept).
    const level = (name: string): number =>
      Number(new RegExp(`const val ${name} = (\\d+)`).exec(contract)?.[1]);
    expect(level("LOW_PCT")).toBe(BABY_BATTERY_LOW_PCT);
    expect(level("CRITICAL_PCT")).toBe(BATTERY_CRITICAL_PCT);
    expect(num("BANNER_LIMIT")).toBe(BANNER_LIMIT);
  });
});
