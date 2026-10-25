import { chromium, expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { createServer, type Server } from "node:http";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { cspProbeStub, cspViolations, wakeLockStub } from "./utils/browser-stubs";
import { fixturesDir, launchDualContexts, type DualContexts } from "./utils/dual-context";
import { startProdServer, type ProdServer } from "./utils/prod-server";
import { CLIPBOARD_PERMISSIONS } from "./utils/web";

/**
 * Content-Security-Policy (task 7.4), against the production build served by `guardedsleep-server
 * --serve-web`: the header is there and matches the <meta> fallback, every screen runs without a
 * single violation (no inline script or style, Trusted Types, same-origin WebSocket), and
 * cross-origin script is blocked however it is injected.
 */

let prod: ProdServer;
test.beforeAll(async () => {
  test.setTimeout(300_000);
  prod = await startProdServer();
});
test.afterAll(() => prod?.stop());

async function expectNoViolations(pages: Page[], where: string): Promise<void> {
  for (const page of pages) expect(await cspViolations(page), where).toEqual([]);
}

test("the header matches the meta fallback, and no screen violates the policy", async () => {
  test.setTimeout(240_000);
  const dual: DualContexts = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
    baseUrl: prod.baseUrl,
    permissions: CLIPBOARD_PERMISSIONS,
  });
  const { babyPage: baby, parentPage: parent } = dual;
  try {
    for (const ctx of [dual.baby, dual.parent]) {
      await ctx.addInitScript(cspProbeStub);
      await ctx.addInitScript(wakeLockStub);
    }

    // The header, and the <meta> in the page that carries the same policy.
    const response = await baby.goto("/");
    const header = response?.headers()["content-security-policy"] ?? "";
    expect(header).toContain("default-src 'self'");
    expect(header).toContain("require-trusted-types-for 'script'");
    expect(header).toContain("trusted-types guardedsleep svelte-trusted-html");
    expect(header).not.toMatch(/'unsafe-inline'|'unsafe-eval'|\*/);
    const meta = await baby
      .locator('meta[http-equiv="Content-Security-Policy"]')
      .getAttribute("content");
    expect(meta).toBeTruthy();
    expect(header.replace("frame-ancestors 'none'; ", "")).toBe(meta);
    await expect(baby.getByTestId("role_title")).toBeVisible();
    await expectNoViolations([baby], "role screen");

    // The page starts a service worker under Trusted Types.
    await expect
      .poll(() =>
        baby.evaluate(async () => (await navigator.serviceWorker.getRegistration())?.scope),
      )
      .toBe(`${prod.baseUrl}/`);

    // Pair with the default (same-origin) signaling address.
    await baby.getByTestId("role_baby").click();
    await expect(baby.getByTestId("baby_pair_qr")).toBeVisible({ timeout: 20_000 });
    await baby.getByTestId("baby_pair_copy").click();
    const code = await baby.evaluate(() => navigator.clipboard.readText());
    await expectNoViolations([baby], "baby pairing screen");

    await parent.goto("/");
    await parent.getByTestId("role_parent").click();
    await expectNoViolations([parent], "parent pairing screen");
    await parent.getByTestId("parent_pair_paste_field").fill(code);
    await parent.getByTestId("parent_pair_paste_submit").click();
    await expect(parent.getByTestId("parent_ambient_bar")).toHaveAttribute(
      "data-kind",
      "connected",
      {
        timeout: 60_000,
      },
    );
    await expect(baby.getByTestId("baby_status")).toHaveAttribute("data-state", "connected", {
      timeout: 60_000,
    });
    await expectNoViolations([baby, parent], "monitoring screens");

    // Dialogs, sheets, the night screen and the settings screen.
    await parent.getByTestId("parent_snooze").click();
    await expect(parent.getByTestId("sheet_snooze")).toBeVisible();
    await parent.getByText("Cancel").first().click();
    await parent.getByTestId("parent_night_open").click();
    await parent.getByTestId("night_screen_tap").click();
    await expect(parent.getByTestId("night_screen_status")).toBeVisible();
    await parent.keyboard.press("Escape");
    await baby.getByTestId("baby_night_open").click();
    await baby.keyboard.press("Escape");
    await parent.getByTestId("parent_overflow").click();
    await parent.getByTestId("parent_settings").click();
    await expect(parent.getByTestId("settings_back")).toBeVisible();
    await expectNoViolations([baby, parent], "sheets, night screen and settings");
    await parent.getByTestId("settings_back").click();

    // The full-screen link alarm.
    await baby.getByTestId("baby_stop").click();
    await baby.getByTestId("dialog_stop_confirm").click();
    await expect(parent.getByTestId("link_alarm_title")).toBeVisible({ timeout: 30_000 });
    await expectNoViolations([baby, parent], "link alarm");
  } finally {
    await dual.dispose();
  }
});

