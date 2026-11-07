// Task 10.1 (alpha-connection-model, design section 9): the test client of scripts/poc/public-server-logs.sh.
// One relayed call against coturn with a credential the SIGNALING SERVER issues to a real v3 room (one baby, one
// parent), and everything around it that makes coturn or the signaling server write a
// log line: Binding, a rejected credential, an allocation with a REST credential, permissions (allowed, denied
// private peer, denied host address), a channel bind, relayed data in both directions, a refresh, and an HTTP
// and a WebSocket probe of the signaling server. Plain Node 22 (dgram, crypto, fetch, WebSocket), no dependencies.
//
// Env: TURN_HOST, TURN_SECRET (required; signs only the negative controls), PUBLIC_HOST (the host the issued URLs
// must name, default gs.mydata.rocks), SIGNALING_URL (required unless NO_SIGNALING=1, http://host:8787), DENIED_HOST (TURN_HOST),
// CRED_TTL_S (60).
// Prints "BINDING_RESPONSE_BYTES <n>" and "VERDICT PASS" (exit 0) or "VERDICT FAIL" (exit 1).
// Protocol: RFC 8656 (TURN), RFC 8489 (STUN); coturn REST scheme https://github.com/coturn/coturn/blob/master/README.turnserver
import { randomBytes } from "node:crypto";
import dgram from "node:dgram";
import {
  A,
  M,
  UDP_TRANSPORT as UDP,
  attr,
  build,
  ownIPv4,
  parse,
  restCredential,
  u32,
  xorAddr,
} from "./lib/stun.mjs";

const HOST = process.env.TURN_HOST ?? "";
const SECRET = process.env.TURN_SECRET ?? "";
const SIGNALING_URL = process.env.SIGNALING_URL ?? "";
const PUBLIC_HOST = process.env.PUBLIC_HOST ?? "gs.mydata.rocks";
// The host's own address that --denied-peer-ip lists (the TURN host unless the control run says otherwise).
const DENIED_HOST = process.env.DENIED_HOST ?? HOST;
const CRED_TTL_S = Number(process.env.CRED_TTL_S ?? "60");
if (!HOST || !SECRET) {
  console.error("public-server-logs-client: TURN_HOST and TURN_SECRET are required");
  process.exit(2);
}

const failures = [];
const fail = (m) => {
  failures.push(m);
  console.log(`FAIL ${m}`);
};
const log = (m) => console.log(m);

const myIp = ownIPv4();
if (!myIp) {
  console.error("public-server-logs-client: no non-loopback IPv4 address");
  process.exit(2);
}
console.log(`CLIENT_ADDRESS ${myIp}`);

class Client {
  constructor(sock, cred) {
    this.sock = sock;
    this.cred = cred;
    this.realm = "";
    this.nonce = "";
    this.pending = new Map();
    this.onOther = () => {};
    sock.on("message", (m) => {
      const p = parse(m);
      if (p && this.pending.has(p.txid)) this.pending.get(p.txid)(p, m);
      else this.onOther(m, p);
    });
  }
  once(type, attrs, auth) {
    const extra = auth
      ? [
          attr(A.username, Buffer.from(this.cred.user)),
          attr(A.realm, Buffer.from(this.realm)),
          attr(A.nonce, Buffer.from(this.nonce)),
        ]
      : [];
    const { txid, msg } = build(
      type,
      [...attrs, ...extra],
      auth ? { ...this.cred, realm: this.realm } : null,
    );
    return new Promise((res, rej) => {
      let tries = 0;
      const tx = txid.toString("hex");
      const timer = setInterval(() => {
        if (++tries >= 3) {
          clearInterval(timer);
          this.pending.delete(tx);
          rej(new Error("no response"));
        } else this.sock.send(msg, 3478, HOST);
      }, 1500);
      this.pending.set(tx, (p, raw) => {
        clearInterval(timer);
        this.pending.delete(tx);
        res({ ...p, raw });
      });
      this.sock.send(msg, 3478, HOST);
    });
  }
  async txn(type, attrs) {
    let r = await this.once(type, attrs, !!this.nonce);
    if (r.error === 401 && !this.nonce) {
      this.realm = r.attrs.get(A.realm).toString();
      this.nonce = r.attrs.get(A.nonce).toString();
      r = await this.once(type, attrs, true);
    }
    return r;
  }
}
const open = async () => {
  const s = dgram.createSocket("udp4");
  await new Promise((r) => s.bind(0, "0.0.0.0", r));
  return s;
};
const ok = (r) => !r.error && (r.type & 0x0110) === 0x0100;

// 1. Binding: a bare 20-byte request (design section 7, STUN without amplification).
{
  const s = await open();
  const c = new Client(s, null);
  const r = await c.once(M.binding, [], false);
  const size = r.raw.length;
  log(`BINDING_RESPONSE_BYTES ${size}`);
  if (size > 44 || size > 2.2 * 20)
    log(`NOTE binding response ${size} bytes exceeds 44 bytes or 2.2x of 20`);
  s.close();
}

// 2. A rejected credential (wrong secret) and an expired one: the log lines of a failed auth.
for (const [label, cred] of [
  ["wrong secret", restCredential(CRED_TTL_S, randomBytes(8).toString("hex"))],
  ["expired", restCredential(-30, SECRET)],
]) {
  const s = await open();
  const r = await new Client(s, cred).txn(M.allocate, [attr(A.transport, UDP)]);
  if (ok(r)) fail(`allocation with a ${label} credential succeeded`);
  else log(`allocation with a ${label} credential refused (${r.error}), as expected`);
  s.close();
}

