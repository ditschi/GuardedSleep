import type { Page } from "@playwright/test";
import { logConnectionDiagnostics } from "./connection-diagnostics.js";

const BAR = '[data-testid="parent_ambient_bar"],[data-testid="baby_status"]';

export async function waitForConnected(page: Page, timeoutMs = 45_000): Promise<void> {
  try {
    await page.waitForFunction(
      (sel) => document.querySelector(sel)?.getAttribute("data-state") === "connected",
      BAR,
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
  } catch (err) {
    await logConnectionDiagnostics("waitForConnected", page, page);
    throw err;
  }
}

export type BarSnapshot = { state: string | null; level: number | null };

export async function readBar(page: Page): Promise<BarSnapshot> {
  return page.evaluate((sel) => {
    const el = document.querySelector(sel);
    const level = el?.getAttribute("data-level");
    return {
      state: el?.getAttribute("data-state") ?? null,
      level: level == null ? null : Number(level),
    };
  }, BAR);
}

export async function waitForLevel(page: Page, atLeast: number, timeoutMs: number): Promise<void> {
  await page.waitForFunction(
    ({ sel, min }) => {
      const el = document.querySelector(sel);
      return (
        el != null &&
        el.getAttribute("data-state") !== "connected" &&
        Number(el.getAttribute("data-level")) >= min
      );
    },
    { sel: BAR, min: atLeast },
    { timeout: timeoutMs },
  );
}

/** Level-2 troubleshooter cause hint (`server_unreachable`, `peer_absent`, `media_blocked`). */
export async function waitForCause(page: Page, cause: string, timeoutMs: number): Promise<void> {
  await page
    .locator(`[data-testid="link_cause"][data-cause="${cause}"]`)
    .first()
    .waitFor({ state: "visible", timeout: timeoutMs });
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

export async function parentAudioHasSrcObject(page: Page): Promise<boolean> {
  return page.evaluate(() => {
    const a = document.querySelector("audio");
    if (!a?.srcObject) return false;
    const ms = a.srcObject as MediaStream;
    return ms.getAudioTracks().length > 0;
  });
}

export type RemoteAudioRmsSample = {
  maxRms: number;
  avgRms: number;
  samples: number;
};

/** Minimum maxRms for loud fixtures / synthetic tone (not silence). */
export const E2E_MIN_LOUD_MAX_RMS = 0.012;

/** Upper bound for silence fixtures (guard against false positives). */
export const E2E_MAX_SILENCE_MAX_RMS = 0.008;

export async function sampleParentRemoteAudioRms(
  page: Page,
  opts?: { windowMs?: number; intervalMs?: number },
): Promise<RemoteAudioRmsSample> {
  return page.evaluate(async (o) => {
    const sample = await window.__guardedsleep_test?.sampleRemoteAudioRms?.(o);
    return sample ?? { maxRms: 0, avgRms: 0, samples: 0 };
  }, opts ?? {});
}

export async function waitForParentRemoteAudioAbove(
  page: Page,
  minMaxRms: number,
  timeoutMs = 25_000,
  opts?: { windowMs?: number; intervalMs?: number },
): Promise<RemoteAudioRmsSample> {
  const deadline = Date.now() + timeoutMs;
  let last: RemoteAudioRmsSample = { maxRms: 0, avgRms: 0, samples: 0 };
  while (Date.now() < deadline) {
    last = await sampleParentRemoteAudioRms(page, opts);
    if (last.maxRms >= minMaxRms && last.samples > 0) return last;
    await new Promise((r) => setTimeout(r, 300));
  }
  return last;
}

export async function readBabyLocalAudioTrackEnabled(page: Page): Promise<boolean | null> {
  return page.evaluate(
    () => window.__guardedsleep_test?.getSession?.()?.getE2eBabyLocalAudioTrackEnabled?.() ?? null,
  );
}
