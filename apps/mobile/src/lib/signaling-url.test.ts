import { afterEach, describe, expect, it, vi } from "vitest";
import {
  clearStoredSignalingUrl,
  getEffectiveSignalingUrl,
  getStoredSignalingUrl,
  inferSignalingUrlFromPageOrigin,
  isLoopbackSignalingHost,
  resolveSignalingUrlForSession,
  SIGNALING_URL_STORAGE_KEY,
  setStoredSignalingUrl,
  validateSignalingUrl,
} from "./signaling-url.js";

describe("signaling-url", () => {
  afterEach(() => {
    localStorage.clear();
    vi.unstubAllEnvs();
  });

  it("validateSignalingUrl accepts ws and wss", () => {
    expect(validateSignalingUrl("ws://127.0.0.1:8787")).toBeNull();
    expect(validateSignalingUrl("wss://signal.example.com")).toBeNull();
  });

  it("isLoopbackSignalingHost detects localhost and LAN hosts", () => {
    expect(isLoopbackSignalingHost("ws://127.0.0.1:8787")).toBe(true);
    expect(isLoopbackSignalingHost("wss://localhost:8443/signaling")).toBe(true);
    expect(isLoopbackSignalingHost("wss://[::1]:8443/ws")).toBe(true);
    expect(isLoopbackSignalingHost("wss://192.168.1.10:8443/signaling")).toBe(false);
    expect(isLoopbackSignalingHost("https://x")).toBe(false);
  });

  it("validateSignalingUrl rejects http and empty", () => {
    expect(validateSignalingUrl("")).not.toBeNull();
    expect(validateSignalingUrl("   ")).not.toBeNull();
    expect(validateSignalingUrl("http://x")).not.toBeNull();
  });

  it("stored URL overrides vite default", () => {
    vi.stubEnv("VITE_SIGNALING_URL", "ws://from-env:1");
    setStoredSignalingUrl("wss://stored.example");
    expect(getEffectiveSignalingUrl()).toBe("wss://stored.example");
  });

  it("vite default used when nothing stored", () => {
    vi.stubEnv("VITE_SIGNALING_URL", "ws://env-only:9");
    expect(getStoredSignalingUrl()).toBeNull();
    expect(getEffectiveSignalingUrl()).toBe("ws://env-only:9");
  });

  it("clearStoredSignalingUrl removes override", () => {
    vi.stubEnv("VITE_SIGNALING_URL", "ws://env:1");
    setStoredSignalingUrl("wss://x");
    clearStoredSignalingUrl();
    expect(localStorage.getItem(SIGNALING_URL_STORAGE_KEY)).toBeNull();
    expect(getEffectiveSignalingUrl()).toBe("ws://env:1");
  });

  it("resolveSignalingUrlForSession prefers valid invite signal", () => {
    vi.stubEnv("VITE_SIGNALING_URL", "ws://default:1");
    expect(resolveSignalingUrlForSession("wss://invite.example/ws")).toBe(
      "wss://invite.example/ws",
    );
  });

  it("resolveSignalingUrlForSession falls back when invite invalid", () => {
    vi.stubEnv("VITE_SIGNALING_URL", "ws://default:1");
    expect(resolveSignalingUrlForSession("https://bad")).toBe("ws://default:1");
    expect(resolveSignalingUrlForSession(null)).toBe("ws://default:1");
  });

  it("inferSignalingUrlFromPageOrigin uses LAN HTTPS host", () => {
    expect(
      inferSignalingUrlFromPageOrigin({
        protocol: "https:",
        hostname: "192.168.1.42",
        port: "8443",
      }),
    ).toBe("wss://192.168.1.42:8443/signaling");
  });

  it("inferSignalingUrlFromPageOrigin skips loopback", () => {
    expect(
      inferSignalingUrlFromPageOrigin({
        protocol: "https:",
        hostname: "127.0.0.1",
        port: "8443",
      }),
    ).toBeNull();
  });

  it("infers LAN signaling when no override", () => {
    vi.stubEnv("VITE_SIGNALING_URL", "wss://127.0.0.1:8443/signaling");
    vi.stubGlobal("window", {
      location: { protocol: "https:", hostname: "192.168.1.42", port: "8443" },
    });
    expect(getEffectiveSignalingUrl()).toBe("wss://192.168.1.42:8443/signaling");
  });

  it("stored URL wins over page origin inference", () => {
    vi.stubEnv("VITE_SIGNALING_URL", "wss://127.0.0.1:8443/signaling");
    vi.stubGlobal("window", {
      location: { protocol: "https:", hostname: "192.168.1.42", port: "8443" },
    });
    setStoredSignalingUrl("wss://custom.example/ws");
    expect(getEffectiveSignalingUrl()).toBe("wss://custom.example/ws");
  });
});
