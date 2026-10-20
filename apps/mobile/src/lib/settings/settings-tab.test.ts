import { describe, expect, it } from "vitest";
import { defaultSettingsTab, parseSettingsTab, resolveSettingsTab } from "./settings-tab.js";

describe("settings-tab", () => {
  it("parses valid tab ids", () => {
    expect(parseSettingsTab("general")).toBe("general");
    expect(parseSettingsTab("child")).toBe("child");
    expect(parseSettingsTab("parent")).toBe("parent");
    expect(parseSettingsTab("nope")).toBeNull();
  });

  it("defaults from returnTo when tab missing", () => {
    expect(defaultSettingsTab("baby")).toBe("child");
    expect(defaultSettingsTab("parent")).toBe("parent");
    expect(defaultSettingsTab(null)).toBe("general");
  });

  it("prefers explicit tab over returnTo", () => {
    expect(resolveSettingsTab("general", "baby")).toBe("general");
    expect(resolveSettingsTab(null, "baby")).toBe("child");
  });
});
