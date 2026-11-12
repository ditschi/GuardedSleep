import { expect, test, type Page } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { CLIPBOARD_PERMISSIONS, pairWeb } from "./utils/web";

/**
 * Talk-back between two browsers: a web parent holds the talk button, a web baby plays the voice.
 * The fake parent microphone replays pulses; the baby's microphone is replaced from outside by a
 * quiet room with an acoustic loopback of whatever the page sends to its loudspeaker (a headless
 * browser has no speaker and no room), so the baby's own playback check can hear itself. The
 * stub also records what is connected to the audio destination and can mute that output. It
 * changes no page code; the page has no test hooks (README, "No test hooks in the page").
 */

const pulses = join(fixturesDir, "pulses-30s.wav");
const silence = join(fixturesDir, "silence-30s.wav");

/** Runs inside the baby page before the app: a room that hears the speaker, and a mutable output. */
const loopbackRoom = (): void => {
  interface Stub {
    __ctx?: AudioContext;
    __loop?: GainNode;
    __mute?: boolean;
    __connected?: Set<AudioNode>;
    __destCount?: () => number;
  }
  const w = window as unknown as Stub & Window;
  const Native = window.AudioContext;
  const connected = new Set<AudioNode>();
  w.__connected = connected;
  w.__destCount = () => connected.size;
  // One context for the page and for the stubbed microphone: nodes of two contexts cannot connect.
  const shared = (options?: AudioContextOptions): AudioContext => {
    w.__ctx ??= new Native(options);
    return w.__ctx;
  };
  window.AudioContext = function (options?: AudioContextOptions) {
    return shared(options);
  } as unknown as typeof AudioContext;

  const nativeConnect = AudioNode.prototype.connect as (
    this: AudioNode,
    ...args: unknown[]
  ) => unknown;
  const nativeDisconnect = AudioNode.prototype.disconnect as (
    this: AudioNode,
    ...args: unknown[]
  ) => void;
  AudioNode.prototype.connect = function (this: AudioNode, ...args: unknown[]) {
    const target = args[0];
    if (target instanceof AudioDestinationNode) {
      // A muted output: nothing reaches the speaker, so nothing reaches the room either.
      if (w.__mute) return target;
      connected.add(this);
      if (w.__loop) nativeConnect.call(this, w.__loop);
    }
    return nativeConnect.apply(this, args);
  } as typeof AudioNode.prototype.connect;
  AudioNode.prototype.disconnect = function (this: AudioNode, ...args: unknown[]) {
    if (args.length === 0) connected.delete(this);
    return nativeDisconnect.apply(this, args);
  } as typeof AudioNode.prototype.disconnect;

  navigator.mediaDevices.getUserMedia = async () => {
    const ctx = shared();
    const room = ctx.createMediaStreamDestination();
    // A quiet, steady room.
    const hum = ctx.createOscillator();
    const level = ctx.createGain();
    hum.frequency.value = 180;
    level.gain.value = 0.003;
    hum.connect(level);
    level.connect(room);
    hum.start();
    // The room hears the loudspeaker.
    w.__loop = ctx.createGain();
    w.__loop.connect(room);
    return room.stream;
  };
};

/** Runs inside the parent page before the app: remembers its peer connections for the stats read. */
const trackPeerConnections = (): void => {
  const w = window as unknown as { __pcs: RTCPeerConnection[] };
  w.__pcs = [];
  const Native = window.RTCPeerConnection;
  window.RTCPeerConnection = function (...args: ConstructorParameters<typeof Native>) {
    const pc = new Native(...args);
    w.__pcs.push(pc);
    return pc;
  } as unknown as typeof RTCPeerConnection;
  window.RTCPeerConnection.prototype = Native.prototype;
};

/** Audio packets the parent page has sent: its only sender is the talk line (monitoring is receive-only). */
const packetsSent = (page: Page): Promise<number> =>
  page.evaluate(async () => {
    const pcs = (window as unknown as { __pcs: RTCPeerConnection[] }).__pcs;
    let sent = 0;
    for (const pc of pcs) {
      const report = await pc.getStats();
      report.forEach((stat: { type: string; kind?: string; packetsSent?: number }) => {
        if (stat.type === "outbound-rtp" && stat.kind === "audio") sent += stat.packetsSent ?? 0;
      });
    }
    return sent;
  });

