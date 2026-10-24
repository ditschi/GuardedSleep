import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { get } from "svelte/store";
import { loadCoreNode } from "@guardedsleep/core-wasm/node";
import type { LinkConfig, LinkEvent } from "@guardedsleep/core-wasm";
import { LinkDriver } from "./link-driver.js";
import { ParentMonitor } from "./parent-monitor.js";
import { createHarness, FakeBabyAudio, FakePeer } from "./testing/fake-env.js";

const core = loadCoreNode();
const PSK = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8";
const OTHER_PSK = "HxgZGhscHR4fICEiItQlJicoKSorLC0uLzAxMjM0NTY";
const URL_A = "ws://signal.test:1";
const URL_B = "ws://signal.test:2";

const cfg = (role: LinkConfig["role"], psk = PSK): LinkConfig => ({
  role,
  psk_b64url: psk,
  seed: "07".repeat(32),
  heartbeat_interval_ms: 10_000,
  alarm_after_ms: 60_000,
  noise_threshold: 0.12,
  noise_hold_ms: 800,
});

function setup(opts?: { parentPsk?: string; urls?: string[] }) {
  const h = createHarness();
  const babyEvents: LinkEvent[] = [];
  const babyEnv = h.env();
  const parentEnv = h.env();
  const babyAudio = new FakeBabyAudio();
  const baby = new LinkDriver({
    core,
    config: cfg("baby"),
    signalingUrls: opts?.urls ?? [URL_A],
    env: babyEnv.env,
    babyAudio,
    onEvent: (e) => babyEvents.push(e),
  });
  const parent = new ParentMonitor({
    core,
    config: cfg("parent", opts?.parentPsk ?? PSK),
    signalingUrls: opts?.urls ?? [URL_A],
    env: parentEnv.env,
    wallNow: () => h.clock.now,
  });
  return { h, baby, parent, babyEvents, babyAudio };
}

const babyPhase = (events: LinkEvent[]): string | undefined =>
  [...events].reverse().find((e) => e.type === "status")?.status.state;

async function connect(h: ReturnType<typeof setup>): Promise<void> {
  h.baby.start();
  h.parent.start();
  await h.h.advance(15_000);
}

