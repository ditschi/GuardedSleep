import { createServer } from "node:http";
import { WebSocketServer } from "ws";
import { assertProductionOriginAllowlist } from "./config.js";
import { handleHttpRequest } from "./http-handler.js";
import { attachSignalingServer } from "./server.js";
import { createWebSocketServerOptions } from "./ws-options.js";

assertProductionOriginAllowlist();

const port = Number(process.env.PORT ?? "8787");

const server = createServer(handleHttpRequest);

const wss = new WebSocketServer({
  server,
  ...createWebSocketServerOptions(),
});
attachSignalingServer(wss);

server.listen(port, () => {
  console.log(`signaling-server listening on port ${port}`);
});
