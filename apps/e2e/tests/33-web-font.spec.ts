import { chromium, expect, test } from "@playwright/test";
import { startProdServer, type ProdServer } from "./utils/prod-server";

/**
 * Bundled typeface (`ui-appearance` "Bundled typeface", redesign-calm-ui task 1.3): Plus Jakarta Sans
 * is served from the web client's own origin and no request for it, or for anything else, leaves
 * the origin. The production build is served by `guardedsleep-server --serve-web`, as in local PC
 * mode.
 */

let prod: ProdServer;
test.beforeAll(async () => {
  test.setTimeout(300_000);
  prod = await startProdServer();
});
test.afterAll(() => prod?.stop());

test("the font loads from the own origin and no request goes elsewhere", async () => {
  // Names other than the local server do not resolve, so a stray request fails loudly in CI too.
  // Chromium flag: https://chromium.googlesource.com/chromium/src/+/HEAD/net/dns/README.md
  const browser = await chromium.launch({
    headless: true,
    args: ["--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE 127.0.0.1", "--no-sandbox"],
  });
  try {
    const context = await browser.newContext({ baseURL: prod.baseUrl });
    const page = await context.newPage();
    const origin = new URL(prod.baseUrl).origin;
    const requested: string[] = [];
    page.on("request", (request) => requested.push(request.url()));

    await page.goto("/");
    await expect(page.getByTestId("role_title")).toBeVisible();
    // Fonts load on first use of a face; wait until the 600 weight is ready.
    const loaded = await page.evaluate(async () => {
      await document.fonts.load('600 16px "Plus Jakarta Sans"');
      return document.fonts.check('600 16px "Plus Jakarta Sans"');
    });
    expect(loaded).toBe(true);

    const fonts = requested.filter((url) => /\.woff2?(\?|$)/.test(url));
    expect(fonts.length).toBeGreaterThan(0);
    for (const url of fonts) expect(new URL(url).origin).toBe(origin);
    const foreign = requested
      .filter((url) => url.startsWith("http"))
      .filter((url) => new URL(url).origin !== origin);
    expect(foreign).toEqual([]);
    expect(requested.join("\n")).not.toMatch(/fonts\.(googleapis|gstatic)\.com/);
  } finally {
    await browser.close();
  }
});
