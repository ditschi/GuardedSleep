import type {
  CoreLink,
  LinkAction,
  LinkConfig,
  LinkInput,
  LinkStatus,
} from "@guardedsleep/core-wasm";
import { loadCoreNode } from "@guardedsleep/core-wasm/node";
import { describe, expect, it } from "vitest";
import { createClock } from "./clock";
import { LinkDriver, type LinkListener } from "./link-driver";
import { createWebSocketSignaling } from "./signaling";
import { FakeMedia, FakeTime, FakeWebSocket } from "./test-support";

// Counting bytes 0..31, encoded at run time so no key-shaped literal sits in the source.
const PSK_BYTES = Uint8Array.from({ length: 32 }, (_, i) => i);
const PSK = btoa(String.fromCharCode(...PSK_BYTES))
  .replaceAll("+", "-")
  .replaceAll("/", "_")
  .replaceAll("=", "");

const config = (role: LinkConfig["role"]): LinkConfig => ({
  role,
  psk_b64url: PSK,
  seed: "07".repeat(32),
  heartbeat_interval_ms: 10_000,
  alarm_after_ms: 60_000,
  noise_threshold: 0.12,
  noise_hold_ms: 800,
});

const status = (state: LinkStatus["state"], level = 0): LinkStatus => ({
  state,
  level,
  key: null,
  cause: null,
  acknowledged_until: null,
  last_alive_ms: null,
});

/** A core whose answer to every input is scripted by the test; records the inputs it got. */
class ScriptedCore {
  readonly inputs: Array<{ at: number; input: LinkInput }> = [];
  /** The wall clock reports, kept apart so the input assertions stay about behaviour. */
  readonly wallClocks: number[] = [];
  freed = false;
  script: (input: LinkInput) => LinkAction[] = () => [];

  createLink(): CoreLink {
    return {
      handle: (at, input) => {
        if (input.type === "wall_clock") {
          this.wallClocks.push(input.unix_s);
          return [];
        }
        this.inputs.push({ at, input });
        return this.script(input);
      },
      roomId: () => "room",
      free: () => {
        this.freed = true;
      },
    };
  }

  types(): string[] {
    return this.inputs.map((i) => i.input.type);
  }
}

class Recorder implements LinkListener {
  readonly log: string[] = [];
  statuses: LinkStatus[] = [];
  micLevels: number[] = [];
  remoteLevels: number[] = [];
  torn = false;
  onStatus(s: LinkStatus): void {
    this.statuses.push(s);
    this.log.push(`status:${s.state}`);
  }
  onEvent(e: { type: string }): void {
    this.log.push(`event:${e.type}`);
  }
  onTick(): void {
    this.log.push("tick");
  }
  onMicLevel(rms: number): void {
    this.micLevels.push(rms);
  }
  onRemoteLevel(rms: number): void {
    this.remoteLevels.push(rms);
  }
  onInboundStats(): void {
    this.log.push("stats");
  }
  onTornDown(): void {
    this.torn = true;
  }
}

function setup(role: LinkConfig["role"] = "parent", core: ScriptedCore = new ScriptedCore()) {
  FakeWebSocket.instances = [];
  const time = new FakeTime();
  const media = new FakeMedia();
  const listener = new Recorder();
  const driver = new LinkDriver({
    config: config(role),
    core,
    signaling: createWebSocketSignaling(FakeWebSocket),
    media,
    clock: time,
    timers: time,
    listener,
    entropy: 7,
  });
  return { time, media, listener, driver, core };
}

