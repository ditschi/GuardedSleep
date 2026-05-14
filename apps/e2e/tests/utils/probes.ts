import type { Page } from "@playwright/test";

export async function waitForConnected(page: Page, timeoutMs = 45_000): Promise<void> {
  await page.waitForFunction(
    () => {
      const bar = document.querySelector('[data-testid="connection-status-bar"]');
      if (bar?.getAttribute("data-status") === "connected") return true;
      const pill = document.querySelector('[data-testid="connection-status"]');
      return pill?.getAttribute("data-session-state") === "connected";
    },
    undefined,
    { timeout: timeoutMs },
  );
  await page.waitForFunction(
    () => {
      const pc = window.__guardedsleep_test?.getSession?.()?.getE2ePeerConnection?.() ?? null;
      return pc?.connectionState === "connected";
    },
    undefined,
    { timeout: timeoutMs },
  );
}

export async function waitDataChannelOpen(page: Page, timeoutMs = 45_000): Promise<void> {
  await page.waitForFunction(
    () => window.__guardedsleep_test?.getSession?.()?.getE2eDataChannel?.()?.readyState === "open",
    undefined,
    { timeout: timeoutMs },
  );
}

export async function waitForPairingVerified(page: Page, timeoutMs = 45_000): Promise<void> {
  await page.waitForFunction(
    () => window.__guardedsleep_test?.getSession?.()?.getE2ePairingVerified?.() === true,
    undefined,
    { timeout: timeoutMs },
  );
}

export async function parentSendListen(page: Page, on: boolean): Promise<void> {
  await page.evaluate((enabled) => {
    window.__guardedsleep_test?.getSession?.()?.sendListen(enabled);
  }, on);
}

export type InboundAudioStats = {
  packetsReceived?: number;
  bytesReceived?: number;
  jitter?: number;
};

export async function readInboundAudioStats(page: Page): Promise<InboundAudioStats | null> {
  return page.evaluate(async () => {
    const pc = window.__guardedsleep_test?.getSession?.()?.getE2ePeerConnection?.() ?? null;
    if (!pc) return null;
    const stats = await pc.getStats();
    let best: InboundAudioStats | null = null;
    for (const rep of stats.values()) {
      if (rep.type === "inbound-rtp" && "kind" in rep && rep.kind === "audio") {
        const r = rep as RTCInboundRtpStreamStats;
        best = {
          packetsReceived: r.packetsReceived,
          bytesReceived: r.bytesReceived,
          jitter: r.jitter,
        };
      }
    }
    return best;
  });
}

export async function logRtcRoundTripSamples(page: Page): Promise<void> {
  const rows = await page.evaluate(async () => {
    const pc = window.__guardedsleep_test?.getSession?.()?.getE2ePeerConnection?.() ?? null;
    if (!pc) return [];
    const stats = await pc.getStats();
    const out: { id: string; type: string; rtt?: number }[] = [];
    for (const rep of stats.values()) {
      if (
        (rep.type === "remote-outbound-rtp" || rep.type === "remote-inbound-rtp") &&
        "roundTripTime" in rep &&
        typeof rep.roundTripTime === "number"
      ) {
        out.push({ id: rep.id, type: rep.type, rtt: rep.roundTripTime });
      }
    }
    return out;
  });
  console.info("[e2e] rtc RTT-related stats", rows);
}

export async function waitForFailed(page: Page, timeoutMs = 30_000): Promise<void> {
  await page
    .locator('[data-testid="connection-status"][data-session-state="failed"]')
    .first()
    .waitFor({
      state: "visible",
      timeout: timeoutMs,
    });
}

async function readConnectionPillSessionState(page: Page): Promise<string | null> {
  return page.evaluate(() => {
    const pill = document.querySelector('[data-testid="connection-status"]');
    return pill?.getAttribute("data-session-state")?.trim() ?? null;
  });
}

/** Resolves when either page shows `failed` (WebRTC / pairing failure). */
export async function waitForEitherFailed(
  babyPage: Page,
  parentPage: Page,
  timeoutMs = 35_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const [b, p] = await Promise.all([
      readConnectionPillSessionState(babyPage),
      readConnectionPillSessionState(parentPage),
    ]);
    if (b === "failed" || p === "failed") return;
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error("expected failed on baby or parent");
}

export async function parentAudioHasSrcObject(page: Page): Promise<boolean> {
  return page.evaluate(() => {
    const a = document.querySelector("audio");
    if (!a?.srcObject) return false;
    const ms = a.srcObject as MediaStream;
    return ms.getAudioTracks().length > 0;
  });
}
