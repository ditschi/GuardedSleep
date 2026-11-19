import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { defaultLinkUi, type LinkUi } from "../contract";
import { kioskAttention, kioskMarkShape, kioskStatusLine, type KioskMarkShape } from "./kiosk";

const link = (phase: LinkUi["phase"], level = 0): LinkUi => ({ ...defaultLinkUi(), phase, level });

describe("kioskMarkShape", () => {
  it.each<[LinkUi, KioskMarkShape]>([
    [link("connected"), "bar"],
    [link("connecting"), "none"],
    [link("reconnecting", 0), "none"],
    [link("reconnecting", 1), "dashed"],
    [link("connecting", 1), "dashed"],
    [link("reconnecting", 2), "frame"],
    [link("reconnecting", 3), "frame"],
    [link("failed_needs_user"), "frame"],
    [link("peer_stopped"), "none"],
    [link("idle"), "none"],
    [link("stopped"), "none"],
  ])("%j is %s", (l, shape) => {
    expect(kioskMarkShape(l)).toBe(shape);
  });
});

describe("kioskAttention", () => {
  it("counts a nearly empty battery even while charging", () => {
    expect(kioskAttention({ pct: 4, charging: true }, false)).toBe("battery_critical");
  });
  it("says low only while not charging", () => {
    expect(kioskAttention({ pct: 12, charging: false }, false)).toBe("battery_low");
    expect(kioskAttention({ pct: 12, charging: true }, false)).toBeNull();
  });
  it("says not charging only after the charger was seen", () => {
    expect(kioskAttention({ pct: 80, charging: false }, true)).toBe("not_charging");
    expect(kioskAttention({ pct: 80, charging: false }, false)).toBeNull();
  });
  it("shows nothing without a battery reading", () => {
    expect(kioskAttention(null, true)).toBeNull();
  });
});

describe("kioskStatusLine", () => {
  const t = (key: string, args?: Record<string, string | number>): string =>
    key === "native.ui.kiosk.withName" ? `${args?.state} · ${args?.name}` : key;

  it("is 'State · Name' with a name and the plain state without", () => {
    const status = "native.ui.baby.status.waiting";
    expect(kioskStatusLine(t, link("connected"), status, "Travel cot")).toBe(
      "native.ui.kiosk.listening · Travel cot",
    );
    expect(kioskStatusLine(t, link("reconnecting", 1), status, null)).toBe(status);
  });

  it("leaves the name off the key-mismatch sentence", () => {
    expect(kioskStatusLine(t, link("failed_needs_user"), "core.pairingKeyMismatch", "Cot")).toBe(
      "core.pairingKeyMismatch",
    );
  });
});

// Read from disk: Vitest does not hand back CSS sources through `?raw`.
const themeCss = readFileSync(resolve(process.cwd(), "src/lib/ui/theme.css"), "utf8");
const themeKt = readFileSync(
  resolve(process.cwd(), "../android/app/src/main/kotlin/org/guardedsleep/app/ui/theme/Theme.kt"),
  "utf8",
);

function cssColour(name: string): string {
  const match = new RegExp(`${name}:\\s*(#[0-9a-fA-F]{6})`).exec(themeCss);
  if (match === null) throw new Error(`theme.css has no ${name}`);
  return (match[1] ?? "").toLowerCase();
}

function kotlinColour(name: string): string {
  const match = new RegExp(`val ${name} = Color\\(0xFF([0-9A-Fa-f]{6})\\)`).exec(themeKt);
  if (match === null) throw new Error(`Theme.kt has no ${name}`);
  return `#${(match[1] ?? "").toLowerCase()}`;
}

function luminance(hex: string): number {
  const channel = (i: number): number => {
    const c = parseInt(hex.slice(1 + i * 2, 3 + i * 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(0) + 0.7152 * channel(1) + 0.0722 * channel(2);
}

describe("kiosk mark colours", () => {
  it("no mark is brighter than the cap, #5a1414", () => {
    const cap = luminance(cssColour("--gs-kiosk-cap"));
    expect(cssColour("--gs-kiosk-cap")).toBe("#5a1414");
    for (const name of ["--gs-kiosk-ok", "--gs-kiosk-warn", "--gs-kiosk-alert"]) {
      expect(luminance(cssColour(name)), name).toBeLessThanOrEqual(cap);
    }
  });

  it("match the Compose kiosk palette", () => {
    expect(cssColour("--gs-kiosk-ok")).toBe(kotlinColour("Ok"));
    expect(cssColour("--gs-kiosk-warn")).toBe(kotlinColour("Warn"));
    expect(cssColour("--gs-kiosk-cap")).toBe(kotlinColour("Cap"));
    expect(cssColour("--gs-kiosk-text")).toBe(kotlinColour("KioskText"));
  });

  it("the status line text reaches 4.5:1 on black", () => {
    const l = luminance(cssColour("--gs-kiosk-text"));
    expect((l + 0.05) / 0.05).toBeGreaterThan(4.5);
  });
});
