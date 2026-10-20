import type {
  ClientToServerMessage,
  ErrorCode,
  ServerToClientMessage,
  UnitRole,
} from "./messages.js";
import { PROTOCOL_VERSION } from "./messages.js";

const ERROR_CODES: ReadonlySet<ErrorCode> = new Set([
  "BAD_JSON",
  "UNSUPPORTED_PROTOCOL",
  "INVALID_MESSAGE",
  "ROOM_FULL",
  "NOT_JOINED",
  "BABY_TAKEN",
  "PARENT_CAP",
  "SIGNAL_TARGET_REQUIRED",
  "RATE_LIMITED",
]);

export function safeJsonParse(raw: string): unknown {
  try {
    return JSON.parse(raw) as unknown;
  } catch {
    return undefined;
  }
}

export function parseClientMessage(raw: string): ClientToServerMessage | undefined {
  const v = safeJsonParse(raw);
  if (!v || typeof v !== "object" || !("type" in v)) return undefined;
  const o = v as Record<string, unknown>;
  switch (o.type) {
    case "ping":
      return { type: "ping" };
    case "hello": {
      const role = o.role;
      const roomId = o.roomId;
      const protocolVersion = o.protocolVersion;
      if (
        (role !== "baby" && role !== "parent") ||
        typeof roomId !== "string" ||
        roomId.length === 0 ||
        roomId.length > 128 ||
        typeof protocolVersion !== "number"
      ) {
        return undefined;
      }
      return {
        type: "hello",
        role,
        roomId,
        protocolVersion,
      };
    }
    case "signal": {
      const roomId = o.roomId;
      if (typeof roomId !== "string" || roomId.length === 0 || roomId.length > 128)
        return undefined;
      if (!("payload" in o)) return undefined;
      const toRaw = o.to;
      if (
        toRaw !== undefined &&
        (typeof toRaw !== "string" || toRaw.length === 0 || toRaw.length > 128)
      )
        return undefined;
      return {
        type: "signal",
        roomId,
        ...(toRaw !== undefined ? { to: toRaw } : {}),
        payload: o.payload,
      };
    }
    default:
      return undefined;
  }
}

export function parseServerMessage(raw: string): ServerToClientMessage | undefined {
  const v = safeJsonParse(raw);
  if (!v || typeof v !== "object" || !("type" in v)) return undefined;
  const o = v as Record<string, unknown>;
  switch (o.type) {
    case "pong":
      return { type: "pong" };
    case "welcome": {
      if (typeof o.clientId !== "string" || typeof o.roomId !== "string") return undefined;
      return { type: "welcome", clientId: o.clientId, roomId: o.roomId };
    }
    case "peer": {
      const peerId = o.peerId;
      if (peerId !== null && typeof peerId !== "string") return undefined;
      return { type: "peer", peerId };
    }
    case "members": {
      const peers = o.peers;
      if (!Array.isArray(peers)) return undefined;
      const out: { clientId: string; role: UnitRole }[] = [];
      for (const item of peers) {
        if (!item || typeof item !== "object") return undefined;
        const rec = item as Record<string, unknown>;
        const role = rec.role;
        if (typeof rec.clientId !== "string" || (role !== "baby" && role !== "parent"))
          return undefined;
        out.push({ clientId: rec.clientId, role });
      }
      return { type: "members", peers: out };
    }
    case "signal": {
      if (typeof o.from !== "string" || !("payload" in o)) return undefined;
      return { type: "signal", from: o.from, payload: o.payload };
    }
    case "error": {
      const code = o.code;
      const message = o.message;
      if (typeof code !== "string" || typeof message !== "string") return undefined;
      if (!ERROR_CODES.has(code as ErrorCode)) return undefined;
      return { type: "error", code: code as ErrorCode, message };
    }
    default:
      return undefined;
  }
}

export function stringifyServerMessage(msg: ServerToClientMessage): string {
  return JSON.stringify(msg);
}

export function stringifyClientMessage(msg: ClientToServerMessage): string {
  return JSON.stringify(msg);
}

export function assertCompatibleProtocol(clientVersion: number): boolean {
  return clientVersion === PROTOCOL_VERSION;
}