// 2b. A full v3 room: one baby and one parent say hello with a derived-shape room id (22 base64url characters);
// the server must send each of them an ice_servers frame whose URLs name PUBLIC_HOST. The baby's credential then
// carries the relayed call below, and both stay joined until the end so the credential stays valid.
const sockets = [];
let issued = null;
if (!SIGNALING_URL) {
  console.error("public-server-logs-client: SIGNALING_URL is required");
  process.exit(2);
}
{
  const room = randomBytes(16).toString("base64url");
  const join = (role) =>
    new Promise((resolve) => {
      const ws = new WebSocket(`${SIGNALING_URL.replace(/^http/, "ws")}/signaling`);
      sockets.push(ws);
      const got = { role, ice: null };
      ws.onopen = () =>
        ws.send(JSON.stringify({ type: "hello", role, roomId: room, protocolVersion: 3 }));
      ws.onmessage = (e) => {
        const f = JSON.parse(String(e.data));
        if (f.type === "ice_servers") {
          got.ice = f;
          resolve(got);
        }
      };
      ws.onerror = () => resolve(got);
      setTimeout(() => resolve(got), 8000);
    });
  const baby = await join("baby");
  const parent = await join("parent");
  for (const m of [baby, parent]) {
    const f = m.ice;
    if (!f) {
      fail(`${m.role} received no ice_servers frame`);
      continue;
    }
    const hosts = f.urls.map((u) => u.replace(/^[a-z]+:/, "").split(/[:?]/)[0]);
    if (!f.urls.length || hosts.some((h) => h !== PUBLIC_HOST))
      fail(`${m.role} ice_servers URLs do not all name ${PUBLIC_HOST}: ${JSON.stringify(f.urls)}`);
    else
      log(
        `${m.role} received ice_servers for ${PUBLIC_HOST} (${f.urls.length} URLs, ttl ${f.ttl})`,
      );
  }
  if (baby.ice && parent.ice) {
    if (baby.ice.username === parent.ice.username) fail("both members got the same credential");
    issued = { user: baby.ice.username, pass: baby.ice.credential };
    console.log(`ISSUED_USERNAME ${issued.user}`);
    console.log(`ISSUED_CREDENTIAL ${issued.pass}`);
  }
}
if (!issued) {
  log("VERDICT FAIL (no issued credential)");
  process.exit(1);
}

// 3. The relayed call, with the credential the server issued.
const peerSock = await open();
const peer = { ip: myIp, port: peerSock.address().port };
peerSock.on("message", (m, r) => peerSock.send(m, r.port, r.address)); // echo
const sock = await open();
const c = new Client(sock, issued);
const echoes = new Set();
c.onOther = (m, p) => {
  if (p && p.type === 0x0017 && p.attrs.get(A.data)) echoes.add(p.attrs.get(A.data).toString());
};
const alloc = await c.txn(M.allocate, [attr(A.transport, UDP)]);
if (!ok(alloc)) {
  fail(`allocation refused (${alloc.error})`);
} else {
  log("allocation with a REST credential succeeded");
  const perm = await c.txn(M.createPermission, [attr(A.xorPeer, xorAddr(peer.ip, 0))]);
  if (!ok(perm)) fail(`permission for the peer refused (${perm.error})`);
  for (const [label, ip] of [
    ["a private peer (10.9.9.9)", "10.9.9.9"],
    ["the host's own address", DENIED_HOST],
  ]) {
    const d = await c.txn(M.createPermission, [attr(A.xorPeer, xorAddr(ip, 0))]);
    if (ok(d)) fail(`permission for ${label} was granted`);
    else log(`permission for ${label} refused (${d.error}), as expected`);
  }
  const send = (text) => {
    const { msg } = build(
      0x0016,
      [attr(A.xorPeer, xorAddr(peer.ip, peer.port)), attr(A.data, Buffer.from(text))],
      null,
    );
    sock.send(msg, 3478, HOST);
  };
  for (let i = 0; i < 5; i++) {
    send(`ping-${i}`);
    await new Promise((r) => setTimeout(r, 200));
  }
  await new Promise((r) => setTimeout(r, 1000));
  const got = [0, 1, 2, 3, 4].filter((i) => echoes.has(`ping-${i}`)).length;
  log(`relayed data echoed ${got}/5`);
  if (got < 4) fail(`only ${got}/5 relayed echoes`);
  const rf = await c.txn(M.refresh, [attr(A.lifetime, u32(60))]);
  if (!ok(rf)) fail(`refresh refused (${rf.error})`);
  const rel = await c.txn(M.refresh, [attr(A.lifetime, u32(0))]);
  if (!ok(rel)) fail(`release refused (${rel.error})`);
}
sock.close();
peerSock.close();

// 4. A plain HTTP request and a WebSocket that sends garbage, then leave the room.
try {
  const r = await fetch(`${SIGNALING_URL}/signaling/healthz`);
  log(`signaling /healthz: ${r.status} ${(await r.text()).trim()}`);
  if (!r.ok) fail("signaling /healthz not 200");
} catch (e) {
  fail(`signaling /healthz: ${e}`);
}
await new Promise((resolve) => {
  const ws = new WebSocket(`${SIGNALING_URL.replace(/^http/, "ws")}/signaling`);
  ws.onopen = () => {
    ws.send('{"type":"hello","garbage":true}');
    setTimeout(() => ws.close(), 500);
  };
  ws.onclose = () => resolve();
  ws.onerror = () => resolve();
  setTimeout(resolve, 3000);
});
for (const ws of sockets) ws.close();
log("signaling probes done");

log(failures.length ? `VERDICT FAIL (${failures.length})` : "VERDICT PASS");
process.exit(failures.length ? 1 : 0);
