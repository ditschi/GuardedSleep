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
    const socket = createWebSocketSignaling(FakeWebSocket)("ws://a", l);
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

  it("reports one close when the URL cannot be opened, and never retries on its own", () => {
    const { log, l } = listener();
    createWebSocketSignaling(FakeWebSocket)("ws://a", l);
    FakeWebSocket.instances[0]?.drop();
    expect(log).toEqual(["close"]);
    expect(FakeWebSocket.instances).toHaveLength(1);
  });

  it("treats a constructor that throws as an unreachable URL", () => {
    class Throwing extends FakeWebSocket {
      constructor(url: string) {
        super(url);
        throw new Error("blocked");
      }
    }
    const { log, l } = listener();
    createWebSocketSignaling(Throwing)("ws://bad", l);
    expect(log).toEqual(["close"]);
  });

  it("close silences the callbacks and closes the socket", () => {
    const { log, l } = listener();
    const socket = createWebSocketSignaling(FakeWebSocket)("ws://a", l);
    const ws = FakeWebSocket.instances[0];
    ws?.open();
    socket.close();
    expect(ws?.closed).toBe(true);
    expect(ws?.onclose).toBeNull();
    expect(log).toEqual(["open"]);
  });
});