test("connect-src allows the curated relay origins and blocks an unlisted one", async () => {
  test.setTimeout(120_000);
  const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "../..", "..");
  const data = JSON.parse(readFileSync(join(repoRoot, "data/nostr-relays.json"), "utf8")) as {
    relays: { host: string; status: string }[];
  };
  const listed = data.relays.filter((r) => r.status === "active").map((r) => r.host);
  expect(listed.length).toBeGreaterThanOrEqual(6);
  const unlisted = "unlisted-relay.example.org";
  expect(listed).not.toContain(unlisted);

  // Every name except the local server fails to resolve: the sockets below never reach a public
  // relay (CI makes no request to a third party), but the CSP still decides before any lookup.
  // Chromium flag: https://chromium.googlesource.com/chromium/src/+/HEAD/net/dns/README.md
  const browser = await chromium.launch({
    headless: true,
    args: ["--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE 127.0.0.1", "--no-sandbox"],
  });
  try {
    const context = await browser.newContext({ baseURL: prod.baseUrl });
    await context.addInitScript(cspProbeStub);
    const page = await context.newPage();
    // The header carries each curated origin and no blanket wss:.
    const response = await page.goto("/");
    await expect(page.getByTestId("role_title")).toBeVisible();
    const header = response?.headers()["content-security-policy"] ?? "";
    for (const host of listed) expect(header).toContain(` wss://${host}`);
    expect(header).not.toContain(unlisted);
    expect(header).not.toMatch(/ wss:[ ;]/);

    const open = (hosts: string[]) =>
      page.evaluate(
        async (urls) => {
          for (const url of urls) {
            try {
              new WebSocket(url).close();
            } catch {
              // a malformed URL, not a policy decision
            }
          }
          // The violation events fire asynchronously, right after the attempts.
          await new Promise((resolve) => setTimeout(resolve, 500));
        },
        hosts.map((h) => `wss://${h}`),
      );
    await open([listed[0] ?? "", unlisted]);
    const blocked = (await cspViolations(page))
      .filter((v) => v.directive === "connect-src")
      .map((v) => v.blocked);
    expect(blocked.some((b) => b.includes(unlisted))).toBe(true);
    expect(blocked.some((b) => b.includes(listed[0] ?? "-"))).toBe(false);
  } finally {
    await browser.close();
  }
});

test("a cross-origin script is blocked however it is injected", async () => {
  test.setTimeout(180_000);
  // A server the page must never reach: it counts requests and would run code if it were loaded.
  const hits: string[] = [];
  const evil: Server = createServer((req, res) => {
    hits.push(req.url ?? "");
    res.setHeader("content-type", "text/javascript");
    res.setHeader("access-control-allow-origin", "*");
    res.end("window.__pwned = true;");
  });
  await new Promise<void>((resolve) => evil.listen(0, "127.0.0.1", resolve));
  const address = evil.address();
  const evilOrigin = `http://127.0.0.1:${typeof address === "object" && address ? address.port : 0}`;

  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    baseUrl: prod.baseUrl,
    permissions: CLIPBOARD_PERMISSIONS,
  });
  const page = dual.babyPage;
  try {
    await dual.baby.addInitScript(cspProbeStub);
    await page.goto("/");
    await expect(page.getByTestId("role_title")).toBeVisible();

    const results = await page.evaluate(async (origin) => {
      const outcome: Record<string, string> = {};
      const attempt = async (name: string, run: () => unknown): Promise<void> => {
        try {
          await run();
          outcome[name] = "allowed";
        } catch (error) {
          outcome[name] = error instanceof Error ? error.name : "blocked";
        }
      };
      // 1. a script element with a cross-origin src (a Trusted Types sink).
      await attempt("script-src", () => {
        const el = document.createElement("script");
        (el as { src: string }).src = `${origin}/a.js`;
        document.head.append(el);
      });
      // 2. inline code through a script element (a Trusted Types sink).
      await attempt("script-text", () => {
        const el = document.createElement("script");
        el.textContent = "window.__pwned = true";
        document.head.append(el);
      });
      // 3. markup with a script or handler (a Trusted Types sink).
      await attempt("inner-html", () => {
        document.body.innerHTML = `<img src=x onerror="window.__pwned=true">`;
      });
      await attempt("document-write", () => {
        document.write(`<script src="${origin}/b.js"><\/script>`);
      });
      // 4. a policy of our own.
      await attempt("create-policy", () => {
        (
          window as unknown as { trustedTypes: { createPolicy(n: string, r: object): unknown } }
        ).trustedTypes.createPolicy("evil", { createScriptURL: (s: string) => s });
      });
      // 5. dynamic import, which is not a Trusted Types sink: script-src alone must stop it.
      await attempt("import", () => import(/* @vite-ignore */ `${origin}/c.js`));
      // 6. network requests to another origin: connect-src.
      await attempt("fetch", () => fetch(`${origin}/d`, { mode: "no-cors" }));
      await attempt("websocket", () => {
        const ws = new WebSocket(origin.replace("http", "ws"));
        return new Promise((_, reject) => {
          ws.onerror = () => reject(new Error("blocked"));
          ws.onopen = () => undefined;
        });
      });
      return { outcome, pwned: (window as unknown as { __pwned?: boolean }).__pwned === true };
    }, evilOrigin);

    for (const [name, result] of Object.entries(results.outcome)) {
      expect(result, name).not.toBe("allowed");
    }
    expect(results.pwned).toBe(false);
    expect(hits).toEqual([]);

    const violations = await cspViolations(page);
    const directives = new Set(violations.map((v) => v.directive));
    expect(directives.has("script-src-elem") || directives.has("script-src")).toBe(true);
    expect(directives.has("connect-src")).toBe(true);
    expect(directives.has("require-trusted-types-for")).toBe(true);
    expect(directives.has("trusted-types")).toBe(true);
  } finally {
    await dual.dispose();
    evil.close();
  }
});
