import { randomUUID } from "node:crypto";
import type { IncomingMessage } from "node:http";
import type { RawData } from "ws";
import { WebSocketServer } from "ws";
import {
  SUPPORTED_PROTOCOL_VERSIONS,
  parseClientMessage,
  stringifyServerMessage,
  type ServerToClientMessage,
} from "@guardedsleep/protocol";
import {
  getHelloRatePerMin,
  getHelloTimeoutMs,
  getMaxParentsPerRoom,
  getNewRoomRatePerMin,
  getPingIntervalMs,
  getTrustProxy,
  getTrustedProxyHops,
} from "./config.js";
import { RateLimiter } from "./rate-limit.js";
import { RoomRegistry, type RoomPeer, type SignalingRoom } from "./room.js";

function send(ws: import("ws").WebSocket, msg: ServerToClientMessage): void {
  if (ws.readyState === ws.OPEN) ws.send(stringifyServerMessage(msg));
}

function broadcastRoomMembers(room: SignalingRoom): void {
  const msg: ServerToClientMessage = {
    type: "members",
    peers: room.listMembers(),
  };
  for (const p of room.allPeers()) {
    send(p.socket, msg);
  }
}

function rawToString(data: RawData): string {
  if (typeof data === "string") return data;
  if (Buffer.isBuffer(data)) return data.toString("utf8");
  if (data instanceof ArrayBuffer) return Buffer.from(data).toString("utf8");
  if (Array.isArray(data)) return Buffer.concat(data).toString("utf8");
  const v = data as ArrayBufferView;
  return Buffer.from(v.buffer, v.byteOffset, v.byteLength).toString("utf8");
}

/** Room ids of v3 rooms are 128-bit HKDF output, base64url without padding (ADR-004 amendment). */
const V3_ROOM_ID = /^[A-Za-z0-9_-]{22}$/;

/**
 * Proxies append the address they saw to X-Forwarded-For, so only the rightmost `hops` entries are
 * trustworthy; leading entries are whatever the client sent.
 */
function clientAddress(req: IncomingMessage, trustProxy: boolean, hops: number): string {
  if (trustProxy) {
    const xff = req.headers["x-forwarded-for"];
    const entries = (Array.isArray(xff) ? xff.join(",") : (xff ?? ""))
      .split(",")
      .map((e) => e.trim())
      .filter(Boolean);
    const hop = entries[entries.length - hops];
    if (hop) return hop;
  }
  return req.socket.remoteAddress ?? "unknown";
}

export type SignalingServerOptions = {
  pingIntervalMs?: number;
  helloRatePerMin?: number;
  newRoomRatePerMin?: number;
  trustProxy?: boolean;
  trustedProxyHops?: number;
};

