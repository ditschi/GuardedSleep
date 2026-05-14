import { describe, expect, it } from "vitest";
import { generatePskBase64Url } from "./pairing/compact-invite.js";
import {
  buildInviteUrl,
  isCompleteInvite,
  parseInviteFromString,
  parseInviteSearchParams,
} from "./invite-url.js";

describe("buildInviteUrl", () => {
  it("builds /pair with room and signal", () => {
    const u = buildInviteUrl({
      origin: "https://example.com",
      room: "abcd1234",
      signal: "wss://sig.example/ws",
    });
    expect(u).toBe("https://example.com/pair?room=abcd1234&signal=wss%3A%2F%2Fsig.example%2Fws");
  });
});

describe("parseInviteSearchParams", () => {
  it("parses valid pair", () => {
    const p = new URLSearchParams();
    p.set("room", "nursery-01");
    p.set("signal", "ws://127.0.0.1:8787");
    expect(parseInviteSearchParams(p)).toEqual({
      room: "nursery-01",
      signal: "ws://127.0.0.1:8787",
    });
  });

  it("drops invalid room", () => {
    const p = new URLSearchParams();
    p.set("room", "no");
    p.set("signal", "ws://127.0.0.1:8787");
    expect(parseInviteSearchParams(p)).toEqual({ signal: "ws://127.0.0.1:8787" });
  });

  it("drops invalid signal", () => {
    const p = new URLSearchParams();
    p.set("room", "validroom");
    p.set("signal", "https://bad");
    expect(parseInviteSearchParams(p)).toEqual({ room: "validroom" });
  });
});

describe("parseInviteFromString", () => {
  it("parses full URL", () => {
    const part = parseInviteFromString(
      "https://app.test/pair?room=helloworld12&signal=wss%3A%2F%2Fs.test%2F",
    );
    expect(isCompleteInvite(part)).toBe(true);
    if (isCompleteInvite(part)) {
      expect(part.room).toBe("helloworld12");
      expect(part.signal).toBe("wss://s.test/");
    }
  });

  it("returns empty for garbage", () => {
    expect(parseInviteFromString("not a url")).toEqual({});
  });

  it("parses compact JSON invite", () => {
    const k = generatePskBase64Url();
    const raw = JSON.stringify({
      v: 1,
      r: "nurseryroom01",
      s: "ws://127.0.0.1:8787",
      k,
      t: 1,
    });
    const p = parseInviteFromString(raw);
    expect(p.room).toBe("nurseryroom01");
    expect(p.signal).toBe("ws://127.0.0.1:8787");
    expect(p.pskB64).toBe(k);
  });
});
