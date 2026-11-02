import { beforeEach, describe, expect, it } from "vitest";
import { createWebSocketSignaling, type SignalingListener } from "./signaling";
import { FakeWebSocket } from "./test-support";

function listener() {
  const log: string[] = [];
  const l: SignalingListener = {
    onOpen: () => log.push("open"),
    onMessage: (t) => log.push(`msg:${t}`),
    onClose: () => log.push("close"),
  };
  return { log, l };
}

describe("WebSocket signaling", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
  });

  it("opens the first URL, delivers text frames and sends only while open", () => {
    const { log, l } = listener();
    const socket = createWebSocketSignaling(FakeWebSocket)(["ws://a", "ws://b"], l);
    const ws = FakeWebSocket.instances[0];
    socket.send("too early");
    ws?.open();
    ws?.receive("one");
    ws?.onmessage?.({ data: new ArrayBuffer(2) });
    socket.send("hi");
    expect(ws?.sent).toEqual(["hi"]);
    expect(log).toEqual(["open", "msg:one"]);
    ws?.drop();
    expect(log.at(-1)).toBe("close");
  });

  it("falls through to the next URL when one cannot be reached", () => {
    const { log, l } = listener();
    createWebSocketSignaling(FakeWebSocket)(["ws://dead", "ws://alive"], l);
    FakeWebSocket.instances[0]?.drop();
    expect(FakeWebSocket.instances.map((w) => w.url)).toEqual(["ws://dead", "ws://alive"]);
    FakeWebSocket.instances[1]?.open();
    expect(log).toEqual(["open"]);
  });

  it("reports one close when no URL opens, and never retries on its own", () => {
    const { log, l } = listener();
    createWebSocketSignaling(FakeWebSocket)(["ws://a", "ws://b"], l);
    FakeWebSocket.instances[0]?.drop();
    FakeWebSocket.instances[1]?.drop();
    expect(log).toEqual(["close"]);
    expect(FakeWebSocket.instances).toHaveLength(2);
  });

  it("starts the next connection at the URL that worked last", () => {
    const factory = createWebSocketSignaling(FakeWebSocket);
    const first = listener();
    factory(["ws://dead", "ws://alive"], first.l);
    FakeWebSocket.instances[0]?.drop();
    FakeWebSocket.instances[1]?.open();
    FakeWebSocket.instances[1]?.drop();
    factory(["ws://dead", "ws://alive"], listener().l);
    expect(FakeWebSocket.instances.at(-1)?.url).toBe("ws://alive");
  });

  it("treats a constructor that throws as an unreachable URL", () => {
    class Throwing extends FakeWebSocket {
      constructor(url: string) {
        super(url);
        if (url === "ws://bad") throw new Error("blocked");
      }
    }
    const { log, l } = listener();
    createWebSocketSignaling(Throwing)(["ws://bad", "ws://good"], l);
    FakeWebSocket.instances.at(-1)?.open();
    expect(log).toEqual(["open"]);
  });

  it("close silences the callbacks and closes the socket", () => {
    const { log, l } = listener();
    const socket = createWebSocketSignaling(FakeWebSocket)(["ws://a"], l);
    const ws = FakeWebSocket.instances[0];
    ws?.open();
    socket.close();
    expect(ws?.closed).toBe(true);
    expect(ws?.onclose).toBeNull();
    expect(log).toEqual(["open"]);
  });
});
