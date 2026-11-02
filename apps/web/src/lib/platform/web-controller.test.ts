import type {
  CoreApi,
  CoreLink,
  InviteMailbox,
  LinkAction,
  LinkConfig,
  LinkInput,
  LinkStatus,
} from "@guardedsleep/core-wasm";
import { loadCoreNode } from "@guardedsleep/core-wasm/node";
import { get } from "svelte/store";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AlertSink, LinkSnapshot, NoiseKind, Notice } from "./alert-sink";
import { createSoundGate } from "./audio";
import { createWebSocketSignaling } from "./signaling";
import { FakeMedia, FakeTime, FakeWebSocket } from "./test-support";
import { asContext, FakeAudioContext } from "./test-audio";
import { WebController, type NetworkWatcher } from "./web-controller";

const realCore = loadCoreNode();
const RELAY = "wss://relay.test/sig";
const RELAY_MAILBOX: InviteMailbox = { kind: "relay", url: RELAY };

/** The real invite codec; the link itself is scripted so a test decides what the core "says". */
class ScriptedCore {
  readonly configs: LinkConfig[] = [];
  readonly inputs: LinkInput[] = [];
  /** The wall clock reports, kept apart so the input assertions stay about behaviour. */
  readonly wallClocks: number[] = [];
  freed = 0;
  script: (input: LinkInput) => LinkAction[] = () => [];
  createLink(config: LinkConfig): CoreLink {
    this.configs.push(config);
    return {
      handle: (_at, input) => {
        if (input.type === "wall_clock") {
          this.wallClocks.push(input.unix_s);
          return [];
        }
        this.inputs.push(input);
        return this.script(input);
      },
      roomId: () => "room",
      free: () => {
        this.freed++;
      },
    };
  }
  buildInvite: CoreApi["buildInvite"] = (psk, mailboxes, exp) =>
    realCore.buildInvite(psk, mailboxes, exp);
  parseInvite: CoreApi["parseInvite"] = (text, now) => realCore.parseInvite(text, now);
  types(): string[] {
    return this.inputs.map((i) => i.type);
  }
}

const emitStatus = (status: Partial<LinkStatus> & Pick<LinkStatus, "state">): LinkAction => ({
  type: "emit",
  event: {
    type: "status",
    status: {
      level: 0,
      key: null,
      cause: null,
      acknowledged_until: null,
      last_alive_ms: null,
      ...status,
    },
  },
});

class RecordingSink implements AlertSink {
  log: string[] = [];
  noise: Array<[NoiseKind, boolean, boolean]> = [];
  linkState(_s: LinkSnapshot): void {
    this.log.push("linkState");
  }
  linkLost(): void {}
  noiseAlert(kind: NoiseKind, probe: boolean, snoozed: boolean): void {
    this.noise.push([kind, probe, snoozed]);
  }
  noiseEnded(): void {}
  stopNoiseSound(): void {}
  notice(_n: Notice): void {}
  clear(): void {
    this.log.push("clear");
  }
}

const flush = async (): Promise<void> => {
  for (let i = 0; i < 10; i++) await Promise.resolve();
};

function setup(
  options: {
    micDenied?: boolean;
    core?: ScriptedCore;
    defaultUrls?: string[];
    nostrRelays?: string[];
  } = {},
) {
  FakeWebSocket.instances = [];
  const time = new FakeTime();
  const core = options.core ?? new ScriptedCore();
  const medias: FakeMedia[] = [];
  const media = { requests: [] as Array<{ role: string; ice: RTCIceServer[] }> };
  const ctx = new FakeAudioContext();
  const gate = createSoundGate(() => asContext(ctx));
  const alerts = new RecordingSink();
  const network = { started: 0, stopped: 0, fire: (): void => undefined };
  const watcher: NetworkWatcher = {
    start(cb) {
      network.started++;
      network.fire = cb;
    },
    stop() {
      network.stopped++;
    },
  };
  let counter = 0;
  const controller = new WebController({
    core,
    clock: time,
    timers: time,
    signaling: createWebSocketSignaling(FakeWebSocket),
    createMedia: async (role, ice) => {
      media.requests.push({ role, ice });
      if (options.micDenied && role === "baby") {
        throw new DOMException("denied", "NotAllowedError");
      }
      const m = new FakeMedia();
      medias.push(m);
      return m;
    },
    gate,
    alerts,
    network: watcher,
    randomBytes: (n) => Uint8Array.from({ length: n }, () => ++counter % 251),
    entropy: () => 99,
    defaultSignalingUrls: () => options.defaultUrls ?? [RELAY],
    deviceName: () => "Chrome on Android",
    ...(options.nostrRelays ? { nostrRelays: () => options.nostrRelays ?? [] } : {}),
  });
  return { controller, time, core, medias, media, ctx, gate, alerts, network };
}

type Setup = ReturnType<typeof setup>;

