import { describe, expect, it } from "vitest";
// The files that decide how the shipped bundle is built are read as text, so they cannot drift.
import dockerfile from "../../../../../docker/web/Dockerfile?raw";
import prodServer from "../../../../e2e/tests/utils/prod-server.ts?raw";
import { PROJECT_SERVER } from "../public-lists/generated/public-lists";
import { loopbackAllowed, projectServerEnabled } from "./build-mode";

describe("loopbackAllowed", () => {
  it("is on for the dev server and the E2E build only", () => {
    expect(loopbackAllowed({ DEV: true, MODE: "development" })).toBe(true);
    expect(loopbackAllowed({ DEV: false, MODE: "e2e" })).toBe(true);
  });

  it("is never on in production mode", () => {
    expect(loopbackAllowed({ DEV: false, MODE: "production" })).toBe(false);
  });

  it("is never on in staging mode", () => {
    expect(loopbackAllowed({ DEV: false, MODE: "staging" })).toBe(false);
  });

  it("the web image builds in production mode and only the E2E server builds in e2e mode", () => {
    expect(dockerfile).toMatch(/ARG WEB_BUILD_MODE=production/);
    expect(dockerfile).toMatch(/vite build --mode "\$WEB_BUILD_MODE"/);
    // Only production and staging pass the Dockerfile's mode check; e2e and development never do.
    expect(dockerfile).toMatch(/production\|staging\)/);
    expect(dockerfile).not.toMatch(/--mode e2e/);
    expect(prodServer).toMatch(/"--mode",\s*"e2e"/);
    // The E2E bundle never lands in the directory a release or the image ships.
    expect(prodServer).toMatch(/"--outDir",\s*"dist-e2e"/);
    expect(prodServer).toMatch(/apps\/web\/dist-e2e"/);
    expect(prodServer).not.toMatch(/apps\/web\/dist"/);
  });
});

describe("projectServerEnabled", () => {
  const official = "true";
  it("is off for a release build, official or not, until task 6.2 flips it", () => {
    expect(
      projectServerEnabled({
        DEV: false,
        MODE: "production",
        VITE_GS_OFFICIAL_DEPLOYMENT: official,
      }),
    ).toBe(false);
    expect(projectServerEnabled({ DEV: false, MODE: "production" })).toBe(false);
  });

  it("is on for dev and E2E builds only together with the official-deployment flag", () => {
    expect(
      projectServerEnabled({
        DEV: true,
        MODE: "development",
        VITE_GS_OFFICIAL_DEPLOYMENT: official,
      }),
    ).toBe(true);
    expect(
      projectServerEnabled({ DEV: false, MODE: "e2e", VITE_GS_OFFICIAL_DEPLOYMENT: official }),
    ).toBe(true);
    expect(projectServerEnabled({ DEV: true, MODE: "development" })).toBe(false);
  });

  it("is on for a staging build only together with the official-deployment flag", () => {
    expect(
      projectServerEnabled({ DEV: false, MODE: "staging", VITE_GS_OFFICIAL_DEPLOYMENT: official }),
    ).toBe(true);
    expect(projectServerEnabled({ DEV: false, MODE: "staging" })).toBe(false);
  });

  it("the release value cannot be on while the data entry is pending", () => {
    const releaseOn = projectServerEnabled({
      DEV: false,
      MODE: "production",
      VITE_GS_OFFICIAL_DEPLOYMENT: official,
    });
    expect(!releaseOn || PROJECT_SERVER.live).toBe(true);
  });
});