describe("LinkDriver with a scripted core", () => {
  it("reports the wall clock at start, every 60 s and after a time change", () => {
    const base = 1_800_000_000_000;
    let skew = 0;
    const realNow = Date.now;
    const core = new ScriptedCore();
    const { driver, time } = setup("parent", core);
    try {
      // Date.now follows the manual time plus a skew the test controls.
      Date.now = () => base + time.elapsedMs() + skew;
      driver.start();
      expect(core.wallClocks).toEqual([base / 1000]);
      time.advance(30_000);
      expect(core.wallClocks).toHaveLength(1);
      time.advance(30_500);
      expect(core.wallClocks).toHaveLength(2);
      // The user sets the clock back by an hour: the next tick reports it at once.
      skew = -3_600_000;
      time.advance(500);
      expect(core.wallClocks).toHaveLength(3);
      expect(core.wallClocks[2]).toBeLessThan((core.wallClocks[1] ?? 0) - 3_000);
      // A 1 s wobble is not a time change.
      skew = -3_599_000;
      time.advance(1_000);
      expect(core.wallClocks).toHaveLength(3);
      expect(core.types()).not.toContain("wall_clock");
    } finally {
      Date.now = realNow;
    }
  });

  it("hands the seed to the core once", () => {
    const core = new ScriptedCore();
    const seen: string[] = [];
    const createLink = core.createLink.bind(core);
    core.createLink = ((config: LinkConfig) => {
      seen.push(config.seed);
      return createLink();
    }) as typeof core.createLink;
    const { driver } = setup("parent", core);
    driver.start();
    expect(seen).toEqual(["07".repeat(32)]);
    driver.start();
    expect(seen).toHaveLength(1);
  });

  it("starts the core with the entropy and executes the actions it returns", () => {
    const core = new ScriptedCore();
    core.script = (input) =>
      input.type === "start"
        ? [
            { type: "open_mailbox", id: 0, kind: "relay", url: "ws://relay.test:8787" },
            { type: "emit", event: { type: "status", status: status("connecting") } },
          ]
        : [];
    const { driver, listener } = setup("parent", core);
    driver.start();
    expect(core.inputs[0]?.input).toEqual({ type: "start", entropy: 7 });
    expect(FakeWebSocket.instances).toHaveLength(1);
    expect(FakeWebSocket.instances[0]?.url).toBe("ws://relay.test:8787");
    expect(listener.log).toEqual(["status:connecting"]);
  });

  it("feeds signaling events to the core and ignores a replaced socket", () => {
    const core = new ScriptedCore();
    core.script = (input) =>
      input.type === "start"
        ? [{ type: "open_mailbox", id: 0, kind: "relay", url: "ws://relay.test:8787" }]
        : [];
    const { driver } = setup("parent", core);
    driver.start();
    const first = FakeWebSocket.instances[0];
    first?.open();
    first?.receive('{"type":"welcome"}');
    expect(core.inputs.slice(1).map((i) => i.input)).toEqual([
      { type: "mailbox_opened", id: 0 },
      { type: "mailbox_frame", id: 0, bytes: '{"type":"welcome"}' },
    ]);
    // The core asks for a new socket; late events of the old one no longer reach it.
    core.script = (input) =>
      input.type === "mailbox_closed"
        ? [{ type: "open_mailbox", id: 0, kind: "relay", url: "ws://relay.test:8787" }]
        : [];
    first?.drop();
    expect(FakeWebSocket.instances).toHaveLength(2);
    expect(first?.closed).toBe(false); // already closed by the peer; nothing left to close
    const before = core.inputs.length;
    first?.receive("late");
    first?.drop();
    expect(core.inputs).toHaveLength(before);
  });

  it("sends the core's signaling text through the open socket", () => {
    const core = new ScriptedCore();
    core.script = (input) => {
      if (input.type === "start")
        return [{ type: "open_mailbox", id: 0, kind: "relay", url: "ws://relay.test:8787" }];
      if (input.type === "mailbox_opened") return [{ type: "mailbox_send", id: 0, frame: "hello" }];
      return [];
    };
    const { driver } = setup("parent", core);
    driver.start();
    FakeWebSocket.instances[0]?.open();
    expect(FakeWebSocket.instances[0]?.sent).toEqual(["hello"]);
  });

  it("opens a mailbox with its own address at that address only, next to the relay's list", () => {
    const core = new ScriptedCore();
    core.script = (input) =>
      input.type === "start"
        ? [
            { type: "open_mailbox", id: 0, kind: "relay", url: "ws://relay.test:8787" },
            { type: "open_mailbox", id: 1, kind: "relay", url: "wss://relay-a.example" },
          ]
        : [];
    const { driver } = setup("parent", core);
    driver.start();
    expect(FakeWebSocket.instances.map((w) => w.url)).toEqual([
      "ws://relay.test:8787",
      "wss://relay-a.example",
    ]);
    // Each socket speaks for its own mailbox id.
    FakeWebSocket.instances[1]?.open();
    FakeWebSocket.instances[1]?.receive("frame");
    expect(core.inputs.slice(1).map((i) => i.input)).toEqual([
      { type: "mailbox_opened", id: 1 },
      { type: "mailbox_frame", id: 1, bytes: "frame" },
    ]);
  });

  it("drops a mailbox frame over 64 KiB before the core sees it", () => {
    const core = new ScriptedCore();
    core.script = (input) =>
      input.type === "start"
        ? [{ type: "open_mailbox", id: 1, kind: "relay", url: "wss://relay-a.example" }]
        : [];
    const { driver } = setup("parent", core);
    driver.start();
    const socket = FakeWebSocket.instances[0];
    socket?.open();
    socket?.receive("x".repeat(64 * 1024 + 1));
    socket?.receive("x".repeat(64 * 1024));
    const frames = core.inputs.slice(1).filter((i) => i.input.type === "mailbox_frame");
    expect(frames).toHaveLength(1);
  });

  it("runs the attempt lifecycle from the core's actions and reports the browser's events", () => {
    const core = new ScriptedCore();
    core.script = (input) => {
      if (input.type === "start") return [{ type: "create_attempt", attempt: 3, offerer: true }];
      return [];
    };
    const { driver, media } = setup("baby", core);
    driver.start();
    const attempt = media.attempts[0];
    expect(attempt?.offerer).toBe(true);

    core.script = () => [
      { type: "create_offer", attempt: 3 },
      { type: "set_remote_description", attempt: 3, sdp_type: "answer", sdp: "v=0" },
      { type: "create_answer", attempt: 3 },
      {
        type: "add_ice_candidate",
        attempt: 3,
        candidate: { candidate: "cand", sdp_mid: "0", sdp_m_line_index: 0 },
      },
      { type: "send_data", attempt: 3, text: "ping" },
    ];
    driver.input({ type: "network_available" });
    expect(attempt?.calls).toEqual([
      "createOffer",
      "setRemoteDescription:answer:v=0",
      "createAnswer",
      "addRemoteIce:cand",
    ]);
    expect(attempt?.sent).toEqual(["ping"]);

    core.script = () => [];
    attempt?.listener.onLocalDescription("offer", "sdp");
    attempt?.listener.onLocalIce({ candidate: "c", sdp_mid: null, sdp_m_line_index: null });
    attempt?.listener.onDataOpen();
    attempt?.listener.onDataMessage("hb");
    attempt?.listener.onConnectionState("connected");
    attempt?.listener.onDtlsFingerprint("AB:CD");
    attempt?.listener.onDataClosed();
    expect(core.inputs.slice(2).map((i) => i.input)).toEqual([
      { type: "attempt_local_description", attempt: 3, sdp_type: "offer", sdp: "sdp" },
      {
        type: "attempt_local_ice",
        attempt: 3,
        candidate: { candidate: "c", sdp_mid: null, sdp_m_line_index: null },
      },
      { type: "attempt_data_open", attempt: 3 },
      { type: "attempt_data_message", attempt: 3, text: "hb" },
      { type: "attempt_pc_state", attempt: 3, state: "connected" },
      { type: "attempt_dtls_fingerprint", attempt: 3, fingerprint: "AB:CD" },
      { type: "attempt_data_closed", attempt: 3 },
    ]);
  });

  it("closes an attempt on request and drops its late events", () => {
    const core = new ScriptedCore();
    core.script = (input) =>
      input.type === "start" ? [{ type: "create_attempt", attempt: 1, offerer: false }] : [];
    const { driver, media } = setup("parent", core);
    driver.start();
    core.script = () => [{ type: "close_attempt", attempt: 1 }];
    driver.input({ type: "tick" });
    const attempt = media.attempts[0];
    expect(attempt?.closed).toBe(true);
    const seen = core.inputs.length;
    attempt?.listener.onConnectionState("failed");
    attempt?.listener.onDtlsFingerprint("AB");
    expect(core.inputs).toHaveLength(seen);
  });

  it("tells the core a failed attempt when the peer connection cannot be created", () => {
    const core = new ScriptedCore();
    core.script = (input) =>
      input.type === "start" ? [{ type: "create_attempt", attempt: 5, offerer: true }] : [];
    const { driver, media, time } = setup("baby", core);
    media.throwOnCreate = true;
    driver.start();
    time.advance(1);
    expect(core.inputs.map((i) => i.input)).toContainEqual({
      type: "attempt_pc_state",
      attempt: 5,
      state: "failed",
    });
  });

  it("executes the media actions of the core", () => {
    const core = new ScriptedCore();
    core.script = (input) =>
      input.type === "start"
        ? [
            { type: "set_mic_enabled", enabled: true },
            { type: "send_probe_tone", duration_ms: 1500 },
          ]
        : [];
    const { driver, media } = setup("baby", core);
    driver.start();
    expect(media.calls).toEqual(["mic:true", "probe:1500"]);
  });

  it("routes status and other events to the listener", () => {
    const core = new ScriptedCore();
    core.script = (input) =>
      input.type === "start"
        ? [
            { type: "emit", event: { type: "status", status: status("connected") } },
            { type: "emit", event: { type: "peer_stopped" } },
          ]
        : [];
    const { driver, listener } = setup("parent", core);
    driver.start();
    expect(listener.log).toEqual(["status:connected", "event:peer_stopped"]);
  });

  it("feeds a tick every 250 ms with a monotonic time", () => {
    const core = new ScriptedCore();
    const { driver, time, listener } = setup("parent", core);
    driver.start();
    time.advance(1000);
    const ticks = core.inputs.filter((i) => i.input.type === "tick");
    expect(ticks.map((t) => t.at)).toEqual([250, 500, 750, 1000]);
    expect(listener.log.filter((l) => l === "tick")).toHaveLength(4);
  });

  it("hands a sleep gap to the core as elapsed time and never goes back in time", () => {
    const core = new ScriptedCore();
    FakeWebSocket.instances = [];
    const time = new FakeTime();
    // The device's wall clock; the production clock reads `Date.now()` clamped the same way.
    const wall = { now: 1_700_000_000_000 };
    const driver = new LinkDriver({
      config: config("parent"),
      core,
      signaling: createWebSocketSignaling(FakeWebSocket),
      media: new FakeMedia(),
      clock: createClock(() => wall.now),
      timers: time,
      listener: new Recorder(),
      entropy: 7,
    });
    driver.start();
    wall.now += 250;
    time.advance(250);
    // The laptop sleeps for ten minutes: the next tick carries the whole gap.
    wall.now += 10 * 60_000;
    time.advance(250);
    // The wall clock is set back by a minute: the core's time does not.
    wall.now -= 60_000;
    time.advance(250);
    const ticks = core.inputs.filter((i) => i.input.type === "tick").map((i) => i.at);
    expect(ticks).toEqual([1_700_000_000_250, 1_700_000_600_250, 1_700_000_600_250]);
  });

  it("reports microphone levels to the meter and the core (baby), remote levels to the meter (parent)", () => {
    const baby = setup("baby");
    baby.driver.start();
    baby.media.micRms = 0.4;
    baby.time.advance(100);
    expect(baby.listener.micLevels).toEqual([0.4]);
    expect(baby.core.types()).toContain("audio_level");
    expect(baby.core.inputs.find((i) => i.input.type === "audio_level")?.input).toEqual({
      type: "audio_level",
      rms: 0.4,
    });

    const parent = setup("parent");
    parent.driver.start();
    parent.media.remoteRms = 0.2;
    parent.time.advance(100);
    expect(parent.listener.remoteLevels).toEqual([0.2]);
    expect(parent.core.types()).not.toContain("audio_level");
  });

  it("sends the baby's device status on the first read and on change or after the resend time", () => {
    const core = new ScriptedCore();
    FakeWebSocket.instances = [];
    const time = new FakeTime();
    let battery = 80;
    const driver = new LinkDriver({
      config: config("baby"),
      core,
      signaling: createWebSocketSignaling(FakeWebSocket),
      media: new FakeMedia(),
      clock: time,
      timers: time,
      listener: new Recorder(),
      entropy: 1,
      deviceStatus: () => ({ battery_pct: battery }),
    });
    driver.start();
    const sent = (): number => core.inputs.filter((i) => i.input.type === "device_status").length;
    time.advance(2000);
    expect(sent()).toBe(1);
    time.advance(2000);
    expect(sent()).toBe(1); // unchanged
    battery = 81; // rounds to the same 5 % step
    time.advance(2000);
    expect(sent()).toBe(1);
    battery = 60;
    time.advance(2000);
    expect(sent()).toBe(2);
    time.advance(10_000); // resend although unchanged
    expect(sent()).toBe(3);
  });

  it("answers apply_quiet with the fresh state", () => {
    const core = new ScriptedCore();
    core.script = (input) => (input.type === "request_quiet" ? [{ type: "apply_quiet" }] : []);
    FakeWebSocket.instances = [];
    const time = new FakeTime();
    const driver = new LinkDriver({
      config: config("baby"),
      core,
      signaling: createWebSocketSignaling(FakeWebSocket),
      media: new FakeMedia(),
      clock: time,
      timers: time,
      listener: new Recorder(),
      entropy: 1,
      deviceStatus: () => ({}),
      applyQuiet: () => ({ quiet_active: true }),
    });
    driver.start();
    driver.input({ type: "request_quiet" });
    expect(core.inputs.at(-1)?.input).toEqual({
      type: "quiet_applied",
      state: { quiet_active: true },
    });
  });

  it("never re-enters the core: an input produced during an action runs after it", () => {
    const core = new ScriptedCore();
    const order: string[] = [];
    core.script = (input) => {
      order.push(input.type);
      return input.type === "start"
        ? [{ type: "open_mailbox", id: 0, kind: "relay", url: "ws://relay.test:8787" }]
        : [];
    };
    const { driver } = setup("parent", core);
    driver.start();
    FakeWebSocket.instances[0]?.open();
    expect(order).toEqual(["start", "mailbox_opened"]);
  });

  it("announces a stop, flushes the close and tears down", () => {
    const core = new ScriptedCore();
    core.script = (input) => {
      if (input.type === "start") return [{ type: "create_attempt", attempt: 1, offerer: false }];
      if (input.type === "stop") {
        return [
          { type: "send_data", attempt: 1, text: "stop_notice" },
          { type: "close_attempt", attempt: 1 },
          { type: "close_mailbox", id: 0 },
        ];
      }
      return [];
    };
    const { driver, media, time, listener } = setup("parent", core);
    driver.start();
    driver.stop();
    expect(media.attempts[0]?.sent).toEqual(["stop_notice"]);
    expect(media.attempts[0]?.closed).toBe(false); // the notice needs a moment to leave
    time.advance(300);
    expect(media.attempts[0]?.closed).toBe(true);
    expect(listener.torn).toBe(false);
    time.advance(200);
    expect(listener.torn).toBe(true);
    expect(media.released).toBe(true);
    expect(core.freed).toBe(true);
    expect(time.pending()).toBe(0);
    // Inputs after a stop are ignored.
    const seen = core.inputs.length;
    driver.input({ type: "tick" });
    expect(core.inputs).toHaveLength(seen);
  });

  it("abort tears down at once without telling the peer", () => {
    const core = new ScriptedCore();
    const { driver, listener, time } = setup("parent", core);
    driver.start();
    driver.abort();
    expect(listener.torn).toBe(true);
    expect(core.types()).not.toContain("stop");
    expect(time.pending()).toBe(0);
  });

  it("survives a core that throws on one input", () => {
    const core = new ScriptedCore();
    let calls = 0;
    core.script = () => {
      if (++calls === 2) throw new Error("boom");
      return [];
    };
    const { driver, time } = setup("parent", core);
    driver.start();
    time.advance(500);
    expect(core.types().filter((t) => t === "tick").length).toBeGreaterThanOrEqual(1);
  });
});