async function pairedParent(t: Setup): Promise<void> {
  t.controller.chooseRole("parent");
  const psk = Uint8Array.from({ length: 32 }, (_, i) => i + 1);
  const invite = realCore.buildInvite(psk, [RELAY_MAILBOX], t.time.wallMs() / 1000 + 300);
  if (!invite.ok) throw new Error("invite");
  t.core.script = (input) => (input.type === "start" ? [emitStatus({ state: "connecting" })] : []);
  t.controller.submitPairingCode(invite.text);
  await flush();
  t.core.script = () => [emitStatus({ state: "connected" })];
  t.time.advance(250);
  expect(get(t.controller.pairing)?.phase).toBe("paired");
}

describe("WebController: pairing", () => {
  it("baby: shows a fresh invite for 5 minutes and waits in the room", async () => {
    const t = setup();
    t.controller.chooseRole("baby");
    t.controller.startPairing();
    const state = get(t.controller.pairing);
    expect(state?.phase).toBe("showing_code");
    expect(state?.expiresAtElapsedMs).toBe(300_000);
    const parsed = realCore.parseInvite(state?.codePayload ?? "", t.time.wallMs() / 1000);
    expect(parsed.ok && parsed.invite.mailboxes).toEqual([RELAY_MAILBOX]);
    expect(parsed.ok && parsed.invite.expiresAtEpochS).toBe(
      Math.floor(t.time.wallMs() / 1000) + 300,
    );
    await flush();
    // One microphone request, the relay's STUN and nothing else, one core link, one socket.
    expect(t.media.requests).toEqual([{ role: "baby", ice: [{ urls: ["stun:relay.test:3478"] }] }]);
    expect(t.core.configs).toHaveLength(1);
    expect(t.core.configs[0]).toMatchObject({ role: "baby", paired_before: false });
    expect(FakeWebSocket.instances).toHaveLength(0); // the scripted core did not ask yet
    expect(t.core.types()).toEqual(["start"]);
  });

  it("baby: the invite is single use: a new one gets a new key", async () => {
    const t = setup();
    t.controller.chooseRole("baby");
    t.controller.startPairing();
    await flush();
    const first = t.core.configs[0]?.psk_b64url;
    t.controller.refreshPairingCode();
    await flush();
    expect(t.core.configs[1]?.psk_b64url).not.toBe(first);
    // Every run gets its own 32-byte seed, as 64 hex characters.
    expect(t.core.configs[0]?.seed).toMatch(/^[0-9a-f]{64}$/);
    expect(t.core.configs[1]?.seed).not.toBe(t.core.configs[0]?.seed);
    t.time.advance(500);
    expect(t.core.freed).toBe(1); // the first run said stop, flushed and was torn down
  });

  it("baby: the code expires after 5 minutes and the room closes", async () => {
    const t = setup();
    t.controller.chooseRole("baby");
    t.controller.startPairing();
    await flush();
    t.time.advance(299_000);
    expect(get(t.controller.pairing)?.phase).toBe("showing_code");
    t.time.advance(2_000);
    expect(get(t.controller.pairing)?.phase).toBe("expired");
    expect(t.core.types()).toContain("stop");
  });

  it("only a baby starts pairing, only a parent submits a code", () => {
    const t = setup();
    t.controller.chooseRole("parent");
    t.controller.startPairing();
    expect(get(t.controller.pairing)).toBeNull();
    t.controller.chooseRole("baby");
    t.controller.submitPairingCode("{}");
    expect(get(t.controller.pairing)).toBeNull();
  });

  it("baby without any relay cannot show a code", () => {
    const t = setup({ defaultUrls: [] });
    t.controller.chooseRole("baby");
    t.controller.startPairing();
    expect(get(t.controller.pairing)).toMatchObject({ phase: "failed", error: "unreachable" });
  });

  it("baby: a refused microphone fails the run with a web-only key", async () => {
    const t = setup({ micDenied: true });
    t.controller.chooseRole("baby");
    t.controller.startPairing();
    await flush();
    expect(get(t.controller.baby).link).toMatchObject({
      phase: "failed_needs_user",
      failureKey: "mic_denied",
    });
    expect(get(t.controller.pairing)?.phase).toBe("failed");
    expect(t.core.configs).toHaveLength(0);
  });

  const rejected: Array<[string, string]> = [
    ["not json at all", "not_our_code"],
    ['{"v":3,"k":"AAAA', "incomplete_paste"],
  ];
  it.each(rejected)("parent: %s is rejected as %s", (text, error) => {
    const t = setup();
    t.controller.chooseRole("parent");
    t.controller.submitPairingCode(text);
    expect(get(t.controller.pairing)).toMatchObject({ phase: "failed", error });
    expect(t.core.configs).toHaveLength(0);
  });

  it("parent: an expired invite is rejected", () => {
    const t = setup();
    t.controller.chooseRole("parent");
    const psk = new Uint8Array(32).fill(7);
    const invite = realCore.buildInvite(psk, [RELAY_MAILBOX], t.time.wallMs() / 1000 - 10);
    if (!invite.ok) throw new Error("invite");
    t.controller.submitPairingCode(invite.text);
    expect(get(t.controller.pairing)).toMatchObject({ phase: "failed", error: "expired" });
  });

  it("parent: connects with the invite's relay and shows 'unreachable' after 20 s", async () => {
    const t = setup();
    t.controller.chooseRole("parent");
    const invite = realCore.buildInvite(
      new Uint8Array(32).fill(9),
      [{ kind: "relay", url: "wss://other.example" }],
      1e10,
    );
    if (!invite.ok) throw new Error("invite");
    t.controller.submitPairingCode(invite.text);
    expect(get(t.controller.pairing)?.phase).toBe("connecting");
    await flush();
    expect(t.media.requests[0]).toEqual({
      role: "parent",
      ice: [{ urls: ["stun:other.example:3478"] }],
    });
    t.time.advance(19_000);
    expect(get(t.controller.pairing)?.error).toBeNull();
    t.time.advance(2_000);
    expect(get(t.controller.pairing)).toMatchObject({ phase: "connecting", error: "unreachable" });
  });

  it("a verified link completes pairing and keeps running as the monitoring run", async () => {
    const t = setup();
    await pairedParent(t);
    expect(get(t.controller.app)).toMatchObject({ paired: true, monitoring: true });
    expect(t.core.types()).not.toContain("stop");
    expect(t.core.configs).toHaveLength(1);
  });

  it("a key mismatch fails pairing and ends the run", async () => {
    const t = setup();
    t.controller.chooseRole("parent");
    const invite = realCore.buildInvite(new Uint8Array(32).fill(3), [RELAY_MAILBOX], 1e10);
    if (!invite.ok) throw new Error("invite");
    t.core.script = () => [];
    t.controller.submitPairingCode(invite.text);
    await flush();
    t.core.script = () => [emitStatus({ state: "failed_needs_user", key: "pairing_key_mismatch" })];
    t.time.advance(250);
    expect(get(t.controller.pairing)).toMatchObject({ phase: "failed", error: "key_mismatch" });
    expect(get(t.controller.app).paired).toBe(false);
    expect(t.core.types()).toContain("stop");
  });

  it("another terminal failure while pairing reads as unreachable", async () => {
    const t = setup();
    t.controller.chooseRole("parent");
    const invite = realCore.buildInvite(new Uint8Array(32).fill(3), [RELAY_MAILBOX], 1e10);
    if (!invite.ok) throw new Error("invite");
    t.controller.submitPairingCode(invite.text);
    await flush();
    t.core.script = () => [emitStatus({ state: "failed_needs_user", key: "update_required" })];
    t.time.advance(250);
    expect(get(t.controller.pairing)).toMatchObject({ phase: "failed", error: "unreachable" });
  });

  it("cancel closes the room and clears the pairing state", async () => {
    const t = setup();
    t.controller.chooseRole("baby");
    t.controller.startPairing();
    await flush();
    t.controller.cancelPairing();
    expect(get(t.controller.pairing)).toBeNull();
    expect(t.core.types()).toContain("stop");
  });

  it("cancel while the browser still asks for the microphone releases the late engine", async () => {
    const t = setup();
    t.controller.chooseRole("baby");
    t.controller.startPairing();
    t.controller.cancelPairing();
    await flush();
    expect(t.core.configs).toHaveLength(0);
    expect(t.medias[0]?.released).toBe(true);
  });
});

