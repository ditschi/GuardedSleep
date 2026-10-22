import { describe, expect, it, vi } from "vitest";
import {
  buildCandidates,
  buildPathCandidates,
  selectSignalingPath,
  type SignalingProbeSocket,
} from "./path-selector.js";

type FakeBehavior =
  { kind: "open"; delayMs?: number } | { kind: "error"; delayMs?: number } | { kind: "timeout" }; // never fires anything

function makeFakeFactory(map: Record<string, FakeBehavior>) {
  const opened: string[] = [];
  const closed: string[] = [];
  function factory(url: string): SignalingProbeSocket {
    opened.push(url);
    const listeners: Record<string, Array<() => void>> = {};
    let readyState = 0;
    const sock: SignalingProbeSocket = {
      addEventListener: (event, cb) => {
        listeners[event] ??= [];
        listeners[event].push(cb);
      },
      removeEventListener: (event, cb) => {
        const arr = listeners[event];
        if (!arr) return;
        const i = arr.indexOf(cb);
        if (i >= 0) arr.splice(i, 1);
      },
      close: () => {
        closed.push(url);
      },
      get readyState() {
        return readyState;
      },
    };
    const behavior = map[url] ?? { kind: "error", delayMs: 0 };
    const fire = (event: "open" | "error" | "close"): void => {
      const arr = listeners[event];
      if (!arr) return;
      for (const cb of arr) cb();
    };
    if (behavior.kind === "open") {
      setTimeout(() => {
        readyState = 1;
        fire("open");
      }, behavior.delayMs ?? 0);
    } else if (behavior.kind === "error") {
      setTimeout(() => fire("error"), behavior.delayMs ?? 0);
    }
    return sock;
  }
  return { factory, opened, closed };
}

describe("buildCandidates", () => {
  it("filters empty strings and keeps order", () => {
    expect(buildCandidates("ws://a/", ["", "ws://b/", " "])).toEqual([
      { url: "ws://a/", label: "primary" },
      { url: "ws://b/", label: "fallback" },
    ]);
  });

  it("returns empty list when primary is empty and no fallbacks", () => {
    expect(buildCandidates("  ", undefined)).toEqual([]);
  });
});

describe("buildPathCandidates", () => {
  it("orders LAN prepends before primary then fallbacks", () => {
    expect(buildPathCandidates(["ws://lan/"], "ws://primary/", ["ws://fb/"])).toEqual([
      { url: "ws://lan/", label: "lan" },
      { url: "ws://primary/", label: "primary" },
      { url: "ws://fb/", label: "fallback" },
    ]);
  });

  it("dedupes same URL across prepends and tail", () => {
    expect(buildPathCandidates(["ws://x/"], "ws://x/", ["ws://y/"])).toEqual([
      { url: "ws://x/", label: "lan" },
      { url: "ws://y/", label: "fallback" },
    ]);
  });

  it("dedupes duplicate entries within prepends", () => {
    expect(buildPathCandidates(["ws://lan/", "  ws://lan/  "], "ws://p/", [])).toEqual([
      { url: "ws://lan/", label: "lan" },
      { url: "ws://p/", label: "primary" },
    ]);
  });

  it("allows empty primary when LAN prepend and fallbacks supply paths", () => {
    expect(buildPathCandidates(["ws://embedded/"], "", ["ws://docker-fallback/"])).toEqual([
      { url: "ws://embedded/", label: "lan" },
      { url: "ws://docker-fallback/", label: "fallback" },
    ]);
  });

  it("probes prepends before primary (both LAN fail, primary opens)", async () => {
    const candidates = buildPathCandidates(
      ["ws://dead-lan-1/", "ws://dead-lan-2/"],
      "ws://cloud/",
      [],
    );
    const { factory, opened } = makeFakeFactory({
      "ws://dead-lan-1/": { kind: "error", delayMs: 0 },
      "ws://dead-lan-2/": { kind: "error", delayMs: 0 },
      "ws://cloud/": { kind: "open", delayMs: 0 },
    });
    const result = await selectSignalingPath(candidates, {
      webSocketFactory: factory,
      perUrlTimeoutMs: 50,
    });
    expect(result).toEqual({ url: "ws://cloud/", label: "primary", index: 2 });
    expect(opened).toEqual(["ws://dead-lan-1/", "ws://dead-lan-2/", "ws://cloud/"]);
  });
});

describe("selectSignalingPath", () => {
  it("returns null for empty list", async () => {
    expect(await selectSignalingPath([])).toBeNull();
  });

  it("returns the first reachable URL and stops probing", async () => {
    const { factory, opened } = makeFakeFactory({
      "ws://a/": { kind: "open", delayMs: 0 },
      "ws://b/": { kind: "open", delayMs: 0 },
    });
    const result = await selectSignalingPath(
      [
        { url: "ws://a/", label: "primary" },
        { url: "ws://b/", label: "fallback" },
      ],
      { webSocketFactory: factory, perUrlTimeoutMs: 50 },
    );
    expect(result).toEqual({ url: "ws://a/", label: "primary", index: 0 });
    expect(opened).toEqual(["ws://a/"]);
  });

  it("falls back when the first URL fails (simulates internet unreachable, LAN reachable)", async () => {
    const { factory, opened } = makeFakeFactory({
      "ws://internet/": { kind: "error", delayMs: 0 },
      "ws://lan-host:8787/": { kind: "open", delayMs: 0 },
    });
    const result = await selectSignalingPath(
      [
        { url: "ws://internet/", label: "internet" },
        { url: "ws://lan-host:8787/", label: "lan" },
      ],
      { webSocketFactory: factory, perUrlTimeoutMs: 50 },
    );
    expect(result).toEqual({ url: "ws://lan-host:8787/", label: "lan", index: 1 });
    expect(opened).toEqual(["ws://internet/", "ws://lan-host:8787/"]);
  });

  it("returns null when all URLs fail", async () => {
    const { factory } = makeFakeFactory({
      "ws://a/": { kind: "error", delayMs: 0 },
      "ws://b/": { kind: "error", delayMs: 0 },
    });
    const result = await selectSignalingPath([{ url: "ws://a/" }, { url: "ws://b/" }], {
      webSocketFactory: factory,
      perUrlTimeoutMs: 50,
    });
    expect(result).toBeNull();
  });

  it("times out URLs that never resolve and moves on", async () => {
    vi.useFakeTimers();
    try {
      const { factory } = makeFakeFactory({
        "ws://stuck/": { kind: "timeout" },
        "ws://good/": { kind: "open", delayMs: 0 },
      });
      const promise = selectSignalingPath([{ url: "ws://stuck/" }, { url: "ws://good/" }], {
        webSocketFactory: factory,
        perUrlTimeoutMs: 100,
      });
      await vi.advanceTimersByTimeAsync(150);
      const result = await promise;
      expect(result).toEqual({ url: "ws://good/", label: undefined, index: 1 });
    } finally {
      vi.useRealTimers();
    }
  });

  it("deduplicates URLs case-insensitively", async () => {
    const { factory, opened } = makeFakeFactory({
      "ws://Host/": { kind: "error", delayMs: 0 },
    });
    await selectSignalingPath(
      [{ url: "ws://Host/" }, { url: "ws://host/" }, { url: "WS://HOST/" }],
      { webSocketFactory: factory, perUrlTimeoutMs: 50 },
    );
    expect(opened).toEqual(["ws://Host/"]);
  });
});
