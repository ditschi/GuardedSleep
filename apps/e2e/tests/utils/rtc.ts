import type { Page } from "@playwright/test";

/*
 * Peer-connection probes for the audio and link specs. `rtcTrackerStub` is the body of a
 * `context.addInitScript` and runs inside the page, so it must not use anything from this module.
 * It only observes: the web client builds its connections exactly as in production.
 */

interface Tracked {
  pcs: RTCPeerConnection[];
  channels: RTCDataChannel[];
}

/** Records every RTCPeerConnection and DataChannel the page creates in `window.__rtc`. */
export const rtcTrackerStub = (): void => {
  const tracked: Tracked = { pcs: [], channels: [] };
  (window as unknown as { __rtc: Tracked }).__rtc = tracked;
  const Native = window.RTCPeerConnection;
  class Tracking extends Native {
    constructor(configuration?: RTCConfiguration) {
      super(configuration);
      tracked.pcs.push(this);
      this.addEventListener("datachannel", (event) => tracked.channels.push(event.channel));
    }
    createDataChannel(label: string, init?: RTCDataChannelInit): RTCDataChannel {
      const channel = super.createDataChannel(label, init);
      tracked.channels.push(channel);
      return channel;
    }
  }
  window.RTCPeerConnection = Tracking;
};

export interface AudioCounters {
  /** RTP packets received by the parent's audio receiver. */
  packets: number;
  /** Payload bytes received: a loud signal needs far more bytes per packet than digital silence. */
  bytes: number;
}

/** What the page's live peer connection has received as audio so far (zeros before any). */
export async function inboundAudio(page: Page): Promise<AudioCounters> {
  return page.evaluate(async () => {
    const { pcs } = (window as unknown as { __rtc: Tracked }).__rtc;
    const live = pcs.filter((pc) => pc.connectionState !== "closed").at(-1);
    const out = { packets: 0, bytes: 0 };
    if (!live) return out;
    for (const report of (await live.getStats()).values()) {
      if (report.type === "inbound-rtp" && report.kind === "audio") {
        out.packets += report.packetsReceived ?? 0;
        out.bytes += report.bytesReceived ?? 0;
      }
    }
    return out;
  });
}

/**
 * Average payload bytes per packet over the next `windowMs`. Headless Chromium reports
 * `totalAudioEnergy` for received audio as 0 (nothing pulls the playout), so the size of the
 * Opus frames stands in for loudness: digital silence is several times smaller than the pulses.
 */
export async function bytesPerPacket(page: Page, windowMs: number): Promise<number> {
  const before = await inboundAudio(page);
  await page.waitForTimeout(windowMs);
  const after = await inboundAudio(page);
  const packets = after.packets - before.packets;
  return packets > 0 ? (after.bytes - before.bytes) / packets : 0;
}

/**
 * Accumulated `totalAudioEnergy` of the baby device's microphone source as it enters the
 * connection, that is after the gate: it grows only while the gate is open.
 */
export async function sentAudioEnergy(page: Page): Promise<number> {
  return page.evaluate(async () => {
    const { pcs } = (window as unknown as { __rtc: Tracked }).__rtc;
    const live = pcs.filter((pc) => pc.connectionState !== "closed").at(-1);
    let energy = 0;
    if (!live) return energy;
    for (const report of (await live.getStats()).values()) {
      if (report.type === "media-source" && report.kind === "audio") {
        energy += report.totalAudioEnergy ?? 0;
      }
    }
    return energy;
  });
}

/** How many peer connections the page has opened so far. */
export async function peerConnectionCount(page: Page): Promise<number> {
  return page.evaluate(() => (window as unknown as { __rtc: Tracked }).__rtc.pcs.length);
}

/** Closes every open DataChannel: the media path survives, the core has to notice and recover. */
export async function dropDataChannels(page: Page): Promise<void> {
  await page.evaluate(() => {
    for (const channel of (window as unknown as { __rtc: Tracked }).__rtc.channels) channel.close();
  });
}

/**
 * Cuts the page's media path the way a lost network does: the peer connections are closed. (The
 * browser's offline emulation does not touch WebRTC media, only the signaling socket.)
 */
export async function closePeerConnections(page: Page): Promise<void> {
  await page.evaluate(() => {
    for (const pc of (window as unknown as { __rtc: Tracked }).__rtc.pcs) pc.close();
  });
}