describe("WebController: monitoring", () => {
  it("restarts from the in-memory pairing with paired_before set", async () => {
    const t = setup();
    await pairedParent(t);
    t.controller.stopMonitoring();
    expect(get(t.controller.app).monitoring).toBe(false);
    t.time.advance(600);
    await flush();
    t.controller.startMonitoring();
    await flush();
    expect(get(t.controller.app).monitoring).toBe(true);
    expect(t.core.configs).toHaveLength(2);
    expect(t.core.configs[1]).toMatchObject({ role: "parent", paired_before: true });
    expect(t.core.configs[1]?.psk_b64url).toBe(t.core.configs[0]?.psk_b64url);
  });

  it("does nothing without a pairing, and forgetting one removes it", async () => {
    const t = setup();
    t.controller.chooseRole("parent");
    t.controller.startMonitoring();
    await flush();
    expect(t.core.configs).toHaveLength(0);
    await pairedParent(t);
    t.controller.forgetPairing();
    expect(get(t.controller.app)).toMatchObject({ paired: false, monitoring: false });
    t.controller.startMonitoring();
    await flush();
    expect(t.core.configs).toHaveLength(1);
  });

  it("switching the role forgets the pairing", async () => {
    const t = setup();
    await pairedParent(t);
    t.controller.chooseRole("baby");
    expect(get(t.controller.app)).toMatchObject({ role: "baby", paired: false });
  });

  it("stop announces the stop to the peer and silences the alerts", async () => {
    const t = setup();
    await pairedParent(t);
    t.controller.stopMonitoring();
    expect(t.core.types()).toContain("stop");
    expect(t.alerts.log).toContain("clear");
    expect(t.network.stopped).toBeGreaterThan(0);
  });

  it("the network coming back is an input for the core, not a decision here", async () => {
    const t = setup();
    await pairedParent(t);
    t.network.fire();
    expect(t.core.types()).toContain("network_available");
    t.controller.tryReconnectNow();
    expect(t.core.types().filter((x) => x === "network_available")).toHaveLength(2);
  });

  it("the link config follows the settings within the core's limits", async () => {
    const t = setup();
    t.controller.updateSettings((s) => ({
      ...s,
      heartbeatSeconds: 15,
      threshold: 0.3,
      holdSeconds: 7,
    }));
    await pairedParent(t);
    expect(t.core.configs[0]).toMatchObject({
      heartbeat_interval_ms: 15_000,
      noise_threshold: 0.3,
      noise_hold_ms: 7_000,
    });
    const hb = t.core.configs[0]?.heartbeat_interval_ms ?? 0;
    expect(t.core.configs[0]?.alarm_after_ms).toBeGreaterThanOrEqual(2 * hb + 5_000 + 20_000);
    // Out-of-range heartbeats are clamped, never passed through.
    const low = setup();
    low.controller.updateSettings((s) => ({ ...s, heartbeatSeconds: 1 }));
    await pairedParent(low);
    expect(low.core.configs[0]?.heartbeat_interval_ms).toBe(5_000);
  });

  it("an explicit relay list in advanced settings overrides the invite's", async () => {
    const t = setup();
    t.controller.updateSettings((s) => ({
      ...s,
      advancedEnabled: true,
      signalingUrls: ["wss://mine.example"],
    }));
    t.controller.chooseRole("parent");
    const invite = realCore.buildInvite(new Uint8Array(32).fill(5), [RELAY_MAILBOX], 1e10);
    if (!invite.ok) throw new Error("invite");
    t.controller.submitPairingCode(invite.text);
    await flush();
    expect(t.media.requests[0]?.ice).toEqual([{ urls: ["stun:mine.example:3478"] }]);
  });
});

