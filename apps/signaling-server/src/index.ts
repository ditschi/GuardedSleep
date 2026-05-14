import { createServer } from "node:http";
import { WebSocketServer } from "ws";
import { assertProductionOriginAllowlist } from "./config.js";
import { attachSignalingServer } from "./server.js";
import { createWebSocketServerOptions } from "./ws-options.js";

assertProductionOriginAllowlist();

const port = Number(process.env.PORT ?? "8787");

const server = createServer((_req, res) => {
  res.writeHead(200, {
    "content-type": "text/plain; charset=utf-8",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY",
    "cache-control": "no-store",
  });
  res.end("GuardedSleep signaling\n");
});

const wss = new WebSocketServer({
  server,
  ...createWebSocketServerOptions(),
});
attachSignalingServer(wss);

server.listen(port, () => {
  console.log(`signaling-server listening on port ${port}`);
});
