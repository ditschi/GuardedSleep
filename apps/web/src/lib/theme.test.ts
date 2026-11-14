import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  applyTheme,
  isThemeSetting,
  resolveTheme,
  schemeFor,
  THEME_COLOR,
  readThemeSetting,
  storeThemeSetting,
  THEME_STORAGE_KEY,
  type ThemeSetting,
} from "./theme";

import bootScript from "../../public/theme-boot.js?raw";

describe("resolveTheme", () => {
  it("honours an explicit setting", () => {
    expect(resolveTheme("dark", false)).toBe("dark");
    expect(resolveTheme("light", true)).toBe("light");
  });

  it("follows the system for system", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
  });

  it("dims on the monitoring baby device regardless of the setting", () => {
    for (const s of ["dark", "light", "system"] as const) {
      expect(resolveTheme(s, false, true)).toBe("dim");
    }
  });

  it("validates stored values", () => {
    expect(isThemeSetting("system")).toBe(true);
    expect(isThemeSetting("dim")).toBe(false);
    expect(isThemeSetting(null)).toBe(false);
  });
});

describe("applyTheme", () => {
  it("sets data-theme and the browser chrome colour", () => {
    document.head.innerHTML = '<meta name="theme-color" content="#000000" />';
    applyTheme(document, "light");
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(document.documentElement.dataset.scheme).toBe("day");
    expect(document.querySelector('meta[name="theme-color"]')).toHaveAttribute(
      "content",
      THEME_COLOR.light,
    );
  });
});

/** Runs public/theme-boot.js against the current document, as the browser does before paint. */
function runBoot(): void {
  new Function(bootScript)();
}

describe("theme-boot.js agrees with resolveTheme", () => {
  afterEach(() => {
    window.localStorage.clear();
    delete document.documentElement.dataset.theme;
    vi.restoreAllMocks();
  });

  function bootWith(stored: string | null, prefersLight: boolean): string | undefined {
    document.head.innerHTML = '<meta name="theme-color" content="#000000" />';
    if (stored === null) window.localStorage.removeItem(THEME_STORAGE_KEY);
    else window.localStorage.setItem(THEME_STORAGE_KEY, stored);
    vi.spyOn(window, "matchMedia").mockImplementation(
      (query) => ({ matches: prefersLight && query.includes("light") }) as MediaQueryList,
    );
    runBoot();
    return document.documentElement.dataset.theme;
  }

  it("defaults to dark and the Dusk scheme without a stored setting", () => {
    expect(bootWith(null, true)).toBe("dark");
    expect(document.documentElement.dataset.scheme).toBe("dusk");
    expect(resolveTheme("dark", true)).toBe("dark");
  });

  it.each<[ThemeSetting, boolean]>([
    ["dark", true],
    ["dark", false],
    ["light", true],
    ["light", false],
    ["system", true],
    ["system", false],
  ])("setting %s, system prefers light: %s", (setting, prefersLight) => {
    const applied = resolveTheme(setting, !prefersLight);
    expect(bootWith(setting, prefersLight)).toBe(applied);
    expect(document.documentElement.dataset.scheme).toBe(schemeFor(applied));
  });

  it("ignores garbage and keeps the dark default", () => {
    expect(bootWith("neon", false)).toBe("dark");
  });

  it("paints the browser chrome like applyTheme", () => {
    bootWith("light", false);
    expect(document.querySelector('meta[name="theme-color"]')).toHaveAttribute(
      "content",
      THEME_COLOR.light,
    );
  });

  it("falls back to dark when storage throws", () => {
    document.head.innerHTML = "";
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    runBoot();
    expect(document.documentElement.dataset.theme).toBe("dark");
  });
});

describe("schemeFor", () => {
  it("uses Dusk by default and Day for a light theme", () => {
    expect(schemeFor("dark")).toBe("dusk");
    expect(schemeFor("light")).toBe("day");
    expect(schemeFor("dim")).toBe("dusk");
  });
});

// Read from disk: Vitest does not hand back CSS sources through `?raw`.
const themeCss = readFileSync(resolve(process.cwd(), "src/lib/ui/theme.css"), "utf8");

