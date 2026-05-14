import type { IncomingMessage } from "node:http";
import type { WebSocketServer } from "ws";

/** Default max WebSocket message size (signaling JSON only; mitigates memory DoS). */
export const WS_MAX_PAYLOAD_BYTES = 64 * 1024;

function parseAllowedOrigins(): string[] | null {
  const raw = process.env.SIGNALING_ALLOWED_ORIGINS?.trim();
  if (!raw) return null;
  const list = raw
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean);
  return list.length > 0 ? list : null;
}

/**
 * Optional origin allowlist for browser clients. Native / Node `ws` clients often send no
 * `Origin` header — those connections are allowed when this is set (pairing apps, smoke tests).
 */
export function verifyWebSocketClient(
  info: { origin: string; secure: boolean; req: IncomingMessage },
  callback: (res: boolean, code?: number, message?: string) => void,
): void {
  const allowed = parseAllowedOrigins();
  if (!allowed) {
    callback(true);
    return;
  }
  const origin = info.origin?.trim() ?? "";
  if (origin === "") {
    callback(true);
    return;
  }
  callback(allowed.includes(origin));
}

type WsServerOptions = NonNullable<ConstructorParameters<typeof WebSocketServer>[0]>;

/**
 * WebSocketServer options shared by production `index.ts` and tests.
 */
export function createWebSocketServerOptions(): WsServerOptions {
  return {
    maxPayload: WS_MAX_PAYLOAD_BYTES,
    perMessageDeflate: false,
    verifyClient: verifyWebSocketClient,
  };
}
