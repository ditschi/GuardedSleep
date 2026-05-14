import { chromium, type BrowserContext, type Page } from "@playwright/test";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

export const fixturesDir = join(__dirname, "../fixtures");

const baseAudioArgs = [
  "--use-fake-device-for-media-stream",
  "--use-fake-ui-for-media-stream",
  "--autoplay-policy=no-user-gesture-required",
];

const ciChromeArgs = ["--disable-dev-shm-usage", "--no-sandbox"] as const;

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
}): Promise<DualContexts> {
  const baseUrl = opts.baseUrl ?? "http://127.0.0.1:5173";
  const extra = process.env.CI === "true" ? [...ciChromeArgs] : [];
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
    permissions: ["microphone"],
  });
  const parent = await browserParent.newContext({
    baseURL: baseUrl,
    ignoreHTTPSErrors: true,
    permissions: ["microphone"],
  });

  const babyPage = await baby.newPage();
  const parentPage = await parent.newPage();

  const signalWs = process.env.E2E_SIGNALING_URL ?? "ws://127.0.0.1:18787";
  await babyPage.addInitScript((url: string) => {
    localStorage.setItem("guardedsleep:signaling-url", url);
  }, signalWs);
  await parentPage.addInitScript((url: string) => {
    localStorage.setItem("guardedsleep:signaling-url", url);
  }, signalWs);

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