export function attachSignalingServer(
  wss: WebSocketServer,
  registry = new RoomRegistry(getMaxParentsPerRoom()),
  options: SignalingServerOptions = {},
): RoomRegistry {
  const trustProxy = options.trustProxy ?? getTrustProxy();
  const trustedProxyHops = options.trustedProxyHops ?? getTrustedProxyHops();
  const helloLimiter = new RateLimiter(options.helloRatePerMin ?? getHelloRatePerMin());
  const roomLimiter = new RateLimiter(options.newRoomRatePerMin ?? getNewRoomRatePerMin());

  // A socket that did not answer the previous ping is dead (half-open TCP, NAT drop); terminating
  // it runs the normal close -> cleanup -> members broadcast path.
  const alive = new WeakMap<object, boolean>();
  const livenessTimer = setInterval(() => {
    for (const client of wss.clients) {
      if (alive.get(client) === false) {
        client.terminate();
        continue;
      }
      alive.set(client, false);
      try {
        client.ping();
      } catch {
        client.terminate();
      }
    }
    helloLimiter.prune();
    roomLimiter.prune();
  }, options.pingIntervalMs ?? getPingIntervalMs());
  livenessTimer.unref();
  wss.on("close", () => clearInterval(livenessTimer));

  wss.on("connection", (ws, req) => {
    const address = clientAddress(req, trustProxy, trustedProxyHops);
    alive.set(ws, true);
    ws.on("pong", () => alive.set(ws, true));
    let joined: { roomId: string; clientId: string } | undefined;
    let cleanedUp = false;

    const helloMs = getHelloTimeoutMs();
    const helloTimer = setTimeout(() => {
      if (!joined && ws.readyState === ws.OPEN) {
        ws.close(1008, "hello timeout");
      }
    }, helloMs);

    const clearHelloTimer = (): void => {
      clearTimeout(helloTimer);
    };

    const cleanup = (): void => {
      if (cleanedUp) return;
      cleanedUp = true;
      clearHelloTimer();
      if (!joined) return;
      const room = registry.getOrCreate(joined.roomId);
      room.remove(joined.clientId);
      if (room.size > 0) {
        broadcastRoomMembers(room);
      }
      registry.deleteIfEmpty(joined.roomId);
    };

    ws.on("message", (data) => {
      const raw = rawToString(data);
      const msg = parseClientMessage(raw);
      if (!msg) {
        send(ws, { type: "error", code: "BAD_JSON", message: "invalid message" });
        return;
      }

      if (msg.type === "ping") {
        send(ws, { type: "pong" });
        return;
      }

      if (msg.type === "hello") {
        if (joined) {
          send(ws, { type: "error", code: "INVALID_MESSAGE", message: "already joined" });
          return;
        }
        if (!helloLimiter.take(address)) {
          send(ws, { type: "error", code: "RATE_LIMITED", message: "too many attempts" });
          ws.close(1008, "rate limited");
          return;
        }
        if (!SUPPORTED_PROTOCOL_VERSIONS.includes(msg.protocolVersion)) {
          send(ws, { type: "error", code: "UNSUPPORTED_PROTOCOL", message: "version mismatch" });
          ws.close(1008, "version mismatch");
          return;
        }
        if (msg.protocolVersion >= 3 && !V3_ROOM_ID.test(msg.roomId)) {
          send(ws, { type: "error", code: "INVALID_MESSAGE", message: "invalid room id" });
          ws.close(1008, "invalid room id");
          return;
        }
        const existing = registry.peek(msg.roomId);
        if (existing && existing.size > 0) {
          if (existing.protocolVersion !== msg.protocolVersion) {
            send(ws, { type: "error", code: "UNSUPPORTED_PROTOCOL", message: "version mismatch" });
            ws.close(1008, "version mismatch");
            return;
          }
        } else if (!roomLimiter.take(address)) {
          send(ws, { type: "error", code: "RATE_LIMITED", message: "too many rooms" });
          ws.close(1008, "rate limited");
          return;
        }
        const room = registry.getOrCreate(msg.roomId);
        const firstMember = room.size === 0;
        const clientId = randomUUID();
        const peer: RoomPeer = { id: clientId, role: msg.role, socket: ws };
        const status = room.add(peer);
        if (status === "baby_taken") {
          send(ws, {
            type: "error",
            code: "BABY_TAKEN",
            message: "room already has a baby unit",
          });
          ws.close(1008, "baby taken");
          registry.deleteIfEmpty(msg.roomId);
          return;
        }
        if (status === "parent_cap") {
          send(ws, { type: "error", code: "PARENT_CAP", message: "too many parent units" });
          ws.close(1008, "parent cap");
          registry.deleteIfEmpty(msg.roomId);
          return;
        }
        if (firstMember) room.protocolVersion = msg.protocolVersion;
        joined = { roomId: msg.roomId, clientId };
        clearHelloTimer();
        send(ws, { type: "welcome", clientId, roomId: msg.roomId });
        broadcastRoomMembers(room);
        for (const o of room.allPeers()) {
          if (o.id !== clientId) {
            send(o.socket, { type: "peer", peerId: clientId });
          }
        }
        const firstOther = room.getPeer(clientId);
        send(ws, { type: "peer", peerId: firstOther ? firstOther.id : null });
        return;
      }

      if (!joined) {
        send(ws, { type: "error", code: "NOT_JOINED", message: "send hello first" });
        return;
      }

      if (msg.type === "signal") {
        if (msg.roomId !== joined.roomId) {
          send(ws, { type: "error", code: "INVALID_MESSAGE", message: "room mismatch" });
          return;
        }
        const room = registry.getOrCreate(joined.roomId);
        let target: RoomPeer | undefined;
        const to = msg.to;
        if (to !== undefined && to !== "") {
          target = room.getPeerById(to);
          if (!target || target.id === joined.clientId) {
            send(ws, { type: "error", code: "INVALID_MESSAGE", message: "unknown signal target" });
            return;
          }
        } else if (room.size <= 2) {
          target = room.getPeer(joined.clientId);
        } else {
          send(ws, {
            type: "error",
            code: "SIGNAL_TARGET_REQUIRED",
            message: "set signal.to when more than two peers are in the room",
          });
          return;
        }
        if (!target) return;
        send(target.socket, { type: "signal", from: joined.clientId, payload: msg.payload });
      }
    });

    ws.on("error", cleanup);
    ws.on("close", cleanup);
  });

  return registry;
}