const SECRET = "ab".repeat(32);

const emit = (event: Extract<LinkAction, { type: "emit" }>["event"]): LinkAction => ({
  type: "emit",
  event,
});

describe("WebController: the parent pin", () => {
  it("parent: the config carries the device name, the baby's carries its mailbox set", async () => {
    const parent = setup();
    await pairedParent(parent);
    expect(parent.core.configs[0]).toMatchObject({
      device_name: "Chrome on Android",
      allow_dev_loopback: false,
    });
    expect(parent.core.configs[0]?.mailboxes).toBeUndefined();
    expect(parent.core.configs[0]?.pinned_parent).toBeUndefined();

    const baby = setup();
    baby.controller.chooseRole("baby");
    baby.controller.startPairing();
    await flush();
    expect(baby.core.configs[0]?.mailboxes).toEqual([RELAY_MAILBOX]);
    expect(baby.core.configs[0]?.device_name).toBeUndefined();
  });

  it("the pinned secret is kept in memory and handed back to the next run", async () => {
    const t = setup();
    await pairedParent(t);
    // The event can come after the `connected` status ...
    t.core.script = () => [emit({ type: "parent_pinned", secret: SECRET })];
    t.time.advance(250);
    t.controller.stopMonitoring();
    t.time.advance(600);
    await flush();
    t.controller.startMonitoring();
    await flush();
    expect(t.core.configs[1]?.pinned_parent).toBe(SECRET);
    expect(t.core.configs[1]).toMatchObject({ paired_before: true });
  });

  it("... or before it, while the pairing is still pending", async () => {
    const t = setup();
    t.controller.chooseRole("parent");
    const psk = Uint8Array.from({ length: 32 }, (_, i) => i + 1);
    const invite = realCore.buildInvite(psk, [RELAY_MAILBOX], t.time.wallMs() / 1000 + 300);
    if (!invite.ok) throw new Error("invite");
    t.core.script = (input) =>
      input.type === "tick"
        ? [emit({ type: "parent_pinned", secret: SECRET }), emitStatus({ state: "connected" })]
        : [];
    t.controller.submitPairingCode(invite.text);
    await flush();
    t.time.advance(250);
    expect(get(t.controller.pairing)?.phase).toBe("paired");
    t.core.script = () => [];
    t.controller.stopMonitoring();
    t.time.advance(600);
    await flush();
    t.controller.startMonitoring();
    await flush();
    expect(t.core.configs[1]?.pinned_parent).toBe(SECRET);
  });

  it("the first-pairing window is the invite expiry, then what the core reports", async () => {
    const t = setup();
    await pairedParent(t);
    // The window of the parsed invite goes to the core of the pairing run ...
    expect(t.core.configs[0]?.pin_window_until_s).toBe(Math.floor(t.time.wallMs() / 1000) + 300);
    // ... and the last `window_until_s` of the core to the next run (null: acknowledged).
    t.core.script = () => [emit({ type: "parent_pinned", secret: SECRET, window_until_s: 1_234 })];
    t.time.advance(250);
    t.controller.stopMonitoring();
    t.time.advance(600);
    await flush();
    t.controller.startMonitoring();
    await flush();
    expect(t.core.configs[1]?.pin_window_until_s).toBe(1_234);
    t.core.script = () => [emit({ type: "parent_pinned", secret: SECRET })];
    t.time.advance(250);
    t.controller.stopMonitoring();
    t.time.advance(600);
    await flush();
    t.controller.startMonitoring();
    await flush();
    expect(t.core.configs[2]?.pin_window_until_s).toBeUndefined();
  });

  it("the baby counts the pairing as complete at the pin, not at the connection", async () => {
    const t = setup();
    t.controller.chooseRole("baby");
    t.controller.startPairing();
    await flush();
    expect(t.core.configs[0]?.pin_window_until_s).toBe(Math.floor(t.time.wallMs() / 1000) + 300);
    t.core.script = (input) => (input.type === "tick" ? [emitStatus({ state: "connected" })] : []);
    t.time.advance(250);
    expect(get(t.controller.pairing)?.phase).not.toBe("paired");
    t.core.script = (input) =>
      input.type === "tick" ? [emit({ type: "parent_pinned", secret: SECRET })] : [];
    t.time.advance(250);
    expect(get(t.controller.pairing)?.phase).toBe("paired");
  });

  it("a new pairing starts without the old secret, and forgetting drops it", async () => {
    const t = setup();
    await pairedParent(t);
    t.core.script = () => [emit({ type: "parent_pinned", secret: SECRET })];
    t.time.advance(250);
    t.controller.forgetPairing();
    t.time.advance(600);
    await flush();
    t.controller.chooseRole("parent");
    await pairedParent(t);
    expect(t.core.configs[1]?.pinned_parent).toBeUndefined();
  });

  it("the secret never reaches a store, a socket URL or the storage", async () => {
    const setItem = vi.spyOn(Storage.prototype, "setItem");
    const t = setup();
    await pairedParent(t);
    t.core.script = () => [emit({ type: "parent_pinned", secret: SECRET })];
    t.time.advance(250);
    const everything = [
      JSON.stringify(get(t.controller.app)),
      JSON.stringify(get(t.controller.pairing)),
      JSON.stringify(get(t.controller.settings)),
      JSON.stringify(get(t.controller.parent)),
      JSON.stringify(get(t.controller.baby)),
      ...FakeWebSocket.instances.map((w) => w.url),
      ...Object.values(window.localStorage),
    ].join("\n");
    expect(everything).not.toContain(SECRET);
    expect(setItem).not.toHaveBeenCalled();
    setItem.mockRestore();
  });

  it("a mailbox update from the baby replaces the relay list of the next run", async () => {
    const t = setup();
    await pairedParent(t);
    t.core.script = () => [
      emit({
        type: "mailboxes_updated",
        mailboxes: [{ kind: "relay", url: "wss://new-relay.test/sig" }],
      }),
    ];
    t.time.advance(250);
    t.controller.stopMonitoring();
    t.time.advance(600);
    await flush();
    FakeWebSocket.instances = [];
    // The core asks for its relay mailbox when it starts; the platform opens the stored address.
    t.core.script = (input) =>
      input.type === "start" ? [{ type: "open_mailbox", id: 0, kind: "relay", url: "" }] : [];
    t.controller.startMonitoring();
    await flush();
    expect(FakeWebSocket.instances.map((w) => w.url)).toEqual(["wss://new-relay.test/sig"]);
  });

  it("baby: the parent's name shows while connected; warnings and hints reach the store", async () => {
    const t = setup();
    t.controller.chooseRole("baby");
    t.controller.startPairing();
    await flush();
    t.core.script = () => [
      emit({ type: "parent_connected", name: "Pixel 8" }),
      emit({ type: "hint", key: "clock_skew", active: true }),
      emit({ type: "security_warning", key: "unknown_parent_attempt" }),
      emitStatus({ state: "connected" }),
    ];
    t.time.advance(250);
    expect(get(t.controller.baby)).toMatchObject({
      parentName: "Pixel 8",
      securityWarning: true,
    });
    expect(get(t.controller.baby).link.hints).toEqual(["clock_skew"]);
    t.core.script = () => [
      emit({ type: "hint", key: "clock_skew", active: false }),
      emitStatus({ state: "reconnecting" }),
    ];
    t.time.advance(250);
    expect(get(t.controller.baby).parentName).toBeNull();
    expect(get(t.controller.baby).link.hints).toEqual([]);
  });

  it("parent: calm hints reach the link state, unknown keys are ignored", async () => {
    const t = setup();
    await pairedParent(t);
    t.core.script = () => [
      emit({ type: "hint", key: "recovery_path_unavailable", active: true }),
      emit({ type: "hint", key: "something_new", active: true }),
    ];
    t.time.advance(250);
    expect(get(t.controller.parent).link.hints).toEqual(["recovery_path_unavailable"]);
    expect(get(t.controller.parent).securityWarning).toBe(false);
  });
});

