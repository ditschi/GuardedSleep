// STUN Binding probe (RFC 5389) for every server of data/stun-servers.json.
//   node scripts/spike/stun-probe.mjs [--data data] [--tries 3] [--timeout-ms 2000]
// One UDP datagram of 20 bytes per try. The answer's mapped address (this machine's public
// address) is checked but never printed or stored.
import { randomBytes } from "node:crypto";
import dgram from "node:dgram";
import dns from "node:dns/promises";
import { pathToFileURL } from "node:url";
import { parseArgs, readData } from "./lib.mjs";

const MAGIC = 0x2112a442;

function bindingRequest() {
  const msg = Buffer.alloc(20);
  msg.writeUInt16BE(0x0001, 0); // Binding request
  msg.writeUInt16BE(0, 2); // no attributes
  msg.writeUInt32BE(MAGIC, 4);
  randomBytes(12).copy(msg, 8);
  return msg;
}

/** True when `reply` is a Binding success response to `request` with a mapped address. */
export function isBindingSuccess(request, reply) {
  if (reply.length < 20 || reply.readUInt16BE(0) !== 0x0101) return false;
  if (reply.readUInt32BE(4) !== MAGIC) return false;
  if (!reply.subarray(8, 20).equals(request.subarray(8, 20))) return false;
  const end = Math.min(reply.length, 20 + reply.readUInt16BE(2));
  for (let at = 20; at + 4 <= end;) {
    const type = reply.readUInt16BE(at);
    const length = reply.readUInt16BE(at + 2);
    if (type === 0x0020 || type === 0x0001) return true; // XOR-MAPPED-ADDRESS or MAPPED-ADDRESS
    at += 4 + length + ((4 - (length % 4)) % 4);
  }
  return false;
}

function attempt(address, family, port, timeoutMs) {
  return new Promise((resolve) => {
    const socket = dgram.createSocket(family === 6 ? "udp6" : "udp4");
    const request = bindingRequest();
    const started = performance.now();
    let finished = false;
    const finish = (result) => {
      if (finished) return;
      finished = true;
      clearTimeout(timer);
      socket.close();
      resolve(result);
    };
    const timer = setTimeout(() => finish({ ok: false, reason: "timeout" }), timeoutMs);
    socket.on("error", () => finish({ ok: false, reason: "socket" }));
    socket.on("message", (reply) => {
      if (isBindingSuccess(request, reply)) {
        finish({ ok: true, rtt_ms: Math.round(performance.now() - started) });
      }
    });
    socket.send(request, port, address, (error) => {
      if (error) finish({ ok: false, reason: "send" });
    });
  });
}

/** One server: up to `tries` Binding requests over IPv4; ok on the first answer. */
export async function probeStun(server, { tries = 3, timeoutMs = 2_000 } = {}) {
  let address;
  try {
    ({ address } = await dns.lookup(server.host, { family: 4 }));
  } catch {
    return { host: server.host, port: server.port, ok: false, reason: "dns" };
  }
  let last = { ok: false, reason: "timeout" };
  for (let i = 0; i < tries; i++) {
    last = await attempt(address, 4, server.port, timeoutMs);
    if (last.ok) break;
  }
  return { host: server.host, port: server.port, ...last };
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const args = parseArgs(process.argv.slice(2), {
    data: { default: "data" },
    tries: { default: 3, number: true },
    "timeout-ms": { default: 2_000, number: true },
  });
  const servers = readData(args.data, "stun-servers.json").servers.filter(
    (s) => s.status === "active",
  );
  const results = await Promise.all(
    servers.map((s) => probeStun(s, { tries: args.tries, timeoutMs: args["timeout-ms"] })),
  );
  for (const r of results) console.log(JSON.stringify(r));
  if (results.some((r) => !r.ok)) process.exitCode = 1;
}
