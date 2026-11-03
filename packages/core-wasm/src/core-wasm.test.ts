import { describe, expect, it } from "vitest";
import {
  createCoreApi,
  LINK_MESSAGE_KEYS,
  type InviteMailbox,
  type LinkAction,
  type LinkConfig,
} from "./index.js";
import { loadCoreNode } from "./node.js";

const PSK = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8";

const config = (role: LinkConfig["role"]): LinkConfig => ({
  role,
  psk_b64url: PSK,
  seed: "07".repeat(32),
  heartbeat_interval_ms: 10_000,
  alarm_after_ms: 60_000,
  noise_threshold: 0.12,
  noise_hold_ms: 800,
});

/** Recorded from the real core (parent, entropy 42): start, open, close, 60 s without contact. */
const RECORDED: { at: number; input: object; actions: LinkAction[] }[] = [
  {
    at: 1000,
    input: { type: "start", entropy: 42 },
    actions: [
      { type: "open_mailbox", id: 0, kind: "relay", url: "" },
      {
        type: "emit",
        event: {
          type: "status",
          status: {
            state: "connecting",
            level: 0,
            key: null,
            cause: null,
            acknowledged_until: null,
            last_alive_ms: null,
          },
        },
      },
    ],
  },
  {
    at: 1001,
    input: { type: "mailbox_opened", id: 0 },
    actions: [
      {
        type: "mailbox_send",
        id: 0,
        frame:
          '{"type":"hello","role":"parent","roomId":"FrcBBWi_uGBOkaphqPhl_w","protocolVersion":3}',
      },
    ],
  },
  {
    at: 2000,
    input: { type: "mailbox_closed", id: 0, reason: "closed" },
    actions: [{ type: "open_mailbox", id: 0, kind: "relay", url: "" }],
  },
  {
    at: 62_000,
    input: { type: "tick" },
    actions: [
      { type: "close_mailbox", id: 0 },
      { type: "emit", event: { type: "link_lost", down_ms: 61_000 } },
      {
        type: "emit",
        event: {
          type: "status",
          status: {
            state: "connecting",
            level: 2,
            key: null,
            cause: "server_unreachable",
            acknowledged_until: null,
            last_alive_ms: null,
          },
        },
      },
    ],
  },
];

