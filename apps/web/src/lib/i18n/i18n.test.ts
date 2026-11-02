import { describe, expect, it } from "vitest";
import {
  BUNDLES,
  DIRECTIONS,
  directionOf,
  ENDONYMS,
  LOCALES,
  resolveLocale,
  type Bundle,
} from "./bundles";
import { formatMessage } from "./icu";
import { createTranslator } from "./translate";
import { ROUTES } from "../router";

describe("formatMessage", () => {
  it("substitutes arguments and keeps unknown ones visible", () => {
    expect(formatMessage("Since {time}", { time: "02:14" }, "en")).toBe("Since 02:14");
    expect(formatMessage("Since {time}", {}, "en")).toBe("Since {time}");
  });

  const sounds = "{count, plural, =0 {No sounds} one {# sound} other {# sounds}}";

  it("picks plural categories, exact matches first", () => {
    expect(formatMessage(sounds, { count: 0 }, "en")).toBe("No sounds");
    expect(formatMessage(sounds, { count: 1 }, "en")).toBe("1 sound");
    expect(formatMessage(sounds, { count: 1234 }, "en")).toBe("1,234 sounds");
  });

  it("uses the locale's plural rules and number format", () => {
    const fr = "{n, plural, one {# item} many {# of items} other {# items}}";
    expect(formatMessage(fr, { n: 0 }, "fr")).toBe("0 item");
    expect(formatMessage(fr, { n: 1_000_000 }, "fr")).toBe("1 000 000 of items");
    expect(formatMessage("{n, plural, one {# Ding} other {# Dinge}}", { n: 1500 }, "de")).toBe(
      "1.500 Dinge",
    );
  });

  it("selects by value with an other fallback", () => {
    const m = "{role, select, baby {Baby device} parent {Parent device} other {Device}}";
    expect(formatMessage(m, { role: "baby" }, "en")).toBe("Baby device");
    expect(formatMessage(m, { role: "tablet" }, "en")).toBe("Device");
    expect(formatMessage(m, {}, "en")).toBe("Device");
  });

  it("nests arguments inside cases", () => {
    const m = "{n, plural, one {1 sound at {time}} other {# sounds at {time}}}";
    expect(formatMessage(m, { n: 3, time: "02:14" }, "en")).toBe("3 sounds at 02:14");
  });

  it("handles apostrophes: doubled, lone and quoting braces", () => {
    expect(formatMessage("It''s {x}", { x: 1 }, "en")).toBe("It's 1");
    expect(formatMessage("Don't panic", undefined, "en")).toBe("Don't panic");
    expect(formatMessage("Use '{'braces'}'", undefined, "en")).toBe("Use {braces}");
  });

  it("shows a template it cannot parse as written", () => {
    expect(formatMessage("Broken {n, plural, one {x}", { n: 1 }, "en")).toBe(
      "Broken {n, plural, one {x}",
    );
    expect(formatMessage("Stray }", undefined, "en")).toBe("Stray }");
  });

  it("leaves a pound outside a plural alone", () => {
    expect(formatMessage("Room #5", undefined, "en")).toBe("Room #5");
  });
});

describe("createTranslator", () => {
  const bundles: Record<string, Bundle> = {
    en: { a: { hello: "Hello {name}", only: "English only" } },
    de: { a: { hello: "Hallo {name}" } },
  };

  it("translates and formats", () => {
    const t = createTranslator("de", { bundles });
    // @ts-expect-error -- the test bundles are not the generated catalogue, so the key type does not know "a.hello"
    expect(t("a.hello", { name: "Mia" })).toBe("Hallo Mia");
  });

  it("falls back to English for a missing non-critical key", () => {
    const t = createTranslator("de", { bundles });
    // @ts-expect-error -- see above: test-only key
    expect(t("a.only")).toBe("English only");
  });

  it("never mixes languages for a critical key", () => {
    const t = createTranslator("de", { bundles, criticalKeys: new Set(["a.only"]) });
    // @ts-expect-error -- see above: test-only key
    expect(t("a.only")).toBe("a.only");
  });

  it("returns the key when no locale has it", () => {
    const t = createTranslator("en", { bundles });
    // @ts-expect-error -- deliberately not a catalogue key
    expect(t("a.nothing")).toBe("a.nothing");
  });
});

describe("generated catalogue", () => {
  it("has the English master and every shipped locale", () => {
    for (const l of LOCALES) expect(BUNDLES[l]).toBeDefined();
  });

  it("resolves every route title in every locale", () => {
    for (const l of LOCALES) {
      const t = createTranslator(l);
      for (const r of ROUTES) {
        const title = t(r.titleKey);
        expect(title, `${l} ${r.titleKey}`).not.toBe(r.titleKey);
        expect(title.length).toBeGreaterThan(0);
      }
    }
  });

  it("uses the device wording in the new web keys", () => {
    const en = createTranslator("en");
    expect(en("webShell.route.baby")).toBe("Baby device");
    expect(en("webShell.route.parent")).toBe("Parent device");
  });
});

describe("resolveLocale", () => {
  it("prefers an explicit choice", () => {
    expect(resolveLocale("de", ["fr"])).toBe("de");
  });

  it("follows the browser languages, region tags included", () => {
    expect(resolveLocale(null, ["sw-KE", "de-AT", "en"])).toBe("de");
    expect(resolveLocale(null, ["ES"])).toBe("es");
    expect(resolveLocale(null, ["pt-BR"])).toBe("pt");
    expect(resolveLocale(null, ["zh-CN"])).toBe("zh-Hans");
    expect(resolveLocale(null, ["zh-Hans"])).toBe("zh-Hans");
  });

  it("falls back to English", () => {
    expect(resolveLocale(null, ["sw"])).toBe("en");
    expect(resolveLocale("xx", [])).toBe("en");
    expect(resolveLocale(null, [])).toBe("en");
  });
});

describe("language choice data", () => {
  it("names every shipped locale in its own language", () => {
    expect(LOCALES).toHaveLength(20);
    expect(ENDONYMS.de).toBe("Deutsch");
    expect(ENDONYMS.ja).toBe("日本語");
    expect(ENDONYMS["zh-Hans"]).toBe("中文（简体）");
    expect(Object.keys(ENDONYMS).sort()).toEqual([...LOCALES].sort());
  });

  it("sets right-to-left for Arabic only, from the generated table", () => {
    expect(Object.keys(DIRECTIONS).sort()).toEqual([...LOCALES].sort());
    expect(LOCALES.filter((l) => DIRECTIONS[l] === "rtl")).toEqual(["ar"]);
    expect(directionOf("ar")).toBe("rtl");
    expect(directionOf("de")).toBe("ltr");
    expect(directionOf("zh-Hans")).toBe("ltr");
  });
});
