import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { once } from "node:events";
import { describe, expect, it, vi } from "vitest";
import WebSocket, { WebSocketServer } from "ws";
import { PROTOCOL_VERSION } from "@guardedsleep/protocol";
import { attachSignalingServer } from "./server.js";
import { createWebSocketServerOptions } from "./ws-options.js";

/**
 * Live WebSocket smoke test (ping/pong only).
 * Full hello/relay coverage is in packages/protocol and manual runs; Vitest had
 * intermittent hangs when combining workspace protocol imports with multiple WS clients.
 */
function createTestHttpServer() {
  return createServer((_req, res) => {
    res.writeHead(200, { "content-type": "text/plain" });
    res.end("ok\n");
  });
}

async function shutdown(
  httpServer: ReturnType<typeof createServer>,
  wss: WebSocketServer,
): Promise<void> {
  for (const client of wss.clients) {
    try {
      client.terminate();
    } catch {
      /* ignore */
    }
  }
  await new Promise<void>((resolve) => {
    wss.close(() => {
      httpServer.close(() => resolve());
    });
  });
}

describe("signaling server", () => {
  it("WebSocket ping/pong", async () => {
    const httpServer = createTestHttpServer();
    const wss = new WebSocketServer({
      server: httpServer,
      ...createWebSocketServerOptions(),
    });
    attachSignalingServer(wss);
    await new Promise<void>((resolve, reject) => {
      httpServer.once("listening", resolve);
      httpServer.once("error", reject);
      httpServer.listen(0, "127.0.0.1");
    });
    const port = (httpServer.address() as AddressInfo).port;
    const ws = new WebSocket(`ws://127.0.0.1:${port}`);
    await once(ws, "open");
    const pong = once(ws, "message");
    ws.send(JSON.stringify({ type: "ping" }));
    const [raw] = await pong;
    expect(JSON.parse(String(raw)).type).toBe("pong");
    ws.terminate();
    await shutdown(httpServer, wss);
  });

  it("rejects second hello on same connection", async () => {
    const httpServer = createTestHttpServer();
    const wss = new WebSocketServer({
      server: httpServer,
      ...createWebSocketServerOptions(),
    });
    attachSignalingServer(wss);
    await new Promise<void>((resolve, reject) => {
      httpServer.once("listening", resolve);
      httpServer.once("error", reject);
      httpServer.listen(0, "127.0.0.1");
    });
    const port = (httpServer.address() as AddressInfo).port;
    const ws = new WebSocket(`ws://127.0.0.1:${port}`);
    await once(ws, "open");
    const hello = (role: "baby" | "parent") =>
      JSON.stringify({
        type: "hello",
        role,
        roomId: "r1",
        protocolVersion: PROTOCOL_VERSION,
      });
    const incoming: string[] = [];
    ws.on("message", (raw) => incoming.push(String(raw)));
    ws.send(hello("baby"));
    await vi.waitUntil(() => incoming.length >= 3, { timeout: 3000 });
    expect(JSON.parse(incoming[0]!).type).toBe("welcome");
    expect(JSON.parse(incoming[1]!).type).toBe("members");
    expect(JSON.parse(incoming[2]!).type).toBe("peer");
    ws.send(hello("parent"));
    await vi.waitUntil(
      () =>
        incoming.some((s) => {
          try {
            return (JSON.parse(s) as { type: string }).type === "error";
          } catch {
            return false;
          }
        }),
      { timeout: 3000 },
    );
    const errLine = incoming.find((s) => {
      try {
        return (JSON.parse(s) as { type: string }).type === "error";
      } catch {
        return false;
      }
    });
    const body = JSON.parse(errLine!) as { type: string; code?: string };
    expect(body.type).toBe("error");
    expect(body.code).toBe("INVALID_MESSAGE");
    ws.close();
    await shutdown(httpServer, wss);
  });

  it("rejects a second baby in the same room", async () => {
    const httpServer = createTestHttpServer();
    const wss = new WebSocketServer({
      server: httpServer,
      ...createWebSocketServerOptions(),
    });
    attachSignalingServer(wss);
    await new Promise<void>((resolve, reject) => {
      httpServer.once("listening", resolve);
      httpServer.once("error", reject);
      httpServer.listen(0, "127.0.0.1");
    });
    const port = (httpServer.address() as AddressInfo).port;
    const helloBaby = JSON.stringify({
      type: "hello",
      role: "baby",
      roomId: "r-baby-twice",
      protocolVersion: PROTOCOL_VERSION,
    });
    const a = new WebSocket(`ws://127.0.0.1:${port}`);
    const b = new WebSocket(`ws://127.0.0.1:${port}`);
    await once(a, "open");
    await once(b, "open");
    a.send(helloBaby);
    const welcome = once(a, "message");
    await welcome;
    b.send(helloBaby);
    const denied = once(b, "message");
    const [raw] = await denied;
    const msg = JSON.parse(String(raw)) as { type: string; code?: string };
    expect(msg.type).toBe("error");
    expect(msg.code).toBe("BABY_TAKEN");
    a.close();
    b.close();
    await shutdown(httpServer, wss);
  });

  it("requires signal.to when more than two peers", async () => {
    const httpServer = createTestHttpServer();
    const wss = new WebSocketServer({
      server: httpServer,
      ...createWebSocketServerOptions(),
    });
    attachSignalingServer(wss);
    await new Promise<void>((resolve, reject) => {
      httpServer.once("listening", resolve);
      httpServer.once("error", reject);
      httpServer.listen(0, "127.0.0.1");
    });
    const port = (httpServer.address() as AddressInfo).port;
    const base = `ws://127.0.0.1:${port}`;
    const roomId = "r-three";

    const collect = (ws: WebSocket, out: string[]) => {
      ws.on("message", (raw) => out.push(String(raw)));
    };

    const baby = new WebSocket(base);
    const p1 = new WebSocket(base);
    const p2 = new WebSocket(base);
    await once(baby, "open");
    await once(p1, "open");
    await once(p2, "open");

    const babyIn: string[] = [];
    const p1In: string[] = [];
    const p2In: string[] = [];
    collect(baby, babyIn);
    collect(p1, p1In);
    collect(p2, p2In);

    baby.send(
      JSON.stringify({
        type: "hello",
        role: "baby",
        roomId,
        protocolVersion: PROTOCOL_VERSION,
      }),
    );
    await vi.waitUntil(() => babyIn.length >= 3, { timeout: 3000 });
    expect(JSON.parse(babyIn[0]!).type).toBe("welcome");
    expect(JSON.parse(babyIn[1]!).type).toBe("members");
    expect(JSON.parse(babyIn[2]!).type).toBe("peer");

    p1.send(
      JSON.stringify({
        type: "hello",
        role: "parent",
        roomId,
        protocolVersion: PROTOCOL_VERSION,
      }),
    );
    await vi.waitUntil(() => p1In.length >= 3 && babyIn.length >= 5, { timeout: 3000 });
    expect(JSON.parse(p1In[0]!).type).toBe("welcome");
    expect(JSON.parse(p1In[1]!).type).toBe("members");
    expect(JSON.parse(p1In[2]!).type).toBe("peer");
    expect(JSON.parse(babyIn[3]!).type).toBe("members");
    expect(JSON.parse(babyIn[4]!).type).toBe("peer");

    p2.send(
      JSON.stringify({
        type: "hello",
        role: "parent",
        roomId,
        protocolVersion: PROTOCOL_VERSION,
      }),
    );
    await vi.waitUntil(() => p2In.length >= 3 && babyIn.length >= 7 && p1In.length >= 5, {
      timeout: 3000,
    });
    expect(JSON.parse(p2In[0]!).type).toBe("welcome");
    expect(JSON.parse(p2In[1]!).type).toBe("members");
    expect(JSON.parse(p2In[2]!).type).toBe("peer");
    expect(JSON.parse(babyIn[5]!).type).toBe("members");
    expect(JSON.parse(babyIn[6]!).type).toBe("peer");
    expect(JSON.parse(p1In[3]!).type).toBe("members");
    expect(JSON.parse(p1In[4]!).type).toBe("peer");

    baby.send(
      JSON.stringify({
        type: "signal",
        roomId,
        payload: { n: 1 },
      }),
    );
    await vi.waitUntil(() => babyIn.length >= 8, { timeout: 3000 });
    const errBody = JSON.parse(babyIn[7]!) as { type: string; code?: string };
    expect(errBody.type).toBe("error");
    expect(errBody.code).toBe("SIGNAL_TARGET_REQUIRED");

    baby.close();
    p1.close();
    p2.close();
    await shutdown(httpServer, wss);
  });
});
