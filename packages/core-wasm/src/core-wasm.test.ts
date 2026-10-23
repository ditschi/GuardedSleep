import { describe, expect, it } from "vitest";
import { createCoreApi, LINK_MESSAGE_KEYS, type LinkAction, type LinkConfig } from "./index.js";
import { loadCoreNode } from "./node.js";

const PSK = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8";

const config = (role: LinkConfig["role"]): LinkConfig => ({
  role,
  psk_b64url: PSK,
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
      { type: "open_signaling" },
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
    input: { type: "signaling_opened" },
    actions: [
      {
        type: "send_signaling",
        text: '{"type":"hello","role":"parent","roomId":"FrcBBWi_uGBOkaphqPhl_w","protocolVersion":3}',
      },
    ],
  },
  { at: 2000, input: { type: "signaling_closed" }, actions: [{ type: "open_signaling" }] },
  {
    at: 62_000,
    input: { type: "tick" },
    actions: [
      { type: "close_signaling" },
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

  describe("invite codec", () => {
    const psk = Uint8Array.from({ length: 32 }, (_, i) => i);
    const urls = ["ws://10.0.2.2:8787", "wss://a.example/sig"];
    const now = 1_700_000_000;

    it("builds and parses an invite as typed values", () => {
      const built = core.buildInvite(psk, urls, now + 300);
      if (!built.ok) throw new Error("build failed");
      expect(JSON.parse(built.text)).toEqual({ v: 3, k: PSK, s: urls, x: now + 300 });
      const parsed = core.parseInvite(` ${built.text}\n`, now);
      expect(parsed).toEqual({ ok: true, invite: { psk, urls, expiresAtEpochS: now + 300 } });
    });

    it("classifies rejected texts", () => {
      const built = core.buildInvite(psk, urls, now + 300);
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

    it("refuses to build an invalid invite", () => {
      expect(core.buildInvite(psk.slice(0, 16), urls, now)).toEqual({
        ok: false,
        error: "not_our_code",
      });
      expect(core.buildInvite(psk, ["http://h"], now)).toEqual({
        ok: false,
        error: "not_our_code",
      });
    });
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
