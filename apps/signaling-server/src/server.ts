import { randomUUID } from "node:crypto";
import type { RawData } from "ws";
import { WebSocketServer } from "ws";
import {
  PROTOCOL_VERSION,
  parseClientMessage,
  stringifyServerMessage,
  type ServerToClientMessage,
} from "@guardedsleep/protocol";
import { getHelloTimeoutMs, getMaxParentsPerRoom } from "./config.js";
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

export function attachSignalingServer(
  wss: WebSocketServer,
  registry = new RoomRegistry(getMaxParentsPerRoom()),
): RoomRegistry {
  wss.on("connection", (ws) => {
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
        if (msg.protocolVersion !== PROTOCOL_VERSION) {
          send(ws, { type: "error", code: "UNSUPPORTED_PROTOCOL", message: "version mismatch" });
          ws.close(1008, "version mismatch");
          return;
        }
        const room = registry.getOrCreate(msg.roomId);
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
          return;
        }
        if (status === "parent_cap") {
          send(ws, { type: "error", code: "PARENT_CAP", message: "too many parent units" });
          ws.close(1008, "parent cap");
          return;
        }
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
