import { describe, expect, it } from "vitest";

// AGENTS.md section 5.1: the UI sees the contract models and the controller interface only. The
// same check runs as `rg -n "core-wasm" apps/web/src/lib/ui apps/web/src/screens` in tasks.md.
const uiSources = import.meta.glob(
  ["./lib/ui/**/*.{ts,svelte,css}", "./screens/**/*.{ts,svelte}"],
  {
    query: "?raw",
    import: "default",
    eager: true,
  },
) as Record<string, string>;

describe("layer rules", () => {
  it("scans the UI sources", () => {
    expect(Object.keys(uiSources).length).toBeGreaterThan(10);
  });

  it("keeps the core bindings out of the UI", () => {
    for (const [file, source] of Object.entries(uiSources)) {
      // Test files may name the package in their own checks; production UI code may not import it.
      if (/\.test\.ts$/.test(file)) continue;
      expect(source, file).not.toMatch(/core-wasm|guardedsleep_wasm/);
    }
  });

  it("keeps the platform layer out of the UI", () => {
    for (const [file, source] of Object.entries(uiSources)) {
      expect(source, file).not.toMatch(/from\s+["'](?:\.\.\/)+platform\//);
    }
  });
});