describe("WebController: contract state from the core", () => {
  it("parent: status and events reach the parent store", async () => {
    const t = setup();
    await pairedParent(t);
    expect(get(t.controller.parent).link.phase).toBe("connected");
    t.core.script = () => [
      emitStatus({ state: "reconnecting", level: 2, cause: "server_unreachable" }),
      { type: "emit", event: { type: "link_lost", down_ms: 61_000 } },
    ];
    t.time.advance(250);
    expect(get(t.controller.parent).link).toMatchObject({
      phase: "reconnecting",
      level: 2,
      causeHint: "server_unreachable",
    });
    expect(t.alerts.log).toContain("linkState");
  });

  it("baby: status and the mic level reach the baby store", async () => {
    const t = setup();
    t.controller.chooseRole("baby");
    t.core.script = () => [emitStatus({ state: "connected" })];
    t.controller.startPairing();
    await flush();
    t.time.advance(250);
    expect(get(t.controller.baby).link.phase).toBe("connected");
    t.medias[0]!.micRms = 0.25;
    t.time.advance(100);
    expect(get(t.controller.baby).micLevel).toBeCloseTo(0.25);
    expect(t.core.types()).toContain("audio_level");
  });

  it("baby: a browser reports its own device status, never a ringer", async () => {
    const t = setup();
    t.controller.chooseRole("baby");
    t.controller.startPairing();
    await flush();
    t.time.advance(2_000);
    const sent = t.core.inputs.find((i) => i.type === "device_status");
    expect(sent).toEqual({ type: "device_status", status: {} });
  });

  it("listen, snooze, acknowledge, silence and probe become core inputs", async () => {
    const t = setup();
    await pairedParent(t);
    t.controller.setListen("locked");
    t.controller.snooze(10);
    t.controller.cancelSnooze();
    t.controller.acknowledgeAlarm();
    t.controller.requestSilenceBaby();
    t.controller.requestTestAlert();
    const inputs = t.core.inputs.filter((i) => i.type !== "tick" && i.type !== "start");
    expect(inputs).toEqual([
      { type: "set_listen", enabled: true },
      { type: "set_snooze", until_ms: t.time.now + 600_000 },
      { type: "set_snooze", until_ms: null },
      { type: "acknowledge_alarm" },
      { type: "request_quiet" },
      { type: "request_probe" },
    ]);
    expect(get(t.controller.parent).listen).toBe("locked");
    expect(get(t.controller.parent).silenceRequest).toBe("pending");
  });

  it("listen is refused while the link is down", async () => {
    const t = setup();
    await pairedParent(t);
    t.core.script = () => [emitStatus({ state: "reconnecting", level: 1 })];
    t.time.advance(250);
    t.controller.setListen("hold");
    expect(get(t.controller.parent).listen).toBe("off");
    expect(t.core.inputs.some((i) => i.type === "set_listen")).toBe(false);
  });

  it("a test alert without a link runs the local chain and records the time", async () => {
    const t = setup();
    t.controller.chooseRole("parent");
    t.controller.requestTestAlert();
    expect(t.alerts.noise).toEqual([["new", true, false]]);
    expect(get(t.controller.settings).lastTestWallMs).toBe(t.time.wallMs());
  });

  it("a probe from the baby sets the last test time", async () => {
    const t = setup();
    await pairedParent(t);
    t.core.script = () => [
      { type: "emit", event: { type: "noise_alert", rms: 0.5, probe: true, snoozed: false } },
    ];
    t.time.advance(250);
    expect(get(t.controller.settings).lastTestWallMs).toBe(t.time.wallMs());
    expect(get(t.controller.parent)).toMatchObject({ noiseAlertActive: true, noiseIsTest: true });
  });

  it("baby settings changes reach the core's noise gate", async () => {
    const t = setup();
    t.controller.chooseRole("baby");
    t.controller.startPairing();
    await flush();
    t.controller.updateSettings((s) => ({ ...s, threshold: 0.4, holdSeconds: 9 }));
    expect(t.core.inputs).toContainEqual({
      type: "set_noise_gate",
      threshold: 0.4,
      hold_ms: 9_000,
    });
    expect(get(t.controller.baby)).toMatchObject({ threshold: 0.4, holdSeconds: 9 });
  });

  it("the alert gain reaches the parent's playout", async () => {
    const t = setup();
    await pairedParent(t);
    t.controller.updateSettings((s) => ({ ...s, alertGainPct: 130 }));
    expect(t.medias[0]?.calls).toContain("gain:130");
  });

  it("stop sound mutes the playout; the missed summary can be dismissed", async () => {
    const t = setup();
    await pairedParent(t);
    t.controller.stopAlertSound();
    expect(t.medias[0]?.calls).toContain("muted:true");
    t.core.script = () => [
      {
        type: "emit",
        event: { type: "missed_events", count: 1, max_rms: 0.5, complete: true, down_ms: 1000 },
      },
    ];
    t.time.advance(250);
    expect(get(t.controller.parent).missed).not.toBeNull();
    t.controller.dismissMissedEvents();
    expect(get(t.controller.parent).missed).toBeNull();
  });

  it("small flags: onboarding, coach, kiosk, test result", () => {
    const t = setup();
    t.controller.completeOnboarding();
    expect(get(t.controller.app).onboardingDone).toBe(true);
    t.controller.dismissCoach();
    expect(get(t.controller.app).coachPending).toBe(false);
    t.controller.setKioskLocked(true);
    expect(get(t.controller.baby).kioskLocked).toBe(true);
    t.controller.reportTestResult(false);
    expect(get(t.controller.settings).lastTestWallMs).toBeNull();
    t.controller.reportTestResult(true);
    expect(get(t.controller.settings).lastTestWallMs).toBe(t.time.wallMs());
  });
});

