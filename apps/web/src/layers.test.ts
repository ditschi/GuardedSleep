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

  it("lets only the platform layer import the core bindings", () => {
    for (const [file, source] of Object.entries(appSources)) {
      expect(source, file).not.toMatch(/@guardedsleep\/core-wasm|guardedsleep_wasm/);
    }
  });

  it("keeps test doubles out of the production bundle", () => {
    for (const [file, source] of Object.entries(productionSources)) {
      expect(source, file).not.toMatch(/fake-controller|test-support|test-audio/);
    }
  });
});

// Everything but the platform layer and tests: it must not reach the core bindings itself.
const appSources = import.meta.glob(
  [
    "./**/*.{ts,svelte}",
    "!./lib/platform/**",
    "!./**/*.test.ts",
    "!./test/**",
    "!./lib/contract/fake-controller.ts",
  ],
  { query: "?raw", import: "default", eager: true },
) as Record<string, string>;

// All production sources, platform included: none may import a test double.
const productionSources = import.meta.glob(
  [
    "./**/*.{ts,svelte}",
    "!./**/*.test.ts",
    "!./test/**",
    "!./lib/contract/fake-controller.ts",
    "!./lib/platform/test-support.ts",
    "!./lib/platform/test-audio.ts",
  ],
  { query: "?raw", import: "default", eager: true },
) as Record<string, string>;
