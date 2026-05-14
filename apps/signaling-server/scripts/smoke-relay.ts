/**
 * End-to-end smoke: starts the built server in a child process, then verifies
 * hello + signal relay over WebSocket.
 *
 * Usage:
 *   pnpm --filter @guardedsleep/signaling-server smoke
 */
import { spawn, type ChildProcess } from "node:child_process";
import { once } from "node:events";
import { createServer as createNetServer } from "node:net";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { PROTOCOL_VERSION } from "@guardedsleep/protocol";
import WebSocket from "ws";

const ROOM = "smoke-room";

function readOne(ws: InstanceType<typeof WebSocket>): Promise<string> {
  return new Promise((resolve, reject) => {
    ws.once("message", (d) => resolve(String(d)));
    ws.once("error", reject);
  });
}

function pickFreePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const s = createNetServer();
    s.once("error", reject);
    s.listen(0, "127.0.0.1", () => {
      const addr = s.address();
      if (!addr || typeof addr === "string") {
        s.close();
        reject(new Error("no port"));
        return;
      }
      const p = addr.port;
      s.close(() => resolve(p));
    });
  });
}

async function main(): Promise<void> {
  const rootDir = join(dirname(fileURLToPath(import.meta.url)), "..");
  const entry = join(rootDir, "dist", "index.js");

  const port = await pickFreePort();

  const childEnv = { ...process.env, PORT: String(port) };
  if (childEnv.NODE_ENV === "production") {
    childEnv.NODE_ENV = "development";
  }

  const child: ChildProcess = spawn(process.execPath, [entry], {
    cwd: rootDir,
    env: childEnv,
    // Do not pipe stdout/stderr: an unread pipe can fill and block the child on console.log.
    stdio: ["ignore", "ignore", "ignore"],
  });

  await new Promise((r) => setTimeout(r, 400));

  const base = `ws://127.0.0.1:${port}`;

  try {
    const a = new WebSocket(base);
    await once(a, "open");
    const p0 = readOne(a);
    a.send(
      JSON.stringify({
        type: "hello",
        role: "baby",
        roomId: ROOM,
        protocolVersion: PROTOCOL_VERSION,
      }),
    );
    const wa = JSON.parse(await p0) as { type: string; clientId?: string };
    if (wa.type !== "welcome" || !wa.clientId) throw new Error(`welcome: ${JSON.stringify(wa)}`);
    const m1 = JSON.parse(await readOne(a)) as { type: string };
    const m2 = JSON.parse(await readOne(a)) as { type: string };
    if (m1.type !== "members") throw new Error(`expected members, got ${m1.type}`);
    if (m2.type !== "peer") throw new Error(`expected peer, got ${m2.type}`);

    const b = new WebSocket(base);
    await once(b, "open");
    const pBWelcome = readOne(b);
    const pBMembers = readOne(b);
    const pBPeer = readOne(b);
    const pAMembers = readOne(a);
    const pAPeer = readOne(a);
    b.send(
      JSON.stringify({
        type: "hello",
        role: "parent",
        roomId: ROOM,
        protocolVersion: PROTOCOL_VERSION,
      }),
    );
    const wb = JSON.parse(await pBWelcome) as { type: string; clientId?: string };
    if (wb.type !== "welcome" || !wb.clientId) throw new Error(`welcome b: ${JSON.stringify(wb)}`);
    const bMem = JSON.parse(await pBMembers) as { type: string };
    if (bMem.type !== "members") throw new Error(`b members: ${JSON.stringify(bMem)}`);
    const bPeer = JSON.parse(await pBPeer) as { type: string; peerId?: string | null };
    if (bPeer.type !== "peer" || bPeer.peerId !== wa.clientId) {
      throw new Error(`b peer: ${JSON.stringify(bPeer)}`);
    }
    const aMem2 = JSON.parse(await pAMembers) as { type: string; peers?: { clientId: string }[] };
    if (aMem2.type !== "members" || !aMem2.peers?.some((p) => p.clientId === wb.clientId)) {
      throw new Error(`a members2: ${JSON.stringify(aMem2)}`);
    }
    const aPeer2 = JSON.parse(await pAPeer) as { type: string; peerId?: string | null };
    if (aPeer2.type !== "peer" || aPeer2.peerId !== wb.clientId) {
      throw new Error(`a peer2: ${JSON.stringify(aPeer2)}`);
    }

    const relayP = readOne(b);
    a.send(
      JSON.stringify({
        type: "signal",
        roomId: ROOM,
        payload: { smoke: true },
      }),
    );
    const relayed = JSON.parse(await relayP) as { type: string; from?: string };
    if (relayed.type !== "signal" || relayed.from !== wa.clientId) {
      throw new Error(`relay: ${JSON.stringify(relayed)}`);
    }

    a.close();
    b.close();
  } finally {
    child.kill("SIGTERM");
    await once(child, "close").catch(() => {});
  }

  console.log("smoke-relay: ok");
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