describe("WebController: sound gate", () => {
  it("stays locked until unlockSound ran, then reports it", async () => {
    const t = setup();
    expect(get(t.controller.soundUnlocked)).toBe(false);
    t.controller.unlockSound();
    await flush();
    expect(get(t.controller.soundUnlocked)).toBe(true);
    expect(t.ctx.resumeCalls).toBe(1);
  });

  it("starting a run from a click also unlocks the sound", async () => {
    const t = setup();
    t.controller.chooseRole("baby");
    t.controller.startPairing();
    await flush();
    expect(get(t.controller.soundUnlocked)).toBe(true);
  });
});

describe("WebController: TURN server", () => {
  const TURN = "turns:turn.example.org:5349?transport=tcp";

  async function babyRun(t: Setup): Promise<RTCIceServer[]> {
    t.controller.chooseRole("baby");
    t.controller.startPairing();
    await flush();
    return t.media.requests[0]?.ice ?? [];
  }

  it("offers the relay STUN first, then the TURN server with its credentials", async () => {
    const t = setup();
    expect(t.controller.setTurnServer(TURN, "test-only", "pw")).toBe("saved");
    expect(await babyRun(t)).toEqual([
      { urls: ["stun:relay.test:3478"] },
      { urls: [TURN], username: "test-only", credential: "pw" },
    ]);
  });

  it("leaves out the credential keys when the entry has none", async () => {
    const t = setup();
    expect(t.controller.setTurnServer("turn:203.0.113.10:3478", "", "")).toBe("saved");
    const ice = await babyRun(t);
    expect(ice[1]).toEqual({ urls: ["turn:203.0.113.10:3478"] });
    expect(Object.keys(ice[1] ?? {})).toEqual(["urls"]);
  });

  it("takes effect for the next attempt and stops after clearing", async () => {
    const t = setup();
    t.controller.setTurnServer(TURN, "test-only", "pw");
    t.controller.clearTurnServer();
    expect(get(t.controller.settings)).toMatchObject({
      turnUrl: null,
      turnUsername: null,
      turnPasswordSaved: false,
    });
    expect(await babyRun(t)).toEqual([{ urls: ["stun:relay.test:3478"] }]);
  });

  it("reports the entry without the password and keeps it on an empty field", async () => {
    const t = setup();
    t.controller.setTurnServer(TURN, "test-only", "turn-pw-test-only");
    expect(get(t.controller.settings)).toMatchObject({
      turnUrl: TURN,
      turnUsername: "test-only",
      turnPasswordSaved: true,
    });
    expect(JSON.stringify(get(t.controller.settings))).not.toContain("turn-pw-test-only");
    expect(t.controller.setTurnServer(TURN, "test-only", "")).toBe("saved");
    const ice = await babyRun(t);
    expect(ice[1]).toMatchObject({ credential: "turn-pw-test-only" });
  });

  it("rejects bad input and keeps the previous entry", () => {
    const t = setup();
    t.controller.setTurnServer(TURN, "test-only", "pw");
    expect(t.controller.setTurnServer("stun:x", "test-only", "pw")).toBe("invalid_address");
    expect(t.controller.setTurnServer(TURN, "other", "")).toBe("invalid_credentials");
    expect(get(t.controller.settings).turnUrl).toBe(TURN);
    expect(t.controller.setTurnServer("", "", "")).toBe("cleared");
    expect(get(t.controller.settings).turnUrl).toBeNull();
  });

  it("cannot be set or wiped through updateSettings", () => {
    const t = setup();
    t.controller.setTurnServer(TURN, "test-only", "pw");
    t.controller.updateSettings((s) => ({
      ...s,
      turnUrl: "turn:evil",
      turnUsername: "x",
      turnPasswordSaved: false,
    }));
    expect(get(t.controller.settings)).toMatchObject({
      turnUrl: TURN,
      turnUsername: "test-only",
      turnPasswordSaved: true,
    });
  });
});

