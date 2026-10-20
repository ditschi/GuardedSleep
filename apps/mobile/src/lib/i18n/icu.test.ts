import { describe, expect, it } from "vitest";
import { rawBundles } from "./bundles";
import { formatMessage, parseMessage } from "./icu";
import { getByPath } from "./paths";

describe("formatMessage", () => {
  it("substitutes arguments and leaves missing ones visible", () => {
    expect(formatMessage("Hi {name}, {x}", { name: "Ada" }, "en")).toBe("Hi Ada, {x}");
  });

  it("returns plain text and unparsable templates unchanged", () => {
    expect(formatMessage("Plain — text", undefined, "en")).toBe("Plain — text");
    expect(formatMessage("broken {name", { name: "x" }, "en")).toBe("broken {name");
  });

  it("handles apostrophes like ICU", () => {
    expect(formatMessage("l'été it''s '{x}'", undefined, "fr")).toBe("l'été it's {x}");
  });

  it("selects plural categories per locale and formats #", () => {
    const m = "{count, plural, =0 {none} one {# item} other {# items}}";
    expect(formatMessage(m, { count: 0 }, "en")).toBe("none");
    expect(formatMessage(m, { count: 1 }, "en")).toBe("1 item");
    expect(formatMessage(m, { count: 1234 }, "en")).toBe("1,234 items");
    expect(formatMessage(m, { count: 1234 }, "de")).toBe("1.234 items");
  });

  it("uses the French many category and falls back to other", () => {
    const m = "{n, plural, one {# fois} many {# de fois} other {# fois}}";
    expect(formatMessage(m, { n: 1_000_000 }, "fr")).toContain("de fois");
    expect(formatMessage("{n, plural, other {x}}", { n: 1 }, "en")).toBe("x");
  });

  it("evaluates select with other fallback and nested arguments", () => {
    const m = "{role, select, baby {Baby {name}} other {Parent}}";
    expect(formatMessage(m, { role: "baby", name: "Lu" }, "en")).toBe("Baby Lu");
    expect(formatMessage(m, { role: "x" }, "en")).toBe("Parent");
  });
});

describe("generated bundles", () => {
  const walk = (o: unknown, out: string[] = []): string[] => {
    if (typeof o === "string") out.push(o);
    else if (typeof o === "object" && o !== null) Object.values(o).forEach((v) => walk(v, out));
    return out;
  };

  it("every message in every locale parses", () => {
    for (const [locale, bundle] of Object.entries(rawBundles)) {
      for (const msg of walk(bundle)) {
        expect(() => parseMessage(msg), `${locale}: ${msg}`).not.toThrow();
      }
    }
  });

  it("formats a real catalogue message per locale", () => {
    expect(
      formatMessage(
        getByPath(rawBundles.de, "settings.heartbeatOption") as string,
        { seconds: 5 },
        "de",
      ),
    ).toBe("5 Sekunden");
  });
});
