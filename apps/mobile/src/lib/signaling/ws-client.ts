import {
  PROTOCOL_VERSION,
  parseServerMessage,
  stringifyClientMessage,
  type UnitRole,
} from "@guardedsleep/protocol";

export type MemberSnapshot = { clientId: string; role: UnitRole };

export type SignalingHandlers = {
  onWelcome: (clientId: string, roomId: string) => void;
  onPeer: (peerId: string | null) => void;
  /** Full room roster after each join/leave (protocol v2+). */
  onMembers?: (peers: MemberSnapshot[]) => void;
  onSignal: (from: string, payload: unknown) => void;
  onError: (code: string, message: string) => void;
  /** WebSocket closed or failed — session should treat link as dead. */
  onClose?: () => void;
};

export type SignalingConnection = {
  sendSignal: (payload: unknown, to?: string) => void;
  close: () => void;
};

function decodeWebSocketData(data: unknown): string {
  if (typeof data === "string") return data;
  if (data instanceof ArrayBuffer) return new TextDecoder("utf-8").decode(data);
  if (ArrayBuffer.isView(data)) {
    const v = data as ArrayBufferView;
    return new TextDecoder("utf-8").decode(new Uint8Array(v.buffer, v.byteOffset, v.byteLength));
  }
  return "";
}

export function connectSignaling(
  wsUrl: string,
  roomId: string,
  role: UnitRole,
  handlers: SignalingHandlers,
): SignalingConnection {
  const ws = new WebSocket(wsUrl);
  let joinedRoom: string | undefined;

  ws.addEventListener("open", () => {
    ws.send(
      stringifyClientMessage({
        type: "hello",
        role,
        roomId,
        protocolVersion: PROTOCOL_VERSION,
      }),
    );
  });

  const dispatchServerPayload = (raw: string): void => {
    const msg = parseServerMessage(raw);
    if (!msg) {
      handlers.onError("BAD_JSON", "invalid server message");
      return;
    }
    switch (msg.type) {
      case "welcome":
        joinedRoom = msg.roomId;
        handlers.onWelcome(msg.clientId, msg.roomId);
        break;
      case "peer":
        handlers.onPeer(msg.peerId);
        break;
      case "members":
        handlers.onMembers?.(msg.peers);
        break;
      case "signal":
        handlers.onSignal(msg.from, msg.payload);
        break;
      case "error":
        handlers.onError(msg.code, msg.message);
        break;
      case "pong":
        break;
    }
  };

  ws.addEventListener("message", (ev) => {
    const d = ev.data;
    if (typeof Blob !== "undefined" && d instanceof Blob) {
      void d.text().then(dispatchServerPayload);
      return;
    }
    dispatchServerPayload(decodeWebSocketData(d));
  });

  let socketClosed = false;
  const signalClose = (): void => {
    if (socketClosed) return;
    socketClosed = true;
    joinedRoom = undefined;
    handlers.onClose?.();
  };

  ws.addEventListener("close", signalClose);
  ws.addEventListener("error", signalClose);

  return {
    sendSignal: (payload: unknown, to?: string) => {
      if (!joinedRoom) return;
      ws.send(
        stringifyClientMessage({
          type: "signal",
          roomId: joinedRoom,
          ...(to !== undefined && to !== "" ? { to } : {}),
          payload,
        }),
      );
    },
    close: () => {
      ws.close();
    },
  };
}
