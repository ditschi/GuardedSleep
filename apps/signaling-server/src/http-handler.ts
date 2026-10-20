import type { IncomingMessage, ServerResponse } from "node:http";

const SECURITY_HEADERS = {
  "content-type": "text/plain; charset=utf-8",
  "x-content-type-options": "nosniff",
  "x-frame-options": "DENY",
  "cache-control": "no-store",
} as const;

/**
 * Plain HTTP responses for non-upgrade requests. WebSocket upgrades are accepted on any path, so a
 * reverse proxy can route e.g. `/signaling` here without stripping the prefix.
 */
export function handleHttpRequest(req: IncomingMessage, res: ServerResponse): void {
  const path = (req.url ?? "/").split("?")[0];
  if (path === "/healthz" || path.endsWith("/healthz")) {
    res.writeHead(200, SECURITY_HEADERS);
    res.end("ok\n");
    return;
  }
  res.writeHead(200, SECURITY_HEADERS);
  res.end("GuardedSleep signaling\n");
}
