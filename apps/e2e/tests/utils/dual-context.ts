import { chromium, type BrowserContext, type Page } from "@playwright/test";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { E2E_APP_PREFERENCES_JSON } from "./e2e-bootstrap.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

export const fixturesDir = join(__dirname, "../fixtures");

const baseAudioArgs = [
  "--use-fake-device-for-media-stream",
  "--use-fake-ui-for-media-stream",
  "--autoplay-policy=no-user-gesture-required",
];

const ciChromeArgs = ["--disable-dev-shm-usage", "--no-sandbox"] as const;

function useContainerChromeArgs(): boolean {
  return process.env.CI === "true" || process.env.GUARDEDSLEEP_E2E_IN_DOCKER === "1";
}

export type DualContexts = {
  baby: BrowserContext;
  parent: BrowserContext;
  babyPage: Page;
  parentPage: Page;
  dispose: () => Promise<void>;
};

export async function launchDualContexts(opts: {
  babyFakeWav: string;
  parentFakeWav?: string;
  baseUrl?: string;
  /** Signaling URL seeded into both browsers (default: the e2e server). */
  signalUrl?: string;
  /** Browser permissions for both contexts (default: the microphone only). */
  permissions?: string[];
  /**
   * Local Nostr test relays (`ws://127.0.0.1:port`) the baby device names in its invite. The
   * development and E2E builds read them from localStorage; a release build reads nothing. Without
   * this option the list is empty, not unset: an unset list lets a dev or E2E client add the
   * curated public relays to its invites, and a test must never reach those.
   */
  nostrRelays?: string[];
}): Promise<DualContexts> {
  const baseUrl = opts.baseUrl ?? "http://127.0.0.1:5173";
  // A LAN IP over plain http is not a secure context; treat it as one so getUserMedia works.
  const insecureOrigin = /^http:\/\/(127\.0\.0\.1|localhost)[:/]/.test(baseUrl)
    ? []
    : [`--unsafely-treat-insecure-origin-as-secure=${baseUrl}`];
  const extra = [...(useContainerChromeArgs() ? ciChromeArgs : []), ...insecureOrigin];
  const babyArgs = [
    ...baseAudioArgs,
    ...extra,
    `--use-file-for-fake-audio-capture=${opts.babyFakeWav}`,
  ];
  const parentArgs =
    opts.parentFakeWav != null
      ? [...baseAudioArgs, ...extra, `--use-file-for-fake-audio-capture=${opts.parentFakeWav}`]
      : [...baseAudioArgs, ...extra];

  const headless = process.env.CI === "true" || process.env.HEADLESS !== "0";

  const browserBaby = await chromium.launch({
    headless,
    args: babyArgs,
  });
  const browserParent = await chromium.launch({
    headless,
    args: parentArgs,
  });

  const baby = await browserBaby.newContext({
    baseURL: baseUrl,
    ignoreHTTPSErrors: true,
    permissions: opts.permissions ?? ["microphone"],
  });
  const parent = await browserParent.newContext({
    baseURL: baseUrl,
    ignoreHTTPSErrors: true,
    permissions: opts.permissions ?? ["microphone"],
  });

  const signalWs = opts.signalUrl ?? process.env.E2E_SIGNALING_URL ?? "ws://127.0.0.1:18787";
  // addInitScript takes exactly one argument: pass an object, extra positional args are dropped.
  const seedStorage = ({
    url,
    prefs,
    nostr,
  }: {
    url: string;
    prefs: string;
    nostr: string | null;
  }) => {
    localStorage.setItem("guardedsleep:signaling-url", url);
    localStorage.setItem("guardedsleep:app-preferences", prefs);
    if (nostr !== null) localStorage.setItem("guardedsleep:dev-nostr-relays", nostr);
  };
  const seed = {
    url: signalWs,
    prefs: E2E_APP_PREFERENCES_JSON,
    nostr: JSON.stringify(opts.nostrRelays ?? []),
  };
  await baby.addInitScript(seedStorage, seed);
  await parent.addInitScript(seedStorage, seed);

  // The Vite dev client reloads the page after its socket was cut and restored (as
  // `context.setOffline()` does), which would forget the in-memory pairing key. E2E needs no
  // full reloads: keep the client (it injects CSS) but neutralize `location.reload()`.
  for (const ctx of [baby, parent]) {
    await ctx.route("**/@vite/client", async (route) => {
      const res = await route.fetch();
      const body = (await res.text()).replaceAll("location.reload()", "void 0");
      await route.fulfill({ response: res, body });
    });
  }

  const babyPage = await baby.newPage();
  const parentPage = await parent.newPage();

  return {
    baby,
    parent,
    babyPage,
    parentPage,
    dispose: async () => {
      await babyPage.close().catch(() => {});
      await parentPage.close().catch(() => {});
      await baby.close().catch(() => {});
      await parent.close().catch(() => {});
      await browserBaby.close().catch(() => {});
      await browserParent.close().catch(() => {});
    },
  };
}
