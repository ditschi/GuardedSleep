import type { AlertStyle, PageGuardState, ParentMonitorState, ReadinessIssue } from "../contract";
import { BABY_BATTERY_LOW_PCT, BATTERY_CRITICAL_PCT } from "./parent-view";

/*
 * The notices on the parent monitor, ranked (`parent-alerts` "Banner priority on the monitor"):
 * at most two are shown, the rest fold into one "+N more" row. Pure presentation: every fact comes
 * from the model and nothing here decides about an alert or the link. The Android client mirrors
 * this file in `Banners.kt` (without the three browser kinds, which only a browser has) and the
 * Vitest case `banners.test.ts` reads that file to keep both lists equal.
 */

/** Rank order, most urgent first. The browser kinds sit right below readiness (`web-client`). */
export type BannerKind =
  | "security"
  | "mic_silent"
  | "readiness"
  | "browser_sound_gate"
  | "browser_keep_open"
  | "browser_away"
  | "baby_battery_low"
  | "baby_unplugged"
  | "silence"
  | "unit_warning"
  | "weak_connection"
  | "paused"
  | "vibrate_only";

export const BANNER_ORDER: readonly BannerKind[] = [
  "security",
  "mic_silent",
  "readiness",
  "browser_sound_gate",
  "browser_keep_open",
  "browser_away",
  "baby_battery_low",
  "baby_unplugged",
  "silence",
  "unit_warning",
  "weak_connection",
  "paused",
  "vibrate_only",
];

/** The most banners shown at once. */
export const BANNER_LIMIT = 2;

export interface Banner {
  kind: BannerKind;
  /** Test id and list key: the kind, or the kind with a detail (`unit_warning_page_hidden`). */
  id: string;
}

export interface RankedBanners {
  shown: Banner[];
  folded: Banner[];
}

/** A stable sort by rank: banners of one kind keep their input order. */
export function rankBanners(
  banners: readonly Banner[],
  limit: number = BANNER_LIMIT,
): RankedBanners {
  const rank = (b: Banner): number => BANNER_ORDER.indexOf(b.kind);
  const sorted = banners
    .map((b, i) => ({ b, i }))
    .sort((a, c) => rank(a.b) - rank(c.b) || a.i - c.i)
    .map((x) => x.b);
  return { shown: sorted.slice(0, limit), folded: sorted.slice(limit) };
}

export type BannerLevel = "note" | "info" | "warn" | "alert";

/**
 * warn and alert banners are announced once when they appear (live region on the card). The
 * keep-open banner turns into a warning when the browser let go of the screen wake lock.
 */
export function bannerLevel(kind: BannerKind, id: string = kind): BannerLevel {
  if (kind === "browser_keep_open" && id !== kind) return "warn";
  switch (kind) {
    case "security":
      return "alert";
    case "mic_silent":
    case "readiness":
    case "baby_battery_low":
    case "silence":
    case "unit_warning":
    case "weak_connection":
    case "browser_sound_gate":
      return "warn";
    case "baby_unplugged":
    case "paused":
    case "browser_away":
      return "info";
    case "vibrate_only":
    case "browser_keep_open":
      return "note";
  }
}

/** The warnings of the baby device that get a banner of their own besides "not charging". */
const UNIT_WARNINGS: readonly string[] = ["page_hidden", "wake_lock_off", "resumed_from_sleep"];

const SILENCE_WARNING: readonly string[] = [
  "screen_may_light",
  "may_buzz",
  "may_ring",
  "no_access",
];

/** The notices the monitor has right now, unranked (the screen ranks them). */
export function parentBanners(
  p: ParentMonitorState,
  alertStyle: AlertStyle,
  wallNowMs: number,
): Banner[] {
  const out: Banner[] = [];
  const add = (kind: BannerKind, id: string = kind): void => {
    out.push({ kind, id });
  };
  const connected = p.link.phase === "connected";

  if (p.securityWarning) add("security");
  if (connected && p.peer.micSilent === true) add("mic_silent");
  if (connected && p.peer.unitWarnings.includes("mic_permission_lost")) {
    add("mic_silent", "mic_permission_lost");
  }
  const worst: ReadinessIssue | undefined = p.readiness[0];
  if (worst !== undefined) add("readiness", worst);

  const pct = p.peer.batteryPct;
  if (
    connected &&
    pct !== null &&
    pct >= BATTERY_CRITICAL_PCT &&
    pct < BABY_BATTERY_LOW_PCT &&
    p.peer.charging !== true
  ) {
    add("baby_battery_low");
  }
  if (connected && p.peer.unitWarnings.includes("not_charging")) add("baby_unplugged");
  if (connected && !p.peer.isWebClient && SILENCE_WARNING.includes(p.peer.silence)) {
    add("silence");
  }
  if (connected) {
    for (const w of p.peer.unitWarnings) {
      if (UNIT_WARNINGS.includes(w)) add("unit_warning", `unit_warning_${w}`);
    }
  }
  if (connected && p.link.quality === "poor") add("weak_connection");
  if (p.snoozedUntilWallMs !== null && p.snoozedUntilWallMs > wallNowMs) add("paused");
  if (alertStyle === "discreet") add("vibrate_only");
  return out;
}

/**
 * The notices only a browser parent has (`web-client` "Same monitor in the browser"): "keep this
 * page open" (a warning with an id suffix when the screen wake lock is lost or missing) and "back
 * from the background". They rank below readiness with the rest and are fed from the page guard,
 * never from the core. The alert sound gate is a kind of its own in the order but has no card: its
 * panel covers the monitor while the sound is locked, so a banner would say it twice.
 */
export function browserBanners(guard: PageGuardState): Banner[] {
  const out: Banner[] = [];
  if (guard.wakeLock === "lost" || guard.wakeLock === "unsupported") {
    out.push({ kind: "browser_keep_open", id: `browser_keep_open_${guard.wakeLock}` });
  } else {
    out.push({ kind: "browser_keep_open", id: "browser_keep_open" });
  }
  if (guard.away !== null) out.push({ kind: "browser_away", id: "browser_away" });
  return out;
}