describe("core-wasm wrapper", () => {
  const core = loadCoreNode();

  it("lists exactly the message keys the typed table knows", () => {
    expect([...core.messageKeys()].sort()).toEqual([...LINK_MESSAGE_KEYS].sort());
  });

  it("derives the room id from the PSK bytes", () => {
    const psk = Uint8Array.from({ length: 32 }, (_, i) => i);
    expect(core.deriveRoomId(psk)).toBe("FrcBBWi_uGBOkaphqPhl_w");
    expect(core.createLink(config("parent")).roomId()).toBe("FrcBBWi_uGBOkaphqPhl_w");
  });

  it("replays the recorded sample action for action", () => {
    const link = core.createLink(config("parent"));
    for (const step of RECORDED) {
      expect(link.handle(step.at, step.input as never)).toEqual(step.actions);
    }
  });

  it("rejects an invalid PSK and invalid timing", () => {
    expect(() => core.createLink({ ...config("baby"), psk_b64url: "" })).toThrow();
    expect(() => core.createLink({ ...config("baby"), alarm_after_ms: 1000 })).toThrow();
  });

  it("refuses a link without a 32-byte seed", () => {
    expect(() => core.createLink({ ...config("baby"), seed: "" })).toThrow();
    expect(() => core.createLink({ ...config("baby"), seed: "07".repeat(31) })).toThrow();
    expect(() => core.createLink({ ...config("baby"), seed: "zz".repeat(32) })).toThrow();
  });

  describe("invite codec", () => {
    const psk = Uint8Array.from({ length: 32 }, (_, i) => i);
    const mailboxes: InviteMailbox[] = [
      { kind: "relay", url: "wss://gs.example.com:8443/sig" },
      { kind: "relay", url: "wss://relay.example.org" },
      { kind: "lan", url: "ws://10.0.2.2:8787" },
    ];
    const now = 1_700_000_000;

    it("builds and parses an invite as typed values", () => {
      const built = core.buildInvite(psk, mailboxes, now + 300);
      if (!built.ok) throw new Error("build failed");
      expect(JSON.parse(built.text)).toEqual({
        v: 4,
        k: PSK,
        m: ["r:gs.example.com:8443/sig", "r:relay.example.org", "l:10.0.2.2:8787"],
        x: now + 300,
      });
      const parsed = core.parseInvite(` ${built.text}\n`, now);
      expect(parsed).toEqual({ ok: true, invite: { psk, mailboxes, expiresAtEpochS: now + 300 } });
    });

    it("classifies rejected texts", () => {
      const built = core.buildInvite(psk, mailboxes, now + 300);
      if (!built.ok) throw new Error("build failed");
      expect(core.parseInvite(built.text.slice(0, -10), now)).toEqual({
        ok: false,
        error: "incomplete_paste",
      });
      expect(core.parseInvite("https://example.com", now)).toEqual({
        ok: false,
        error: "not_our_code",
      });
      expect(core.parseInvite(built.text, now + 300)).toEqual({ ok: false, error: "expired" });
    });

    it("rejects a v3 invite", () => {
      const v3 = JSON.stringify({ v: 3, k: PSK, s: ["wss://gs.example.com"], x: now + 300 });
      expect(core.parseInvite(v3, now)).toEqual({ ok: false, error: "not_our_code" });
    });

    it("refuses to build an invalid invite", () => {
      const relay: InviteMailbox = { kind: "relay", url: "wss://gs.example.com" };
      expect(core.buildInvite(psk.slice(0, 16), [relay], now)).toEqual({
        ok: false,
        error: "not_our_code",
      });
      expect(core.buildInvite(psk, [], now)).toEqual({ ok: false, error: "not_our_code" });
      expect(core.buildInvite(psk, [{ kind: "relay", url: "ws://h" }], now)).toEqual({
        ok: false,
        error: "not_our_code",
      });
      expect(core.buildInvite(psk, [{ kind: "lan", url: "ws://8.8.8.8:80" }], now)).toEqual({
        ok: false,
        error: "not_our_code",
      });
    });

    it("accepts loopback only when development builds ask for it", () => {
      const loopback: InviteMailbox = { kind: "lan", url: "ws://127.0.0.1:8787" };
      expect(core.buildInvite(psk, [loopback], now + 300)).toEqual({
        ok: false,
        error: "not_our_code",
      });
      const dev = loadCoreNode({ allowDevLoopback: true });
      const built = dev.buildInvite(psk, [loopback], now + 300);
      if (!built.ok) throw new Error("dev build failed");
      expect(dev.parseInvite(built.text, now).ok).toBe(true);
      expect(core.parseInvite(built.text, now)).toEqual({ ok: false, error: "not_our_code" });
    });
  });

  it("refuses an n: invite entry of the removed Nostr kind", () => {
    const text = JSON.stringify({ v: 4, k: PSK, m: ["n:relay.example.org"], x: 1_700_000_300 });
    expect(core.parseInvite(text, 1_700_000_000)).toEqual({ ok: false, error: "not_our_code" });
  });

  it("fails loudly when the core returns an unknown action", () => {
    const fake = createCoreApi({
      WasmLink: class {
        handle(): string {
          return '[{"type":"from_the_future"}]';
        }
        roomId(): string {
          return "r";
        }
        free(): void {}
      },
      messageKeys: () => "[]",
      deriveRoomId: () => "",
      parseInvite: () => "{}",
      buildInvite: () => "",
      conformanceEval: () => "{}",
    });
    expect(() => fake.createLink(config("baby")).handle(0, { type: "tick" })).toThrow(
      /unknown action/,
    );
  });
});
