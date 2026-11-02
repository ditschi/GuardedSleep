import { describe, expect, it } from "vitest";
// The files that decide how the shipped bundle is built are read as text, so they cannot drift.
import dockerfile from "../../../../../docker/web/Dockerfile?raw";
import prodServer from "../../../../e2e/tests/utils/prod-server.ts?raw";
import { loopbackAllowed, publicRelaysEnabled } from "./build-mode";

describe("loopbackAllowed", () => {
  it("is on for the dev server and the E2E build only", () => {
    expect(loopbackAllowed({ DEV: true, MODE: "development" })).toBe(true);
    expect(loopbackAllowed({ DEV: false, MODE: "e2e" })).toBe(true);
  });

  it("is never on in production mode", () => {
    expect(loopbackAllowed({ DEV: false, MODE: "production" })).toBe(false);
  });

  it("the web image builds in production mode and only the E2E server builds in e2e mode", () => {
    expect(dockerfile).toMatch(/vite build --mode production/);
    expect(dockerfile).not.toMatch(/--mode e2e/);
    expect(prodServer).toMatch(/"--mode",\s*"e2e"/);
    // The E2E bundle never lands in the directory a release or the image ships.
    expect(prodServer).toMatch(/"--outDir",\s*"dist-e2e"/);
    expect(prodServer).toMatch(/apps\/web\/dist-e2e"/);
    expect(prodServer).not.toMatch(/apps\/web\/dist"/);
  });
});

describe("publicRelaysEnabled (release gate until the disclosure and off switch exist)", () => {
  it("is on for the dev server and the E2E build", () => {
    expect(publicRelaysEnabled({ DEV: true, MODE: "development" })).toBe(true);
    expect(publicRelaysEnabled({ DEV: false, MODE: "e2e" })).toBe(true);
  });

  it("is OFF in production mode, which is how the release build and the web image are built", () => {
    expect(publicRelaysEnabled({ DEV: false, MODE: "production" })).toBe(false);
    expect(publicRelaysEnabled({ DEV: false, MODE: "test" })).toBe(false);
    expect(dockerfile).toMatch(/vite build --mode production/);
  });
});
