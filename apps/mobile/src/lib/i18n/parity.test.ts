import { describe, expect, it } from "vitest";
import de from "./messages/de.json";
import en from "./messages/en.json";
import es from "./messages/es.json";
import fr from "./messages/fr.json";
import { collectParityIssues } from "./parity";

describe("i18n parity", () => {
  it("de, fr, and es contain the same keys as en (non-empty)", () => {
    const master = en as Record<string, unknown>;
    const issues = [
      ...collectParityIssues(master, "de", de as Record<string, unknown>),
      ...collectParityIssues(master, "fr", fr as Record<string, unknown>),
      ...collectParityIssues(master, "es", es as Record<string, unknown>),
    ];
    expect(issues, issues.join("\n")).toEqual([]);
  });
});
