import { expect, test, type Page } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { CLIPBOARD_PERMISSIONS, pairWeb } from "./utils/web";

/**
 * Task 5.3 and AGENTS.md section 7.2: the PSK and the pairing material live in page memory only.
 * After pairing, and again after a reload, localStorage, sessionStorage, IndexedDB, the Cache API
 * and the cookies hold no key, no invite and no room id, and neither does the address bar.
 */

/** Every readable byte of the page's storage as one string. */
async function dumpStorage(page: Page): Promise<string> {
  return page.evaluate(async () => {
    const dump: Record<string, unknown> = {
      localStorage: Object.fromEntries(Object.entries(localStorage)),
      sessionStorage: Object.fromEntries(Object.entries(sessionStorage)),
      url: location.href,
    };

    const databases: Record<string, unknown> = {};
    for (const info of (await indexedDB.databases?.()) ?? []) {
      if (!info.name) continue;
      const db = await new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open(info.name as string);
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error);
      });
      const stores: Record<string, unknown> = {};
      for (const name of Array.from(db.objectStoreNames)) {
        stores[name] = await new Promise((resolve, reject) => {
          const request = db.transaction(name).objectStore(name).getAll();
          request.onsuccess = () => resolve(request.result);
          request.onerror = () => reject(request.error);
        });
      }
      db.close();
      databases[info.name] = stores;
    }
    dump.indexedDB = databases;

    const cacheDump: Record<string, unknown> = {};
    for (const name of await caches.keys()) {
      const cache = await caches.open(name);
      const entries: Array<{ url: string; text?: string }> = [];
      for (const request of await cache.keys()) {
        const response = await cache.match(request);
        const type = response?.headers.get("content-type") ?? "";
        entries.push({
          url: request.url,
          text: /text|json|javascript|html|xml/.test(type) ? await response?.text() : undefined,
        });
      }
      cacheDump[name] = entries;
    }
    dump.caches = cacheDump;
    return JSON.stringify(dump);
  });
}

function watchRoomIds(page: Page, roomIds: Set<string>): void {
  page.on("websocket", (socket) => {
    const read = (frame: { payload: string | Buffer }): void => {
      try {
        const message = JSON.parse(String(frame.payload)) as { roomId?: string };
        if (message.roomId) roomIds.add(message.roomId);
      } catch {
        // Not a JSON text frame.
      }
    };
    socket.on("framereceived", read);
    socket.on("framesent", read);
  });
}

test("pairing material is in memory only: pair, reload, nothing in storage", async () => {
  test.setTimeout(150_000);
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
    permissions: CLIPBOARD_PERMISSIONS,
  });
  const roomIds = new Set<string>();
  try {
    watchRoomIds(dual.babyPage, roomIds);
    watchRoomIds(dual.parentPage, roomIds);

    const invite = await pairWeb(dual);
    const key = (JSON.parse(invite) as { k: string }).k;
    expect(key).toHaveLength(43);
    expect([...roomIds].length, "the relay told us the room id").toBeGreaterThan(0);
    const secrets = {
      invite,
      key,
      ...Object.fromEntries([...roomIds].map((r, i) => [`room${i}`, r])),
    };

    const expectNoSecrets = async (page: Page, when: string): Promise<void> => {
      const dump = await dumpStorage(page);
      for (const [name, secret] of Object.entries(secrets)) {
        expect(dump.includes(secret), `${name} found in storage ${when}`).toBe(false);
      }
      const cookies = JSON.stringify(await page.context().cookies());
      expect(cookies.includes(key), `key found in cookies ${when}`).toBe(false);
    };

    for (const page of [dual.babyPage, dual.parentPage])
      await expectNoSecrets(page, "while paired");

    // A reload forgets the session: the monitoring screens give way to the start screen.
    await Promise.all([dual.babyPage.reload(), dual.parentPage.reload()]);
    for (const page of [dual.babyPage, dual.parentPage]) {
      await expect(page.getByTestId("role_title")).toBeVisible({ timeout: 20_000 });
      await expectNoSecrets(page, "after the reload");
    }

    // Opening a monitoring address without a session does not resurrect one either.
    await dual.parentPage.goto("/parent");
    await expect(dual.parentPage.getByTestId("role_title")).toBeVisible();
    await expect(dual.parentPage).toHaveURL(/\/$/);
  } finally {
    await dual.dispose();
  }
});
