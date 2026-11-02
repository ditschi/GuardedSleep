import { describe, expect, it } from "vitest";
import type { PairingError } from "../contract";
import { createTranslator } from "../i18n/translate";
import { babyFailureKey, coreMessageKey, isMicFailure, pairErrorKey } from "./messages";

const t = createTranslator("en");

describe("pairErrorKey", () => {
  const table: Array<[PairingError, string]> = [
    ["camera_denied", "Camera not allowed."],
    ["not_our_code", "This isn't a GuardedSleep code. Scan the code on the baby device."],
    ["expired", "This code has expired. Tap Show new code on the baby device, then scan again."],
    ["key_mismatch", "The code doesn't match. Scan the code that is on the baby device right now."],
    ["already_paired", "This baby device is already paired with another device."],
    ["unreachable", "Can't find the baby device yet. Make sure both devices are online."],
    ["incomplete_paste", "This code is incomplete. Copy the whole code from the baby device."],
  ];

  it.each(table)("%s maps to its catalogue message", (error, text) => {
    expect(t(pairErrorKey(error))).toBe(text);
  });
});

describe("coreMessageKey", () => {
  it("maps the core's snake_case keys onto the core group", () => {
    expect(coreMessageKey("server_unreachable")).toBe("core.serverUnreachable");
    expect(coreMessageKey("pairing_key_mismatch")).toBe("core.pairingKeyMismatch");
    expect(t("core.peerAbsent")).toContain("baby device");
  });

  it("knows nothing about unknown keys, so no raw key reaches the screen", () => {
    expect(coreMessageKey("something_new")).toBeNull();
    expect(coreMessageKey(null)).toBeNull();
    expect(coreMessageKey("toString")).toBeNull();
  });
});

describe("babyFailureKey", () => {
  it("names the real reason of a failed baby run", () => {
    expect(babyFailureKey("mic_denied")).toBe("webScreens.baby.failMicDenied");
    expect(babyFailureKey("mic_unavailable")).toBe("webScreens.baby.failMicUnavailable");
    expect(babyFailureKey("pairing_key_mismatch")).toBe("core.pairingKeyMismatch");
  });

  it("has no reason without a key", () => {
    expect(babyFailureKey(null)).toBeNull();
  });

  it("tells microphone failures apart", () => {
    expect(isMicFailure("mic_denied")).toBe(true);
    expect(isMicFailure("server_unreachable")).toBe(false);
    expect(isMicFailure(null)).toBe(false);
  });
});