describe("LinkDriver with the real core", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("pairs a baby and a parent and reaches connected on both sides", async () => {
    const s = setup();
    await connect(s);
    expect(babyPhase(s.babyEvents)).toBe("connected");
    const st = get(s.parent.state);
    expect(st.link.phase).toBe("connected");
    expect(st.link.level).toBe(0);
    expect(st.quietSinceWallMs).not.toBeNull();
    expect(s.baby.roomId).toBe(s.parent.roomId);
    s.baby.stop();
    s.parent.stop();
  });

  it("starts the baby with the mic disabled and enables it only when the core asks", async () => {
    const s = setup();
    await connect(s);
    expect(s.babyAudio.enabledLog).not.toContain(true);
    s.parent.setListen("hold");
    await s.h.advance(1_000);
    expect(s.babyAudio.enabledLog.at(-1)).toBe(true);
    s.parent.setListen("off");
    await s.h.advance(1_000);
    expect(s.babyAudio.enabledLog.at(-1)).toBe(false);
    s.baby.stop();
    s.parent.stop();
  });

  it("recovers by itself after the DataChannel dropped", async () => {
    const s = setup();
    await connect(s);
    const first = s.h.net.peers.filter((p) => !p.closed);
    expect(first).toHaveLength(2);
    const phases: string[] = [];
    const off = s.parent.state.subscribe((st) => phases.push(st.link.phase));
    s.h.net.dropChannel(first[0] as FakePeer);
    await s.h.advance(20_000);
    off();
    expect(phases).toContain("reconnecting");
    expect(get(s.parent.state).link.phase).toBe("connected");
    expect(babyPhase(s.babyEvents)).toBe("connected");
    const live = s.h.net.peers.filter((p) => !p.closed);
    expect(live).toHaveLength(2);
    expect(live.every((p) => !first.includes(p))).toBe(true);
    s.baby.stop();
    s.parent.stop();
  });

  it("recovers after the signaling server dropped every socket", async () => {
    const s = setup();
    await connect(s);
    s.h.server.dropAll();
    await s.h.advance(20_000);
    expect(get(s.parent.state).link.phase).toBe("connected");
    s.baby.stop();
    s.parent.stop();
  });

  it("a peer that never shows up reaches level 2 with the peer_absent hint", async () => {
    const s = setup();
    s.parent.start();
    await s.h.advance(25_000);
    expect(get(s.parent.state).link.level).toBe(1);
    await s.h.advance(45_000);
    const st = get(s.parent.state);
    expect(st.link.level).toBe(2);
    expect(st.link.causeHint).toBe("peer_absent");
    s.parent.stop();
  });

  it("a long outage reaches level 2 with server_unreachable, then reports the restore", async () => {
    const s = setup();
    await connect(s);
    s.h.server.reachable = false;
    s.h.net.up = false;
    s.h.server.dropAll();
    s.h.net.dropChannel(s.h.net.peers.filter((p) => !p.closed)[0] as FakePeer);
    await s.h.advance(25_000);
    expect(get(s.parent.state).link.level).toBe(1);
    await s.h.advance(45_000);
    expect(get(s.parent.state).link.level).toBe(2);
    expect(get(s.parent.state).link.causeHint).toBe("server_unreachable");
    const restored: (number | null)[] = [];
    const off = s.parent.state.subscribe((st) => restored.push(st.restoredAfterMs));
    s.h.server.reachable = true;
    s.h.net.up = true;
    await s.h.advance(30_000);
    off();
    expect(get(s.parent.state).link.phase).toBe("connected");
    expect(restored.some((r) => r != null && r >= 60_000)).toBe(true);
    expect(get(s.parent.state).restoredAfterMs).toBeNull();
    s.baby.stop();
    s.parent.stop();
  });

  it("a wrong PSK is a different room, so the phones never pair", async () => {
    const s = setup({ parentPsk: OTHER_PSK });
    expect(s.baby.roomId).toBe("");
    s.baby.start();
    s.parent.start();
    expect(s.baby.roomId).not.toBe(s.parent.roomId);
    await s.h.advance(75_000);
    const st = get(s.parent.state);
    expect(st.link.phase).not.toBe("connected");
    expect(st.link.causeHint).toBe("peer_absent");
    expect(s.h.net.peers).toHaveLength(0);
    s.baby.stop();
    s.parent.stop();
  });

  it("shows peer_stopped on the other side after a deliberate stop", async () => {
    const s = setup();
    await connect(s);
    s.parent.stop();
    await s.h.advance(2_000);
    expect(babyPhase(s.babyEvents)).toBe("peer_stopped");
    expect(s.babyEvents.some((e) => e.type === "peer_stopped")).toBe(true);
    s.baby.stop();
  });

  it("tries the next signaling URL after one failed before opening", async () => {
    const s = setup({ urls: [URL_A, URL_B] });
    const seen: string[] = [];
    const server = s.h.server;
    const original = server.connect.bind(server);
    server.connect = (url: string) => {
      seen.push(url);
      return original(url);
    };
    server.reachable = false;
    s.parent.start();
    await s.h.advance(3_000);
    server.reachable = true;
    await s.h.advance(20_000);
    expect(new Set(seen)).toEqual(new Set([URL_A, URL_B]));
    s.parent.stop();
  });

  it("never lets the clock run backwards", async () => {
    const s = setup();
    s.parent.start();
    await s.h.advance(1_000);
    s.h.clock.now -= 600_000;
    const before = s.parent.link.now();
    await s.h.advance(1_000);
    expect(s.parent.link.now()).toBeGreaterThanOrEqual(before);
    s.parent.stop();
  });

  it("answers a remote silence request honestly, without claiming control", async () => {
    const s = setup();
    await connect(s);
    s.parent.requestSilenceBaby();
    await s.h.advance(1_000);
    expect(get(s.parent.state).silenceRequest).toBe("pending");
    await s.h.advance(11_000);
    expect(get(s.parent.state).silenceRequest).toBe("failed");
    s.baby.stop();
    s.parent.stop();
  });

  it("reports a suspend/resume gap between ticks", async () => {
    const s = setup();
    s.parent.start();
    await s.h.advance(1_000);
    const gaps: number[] = [];
    const off = s.parent.suspendGap.subscribe((g) => g && gaps.push(g.gapMs));
    s.h.clock.now += 120_000;
    await s.h.advance(500);
    off();
    expect(gaps.some((g) => g >= 120_000)).toBe(true);
    s.parent.stop();
  });

  it("verifies the peer only after the DTLS fingerprint was reported", async () => {
    const s = setup();
    const reads: string[] = [];
    const orig = FakePeer.prototype.remoteDtlsFingerprint;
    FakePeer.prototype.remoteDtlsFingerprint = async function (this: FakePeer) {
      const fp = await orig.call(this);
      if (fp) reads.push(fp);
      return fp;
    };
    try {
      await connect(s);
      expect(reads.length).toBeGreaterThanOrEqual(2);
      expect(get(s.parent.state).link.phase).toBe("connected");
    } finally {
      FakePeer.prototype.remoteDtlsFingerprint = orig;
      s.baby.stop();
      s.parent.stop();
    }
  });

  it("flags a browser that never exposes the DTLS certificate as unable to verify", async () => {
    const s = setup();
    const orig = FakePeer.prototype.remoteDtlsFingerprint;
    FakePeer.prototype.remoteDtlsFingerprint = async () => null;
    try {
      await connect(s);
      await s.h.advance(10_000);
      const link = get(s.parent.state).link;
      expect(link.phase).toBe("failed_needs_user");
      expect(link.failureKey).toBe("browser_unsupported");
    } finally {
      FakePeer.prototype.remoteDtlsFingerprint = orig;
      s.baby.stop();
      s.parent.stop();
    }
  });

  it("shows the baby's unit warnings and noise threshold on the parent", async () => {
    const s = setup();
    await connect(s);
    s.baby.deviceStatus({ battery_pct: 80, charging: false, unit_warnings: ["page_hidden"] });
    await s.h.advance(1_000);
    const peer = get(s.parent.state).peer;
    expect(peer.unitWarnings).toEqual(["page_hidden"]);
    expect(peer.noiseThreshold).toBeCloseTo(0.12, 3);
    s.baby.stop();
    s.parent.stop();
  });

  it("keeps the last contact time of an outage from the core", async () => {
    const s = setup();
    await connect(s);
    const connectedAt = s.parent.link.now();
    s.h.server.dropAll();
    s.h.net.up = false;
    s.h.net.dropChannel(s.h.net.peers.filter((p) => !p.closed)[0] as FakePeer);
    await s.h.advance(8_000);
    const link = get(s.parent.state).link;
    expect(link.phase).not.toBe("connected");
    expect(link.lastAliveElapsedMs).not.toBeNull();
    expect(link.lastAliveElapsedMs!).toBeLessThanOrEqual(s.parent.link.now());
    expect(link.lastAliveElapsedMs!).toBeGreaterThan(connectedAt - 30_000);
    s.baby.stop();
    s.parent.stop();
  });
});