describe("WebController: pairing material stays in memory", () => {
  let setItem: ReturnType<typeof vi.spyOn>;
  beforeEach(() => {
    window.localStorage.clear();
    window.sessionStorage.clear();
    setItem = vi.spyOn(Storage.prototype, "setItem");
  });
  afterEach(() => {
    setItem.mockRestore();
  });

  it("never writes the key to storage, stores, strings or sockets' URLs", async () => {
    const t = setup();
    t.controller.setTurnServer("turn:turn.example.org", "turn-user-test-only", "turn-pw-test-only");
    t.controller.chooseRole("baby");
    t.controller.startPairing();
    await flush();
    const psk = t.core.configs[0]?.psk_b64url ?? "";
    expect(psk.length).toBeGreaterThan(20);

    const everything = [
      JSON.stringify(get(t.controller.app)),
      JSON.stringify(get(t.controller.pairing)),
      String(get(t.controller.pairing)),
      JSON.stringify(get(t.controller.settings)),
      JSON.stringify(get(t.controller.parent)),
      JSON.stringify(get(t.controller.baby)),
      ...FakeWebSocket.instances.map((w) => w.url),
      ...Object.values(window.localStorage),
      ...Object.values(window.sessionStorage),
    ].join("\n");
    expect(everything).not.toContain(psk);
    // The TURN password follows the same rule. The user name is shown in the settings by design,
    // so it may be in the stores but never in browser storage (the empty-storage check below).
    expect(everything).not.toContain("turn-pw-test-only");
    expect(setItem).not.toHaveBeenCalled();
    expect(window.localStorage.length + window.sessionStorage.length).toBe(0);
  });
});