/** The custom properties a CSS rule with this exact selector sets. */
function tokens(selector: string): Map<string, string> {
  const out = new Map<string, string>();
  const css = themeCss.replace(/\/\*[\s\S]*?\*\//g, "");
  const rules = css.matchAll(/([^{}]+)\{([^{}]*)\}/g);
  for (const [, sel = "", body = ""] of rules) {
    if (!sel.split(",").some((part) => part.trim() === selector)) continue;
    for (const [, name = "", value = ""] of body.matchAll(/(--[\w-]+):\s*([^;]+);/g)) {
      out.set(name, value.trim());
    }
  }
  return out;
}

function luminance(hex: string): number {
  const channel = (i: number): number => {
    const c = parseInt(hex.slice(1 + i * 2, 3 + i * 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(0) + 0.7152 * channel(1) + 0.0722 * channel(2);
}

function contrast(a: string, b: string): number {
  const la = luminance(a);
  const lb = luminance(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

describe("colour scheme tokens", () => {
  const schemes = ["dusk", "day", "ember"] as const;

  it.each(schemes)("%s: body text reaches 4.5:1 on its surfaces", (scheme) => {
    const t = tokens(`:root[data-scheme="${scheme}"]`);
    for (const surface of ["--gs-bg", "--gs-surface", "--gs-surface-variant"]) {
      for (const text of ["--gs-on-bg", "--gs-on-surface-variant"]) {
        expect(contrast(t.get(text)!, t.get(surface)!), `${text} on ${surface}`).toBeGreaterThan(
          4.5,
        );
      }
    }
    expect(contrast(t.get("--gs-on-primary")!, t.get("--gs-primary")!)).toBeGreaterThan(4.5);
    expect(
      contrast(t.get("--gs-on-primary-container")!, t.get("--gs-primary-container")!),
    ).toBeGreaterThan(4.5);
    expect(contrast(t.get("--gs-primary")!, t.get("--gs-surface")!)).toBeGreaterThan(4.5);
  });

  it.each(schemes)("%s: monitor text reaches 7:1", (scheme) => {
    const t = tokens(`:root[data-scheme="${scheme}"]`);
    for (const surface of ["--gs-bg", "--gs-surface", "--gs-surface-variant"]) {
      expect(contrast(t.get("--gs-on-bg")!, t.get(surface)!), surface).toBeGreaterThanOrEqual(7);
    }
  });

  it("Dusk is also the :root default", () => {
    expect(tokens(":root").get("--gs-bg")).toBe(tokens(':root[data-scheme="dusk"]').get("--gs-bg"));
  });

  it("no scheme or theme overrides a safety token", () => {
    for (const scheme of schemes) {
      const names = [...tokens(`:root[data-scheme="${scheme}"]`).keys()];
      expect(names.filter((n) => n.startsWith("--safety-"))).toEqual([]);
    }
    const dim = [...tokens(':root[data-theme="dim"]').keys()];
    expect(dim.filter((n) => n.startsWith("--safety-"))).toEqual([]);
  });

  it("the safety set defines every role and the four alert palettes", () => {
    const names = new Set([...tokens(":root").keys()].filter((n) => n.startsWith("--safety-")));
    for (const role of ["info", "warn", "alert", "caution", "ok"]) {
      expect(names.has(`--safety-${role}`), role).toBe(true);
    }
    for (const palette of ["noise", "lost", "alarm", "battery"]) {
      for (const part of ["bg", "surface", "on", "accent", "slab", "on-slab", "slab-border"]) {
        expect(names.has(`--safety-${palette}-${part}`), `${palette}-${part}`).toBe(true);
      }
    }
  });

  it("alert slab text reaches 7:1", () => {
    const t = tokens(":root");
    for (const palette of ["noise", "lost", "alarm", "battery"]) {
      expect(
        contrast(t.get(`--safety-${palette}-on-slab`)!, t.get(`--safety-${palette}-slab`)!),
        palette,
      ).toBeGreaterThanOrEqual(7);
    }
    expect(contrast(t.get("--safety-on-info")!, t.get("--safety-info")!)).toBeGreaterThanOrEqual(7);
  });
});

/*
 * Cross-check of theme.css against the Compose theme (Theme.kt): the two hand-written token files
 * must carry the same colours. This test goes away once `add-quality-gates` generates both files
 * from tokens.json.
 */
const themeKt = readFileSync(
  resolve(process.cwd(), "../android/app/src/main/kotlin/org/guardedsleep/app/ui/theme/Theme.kt"),
  "utf8",
);

/** `name = Color(0xFFRRGGBB)` pairs of the Kotlin `val <block> =` declaration, as lowercase #rrggbb. */
function kotlinColors(block: string): Map<string, string> {
  const start = themeKt.indexOf(`val ${block} =`);
  if (start < 0) throw new Error(`Theme.kt has no val ${block}`);
  const end = themeKt.indexOf("\n    )\n", start);
  const out = new Map<string, string>();
  for (const [, name = "", hex = ""] of themeKt
    .slice(start, end)
    .matchAll(/(\w+) = Color\(0xFF([0-9A-Fa-f]{6})\)/g)) {
    out.set(name, `#${hex.toLowerCase()}`);
  }
  return out;
}

describe("theme.css matches Theme.kt", () => {
  const pairs = [
    ["dusk", "DuskScheme", "DuskApp"],
    ["day", "DayScheme", "DayApp"],
    ["ember", "EmberScheme", "EmberApp"],
  ] as const;
  const schemeMap: ReadonlyArray<readonly [string, string]> = [
    ["--gs-bg", "background"],
    ["--gs-surface", "surfaceContainer"],
    ["--gs-surface-low", "surfaceContainerLow"],
    ["--gs-surface-variant", "surfaceVariant"],
    ["--gs-surface-highest", "surfaceContainerHighest"],
    ["--gs-on-bg", "onBackground"],
    ["--gs-on-surface-variant", "onSurfaceVariant"],
    ["--gs-outline", "outline"],
    ["--gs-border", "outlineVariant"],
    ["--gs-primary-container", "primaryContainer"],
    ["--gs-secondary-container", "secondaryContainer"],
    ["--gs-tertiary-container", "tertiaryContainer"],
    ["--gs-error", "error"],
    ["--gs-on-error", "onError"],
  ];
  const appMap: ReadonlyArray<readonly [string, string]> = [
    ["--gs-ok", "ok"],
    ["--gs-warn", "warn"],
    ["--gs-bad", "bad"],
    ["--gs-chip", "neutralChip"],
    ["--gs-on-chip", "onNeutralChip"],
  ];

  it.each(pairs)("%s: scheme and text-level colours agree", (scheme, schemeBlock, appBlock) => {
    const css = tokens(`:root[data-scheme="${scheme}"]`);
    const scheme_ = kotlinColors(schemeBlock);
    for (const [prop, name] of schemeMap) {
      expect(scheme_.get(name), `${schemeBlock}.${name} exists`).toMatch(/^#[0-9a-f]{6}$/);
      expect(css.get(prop), `${prop} vs ${schemeBlock}.${name}`).toBe(scheme_.get(name));
    }
    const app = kotlinColors(appBlock);
    for (const [prop, name] of appMap) {
      expect(app.get(name), `${appBlock}.${name} exists`).toMatch(/^#[0-9a-f]{6}$/);
      expect(css.get(prop), `${prop} vs ${appBlock}.${name}`).toBe(app.get(name));
    }
  });

  it("the --safety-* colours are the colours of object SafetyColors", () => {
    const start = themeKt.indexOf("object SafetyColors {");
    const kotlin = new Set(
      [
        ...themeKt.slice(start, themeKt.indexOf("\n}\n", start)).matchAll(/0xFF([0-9A-Fa-f]{6})/g),
      ].map((m) => `#${(m[1] ?? "").toLowerCase()}`),
    );
    // var(...) values reference another safety token and carry no colour of their own.
    const css = new Set(
      [...tokens(":root")]
        .filter(([name, value]) => name.startsWith("--safety-") && value.startsWith("#"))
        .map(([, value]) => value.toLowerCase()),
    );
    expect(kotlin.size).toBeGreaterThan(20);
    expect([...css].sort()).toEqual([...kotlin].sort());
  });
});

describe("storeThemeSetting", () => {
  it("writes the setting under the key the boot script reads", () => {
    const set = vi.fn();
    storeThemeSetting({ setItem: set }, "light");
    expect(set).toHaveBeenCalledWith(THEME_STORAGE_KEY, "light");
  });

  it("carries on when storage is blocked", () => {
    expect(() =>
      storeThemeSetting(
        {
          setItem: () => {
            throw new DOMException("blocked", "SecurityError");
          },
        },
        "dark",
      ),
    ).not.toThrow();
  });
});

describe("readThemeSetting", () => {
  it("returns a stored setting and nothing for junk or blocked storage", () => {
    expect(readThemeSetting({ getItem: () => "light" })).toBe("light");
    expect(readThemeSetting({ getItem: () => "purple" })).toBeNull();
    expect(readThemeSetting({ getItem: () => null })).toBeNull();
    expect(
      readThemeSetting({
        getItem: () => {
          throw new DOMException("blocked", "SecurityError");
        },
      }),
    ).toBeNull();
  });
});
