import type { ConnectionStatusChangeListener } from "@capacitor/network";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Network } from "@capacitor/network";

const hoisted = vi.hoisted(() => ({
  isNative: false,
  removeMock: vi.fn().mockResolvedValue(undefined),
  listenerCb: null as ConnectionStatusChangeListener | null,
}));

vi.mock("@capacitor/core", () => ({
  Capacitor: {
    isNativePlatform: () => hoisted.isNative,
  },
}));

vi.mock("@capacitor/network", () => ({
  Network: {
    getStatus: vi.fn(() => Promise.resolve({ connected: true, connectionType: "wifi" })),
    addListener: vi.fn(),
  },
}));

async function flushPromises(n = 8): Promise<void> {
  for (let i = 0; i < n; i += 1) await Promise.resolve();
}

import { subscribeNetworkHandoff } from "./network-handoff.js";

describe("subscribeNetworkHandoff", () => {
  let unsub: (() => void) | undefined;

  afterEach(() => {
    unsub?.();
    unsub = undefined;
  });

  beforeEach(() => {
    hoisted.isNative = true;
    hoisted.listenerCb = null;
    hoisted.removeMock.mockReset().mockResolvedValue(undefined);
    vi.mocked(Network.addListener).mockClear();
    vi.mocked(Network.getStatus).mockClear();
    vi.mocked(Network.getStatus).mockResolvedValue({ connected: true, connectionType: "wifi" });
    vi.mocked(Network.addListener).mockImplementation((_event, cb) => {
      hoisted.listenerCb = cb;
      return Promise.resolve({ remove: hoisted.removeMock });
    });
  });

  it("does not touch Network APIs when not on native platform", () => {
    hoisted.isNative = false;
    unsub = subscribeNetworkHandoff(vi.fn());
    expect(Network.addListener).not.toHaveBeenCalled();
    expect(Network.getStatus).not.toHaveBeenCalled();
  });

  it("calls remove when unsubscribed before addListener promise resolves", async () => {
    let resolveAdd!: (h: { remove: () => Promise<void> }) => void;
    const pending = new Promise<{ remove: () => Promise<void> }>((res) => {
      resolveAdd = res;
    });
    vi.mocked(Network.addListener).mockReturnValueOnce(pending);
    unsub = subscribeNetworkHandoff(vi.fn());
    unsub();
    unsub = undefined;
    resolveAdd({ remove: hoisted.removeMock });
    await flushPromises();
    expect(hoisted.removeMock).toHaveBeenCalledTimes(1);
  });

  it("debounces distinct fingerprints before invoking callback", async () => {
    vi.useFakeTimers();
    try {
      const onHandoff = vi.fn();
      unsub = subscribeNetworkHandoff(onHandoff);
      await flushPromises();
      hoisted.listenerCb?.({ connected: true, connectionType: "wifi" });
      hoisted.listenerCb?.({ connected: true, connectionType: "cellular" });
      hoisted.listenerCb?.({ connected: true, connectionType: "none" });
      await vi.advanceTimersByTimeAsync(2999);
      expect(onHandoff).not.toHaveBeenCalled();
      await vi.advanceTimersByTimeAsync(2);
      expect(onHandoff).toHaveBeenCalledTimes(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it("removes listener on cleanup after subscription settled", async () => {
    unsub = subscribeNetworkHandoff(vi.fn());
    await flushPromises();
    expect(Network.addListener).toHaveBeenCalledTimes(1);
    unsub();
    unsub = undefined;
    await flushPromises();
    expect(hoisted.removeMock).toHaveBeenCalledTimes(1);
  });
});
