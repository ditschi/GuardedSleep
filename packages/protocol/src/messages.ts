/** Wire format version for signaling + DataChannel JSON payloads. */
export const PROTOCOL_VERSION = 2 as const;

export type UnitRole = "baby" | "parent";

export type ClientToServerMessage =
  | {
      type: "hello";
      role: UnitRole;
      roomId: string;
      protocolVersion: number;
    }
  | {
      type: "signal";
      roomId: string;
      /** Recipient when more than two peers are in the room (required then). */
      to?: string;
      /** Opaque WebRTC signaling payload (offer/answer/candidate JSON). */
      payload: unknown;
    }
  | { type: "ping" };

export type ServerToClientMessage =
  | { type: "welcome"; clientId: string; roomId: string }
  | { type: "peer"; peerId: string | null }
  | {
      type: "members";
      peers: { clientId: string; role: UnitRole }[];
    }
  | {
      type: "signal";
      from: string;
      payload: unknown;
    }
  | { type: "error"; code: ErrorCode; message: string }
  | { type: "pong" };

export type ErrorCode =
  | "BAD_JSON"
  | "UNSUPPORTED_PROTOCOL"
  | "INVALID_MESSAGE"
  | "ROOM_FULL"
  | "NOT_JOINED"
  | "BABY_TAKEN"
  | "PARENT_CAP"
  | "SIGNAL_TARGET_REQUIRED";