async function connectedPair(muteOutput: boolean) {
  const dual = await launchDualContexts({
    babyFakeWav: silence,
    parentFakeWav: pulses,
    permissions: CLIPBOARD_PERMISSIONS,
  });
  await dual.baby.addInitScript(loopbackRoom);
  await dual.parent.addInitScript(trackPeerConnections);
  await pairWeb(dual);
  if (muteOutput) {
    await dual.babyPage.evaluate(() => {
      (window as unknown as { __mute: boolean }).__mute = true;
    });
  }
  // The core measures the room for 2 s before a chime; the parent needs the baby's status too.
  await expect(dual.parentPage.getByTestId("parent_talk")).toBeEnabled({ timeout: 30_000 });
  await dual.babyPage.waitForTimeout(3_000);
  return dual;
}

const destinationCount = (page: Page): Promise<number> =>
  page.evaluate(() => (window as unknown as { __destCount: () => number }).__destCount());

/** Holds the talk button for `holdMs` once the microphone is attached; returns after the release. */
async function holdToTalk(parent: Page, holdMs: number, whileTalking?: () => Promise<void>) {
  const button = parent.getByTestId("parent_talk");
  await button.hover();
  await parent.mouse.down();
  // The permission step comes first when the browser does not report the microphone as granted.
  const sheet = parent.getByTestId("parent_talk_permission_sheet");
  if (await sheet.isVisible()) {
    await parent.mouse.up();
    await parent.getByTestId("parent_talk_permission_allow").click();
    await parent.getByTestId("parent_talk_permission_ok").click();
    await button.hover();
    await parent.mouse.down();
  }
  await expect(button).toHaveAttribute("data-phase", "talking", { timeout: 20_000 });
  await whileTalking?.();
  await parent.waitForTimeout(holdMs);
  await parent.mouse.up();
  await expect(button).not.toHaveAttribute("data-phase", "talking");
}

test("the web parent talks to the web baby and hears that it was audible", async () => {
  test.setTimeout(180_000);
  const dual = await connectedPair(false);
  const { babyPage: baby, parentPage: parent } = dual;
  try {
    // Nothing reaches the loudspeaker outside a lease.
    expect(await destinationCount(baby)).toBe(0);
    // The talk sender sends nothing before a talk (no track on the line).
    await parent.waitForTimeout(2_000);
    expect(await packetsSent(parent)).toBe(0);

    await holdToTalk(parent, 5_000, async () => {
      await expect(baby.getByTestId("baby_talking")).toBeVisible({ timeout: 10_000 });
      expect(await destinationCount(baby)).toBeGreaterThan(0);
      // The microphone is open: nothing is announced.
      await expect(parent.getByTestId("parent_talk_state")).toHaveAttribute("aria-live", "off");
    });

    await expect(baby.getByTestId("baby_talking")).toBeHidden({ timeout: 10_000 });
    await expect(baby.getByTestId("baby_last_talk")).toBeVisible();
    // The output is cut again after the talk.
    await expect.poll(() => destinationCount(baby), { timeout: 10_000 }).toBe(0);
    // ...and nothing again after it: the counter stops with the release.
    await parent.waitForTimeout(1_500);
    const afterTalk = await packetsSent(parent);
    expect(afterTalk).toBeGreaterThan(0);
    await parent.waitForTimeout(3_000);
    expect(await packetsSent(parent)).toBe(afterTalk);
    // The baby heard itself: the parent is never told the voice may be inaudible.
    await parent.waitForTimeout(6_000);
    await expect(parent.getByTestId("parent_talk_unconfirmed")).toHaveCount(0);
  } finally {
    await dual.dispose();
  }
});

test("with the baby's output muted the parent is told the voice may not be audible", async () => {
  test.setTimeout(180_000);
  const dual = await connectedPair(true);
  const { parentPage: parent } = dual;
  try {
    await holdToTalk(parent, 5_000);
    await expect(parent.getByTestId("parent_talk_unconfirmed")).toBeVisible({ timeout: 20_000 });
  } finally {
    await dual.dispose();
  }
});