describe("WebController: with the real core", () => {
  it("a baby run opens the relay socket and says hello", async () => {
    FakeWebSocket.instances = [];
    const t = setup({ core: realCore as unknown as ScriptedCore });
    t.controller.chooseRole("baby");
    t.controller.startPairing();
    await flush();
    expect(FakeWebSocket.instances.map((w) => w.url)).toEqual([RELAY]);
    FakeWebSocket.instances[0]?.open();
    const hello = JSON.parse(FakeWebSocket.instances[0]?.sent[0] ?? "{}") as Record<
      string,
      unknown
    >;
    expect(hello).toMatchObject({ type: "hello", role: "baby", protocolVersion: 3 });
    expect(get(t.controller.baby).link.phase).toBe("connecting");
  });
});

describe("WebController: Nostr mailboxes", () => {
  const NOSTR_A: InviteMailbox = { kind: "nostr", url: "wss://relay-a.example" };
  const NOSTR_B: InviteMailbox = { kind: "nostr", url: "wss://relay-b.example" };

  it("parent: the Nostr relays of the invite go to the core, not to the signaling addresses", async () => {
    const t = setup();
    t.controller.chooseRole("parent");
    const invite = realCore.buildInvite(
      new Uint8Array(32).fill(5),
      [RELAY_MAILBOX, NOSTR_A, NOSTR_B],
      1e10,
    );
    if (!invite.ok) throw new Error("invite");
    t.controller.submitPairingCode(invite.text);
    await flush();
    expect(t.core.configs[0]?.mailboxes).toEqual([NOSTR_A, NOSTR_B]);
    // Only the own relay is a signaling address (and so the only one the ICE servers come from).
    expect(t.media.requests[0]?.ice).toEqual([{ urls: ["stun:relay.test:3478"] }]);
  });

  it("baby: the invite and the config name the configured Nostr relays next to the own relay", async () => {
    const t = setup({ nostrRelays: ["wss://relay-a.example"] });
    t.controller.chooseRole("baby");
    t.controller.startPairing();
    await flush();
    expect(t.core.configs[0]?.mailboxes).toEqual([RELAY_MAILBOX, NOSTR_A]);
    const code = get(t.controller.pairing)?.codePayload ?? "";
    const parsed = realCore.parseInvite(code, 1);
    if (!parsed.ok) throw new Error("the baby's own invite parses");
    expect(parsed.invite.mailboxes).toEqual([RELAY_MAILBOX, NOSTR_A]);
  });

  it("a mailbox update replaces the Nostr relays of the next run as well", async () => {
    const t = setup();
    t.controller.chooseRole("parent");
    const invite = realCore.buildInvite(new Uint8Array(32).fill(5), [RELAY_MAILBOX, NOSTR_A], 1e10);
    if (!invite.ok) throw new Error("invite");
    t.core.script = (input) => (input.type === "start" ? [emitStatus({ state: "connected" })] : []);
    t.controller.submitPairingCode(invite.text);
    await flush();
    t.time.advance(250);
    t.core.script = () => [
      emit({ type: "mailboxes_updated", mailboxes: [RELAY_MAILBOX, NOSTR_B] }),
    ];
    t.time.advance(250);
    t.controller.stopMonitoring();
    t.time.advance(600);
    await flush();
    t.core.script = () => [];
    t.controller.startMonitoring();
    await flush();
    const last = t.core.configs[t.core.configs.length - 1];
    expect(last?.mailboxes).toEqual([NOSTR_B]);
  });
});