describe("LinkDriver with the real core", () => {
  const core = loadCoreNode();

  it("says hello with the derived room id and reconnects as the core decides", () => {
    FakeWebSocket.instances = [];
    const time = new FakeTime();
    const listener = new Recorder();
    const driver = new LinkDriver({
      config: { ...config("parent"), mailboxes: [{ kind: "relay", url: "wss://relay.test:8787" }] },
      core,
      signaling: createWebSocketSignaling(FakeWebSocket),
      media: new FakeMedia(),
      clock: time,
      timers: time,
      listener,
      entropy: 42,
    });
    driver.start();
    const first = FakeWebSocket.instances[0];
    expect(first).toBeDefined();
    first?.open();
    const hello = JSON.parse(first?.sent[0] ?? "{}") as Record<string, unknown>;
    expect(hello).toMatchObject({
      type: "hello",
      role: "parent",
      roomId: core.deriveRoomId(PSK_BYTES),
      protocolVersion: 3,
    });

    // The relay drops the connection: the core asks for a new socket at once (recorded sample in
    // the core-wasm tests), the driver only opens it.
    first?.drop();
    expect(listener.statuses.at(-1)?.state).toBe("connecting");
    expect(FakeWebSocket.instances).toHaveLength(2);
    expect(FakeWebSocket.instances[1]?.url).toBe("wss://relay.test:8787");

    // Long without contact, the core escalates and the driver only reports it.
    time.advance(70_000);
    const last = listener.statuses.at(-1);
    expect(last?.level).toBeGreaterThanOrEqual(2);
    expect(last?.cause).toBe("server_unreachable");
    driver.abort();
  });
});
