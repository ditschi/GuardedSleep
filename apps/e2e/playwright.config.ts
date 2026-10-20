import { defineConfig, devices } from "@playwright/test";

/** Dedicated port so `pnpm run e2e` does not collide with a developer `dev:local` on 8787. */
const e2eSignalPort = process.env.GUARDEDSLEEP_E2E_SIGNAL_PORT ?? "18787";
const e2eSignalWs = `ws://127.0.0.1:${e2eSignalPort}`;
process.env.E2E_SIGNALING_URL = process.env.E2E_SIGNALING_URL ?? e2eSignalWs;

export default defineConfig({
  testDir: "./tests",
  timeout: 90_000,
  expect: { timeout: 20_000 },
  fullyParallel: false,
  workers: 1,
  reporter: [["list"], ["html", { open: "never" }]],
  use: {
    baseURL: "http://127.0.0.1:5173",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "off",
  },
  webServer: {
    command: "pnpm run dev:local",
    cwd: "../..",
    url: "http://127.0.0.1:5173",
    reuseExistingServer: process.env.PW_REUSE_EXISTING_SERVER === "1",
    timeout: 120_000,
    stdout: "pipe",
    stderr: "pipe",
    env: {
      PORT: e2eSignalPort,
      // Many pairings per minute from one IP, and the server caps parents at 1 per room.
      SIGNALING_HELLO_RATE_PER_MIN: "0",
      SIGNALING_NEW_ROOM_RATE_PER_MIN: "0",
    },
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
});
