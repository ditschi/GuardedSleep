// Task 10.1 (alpha-connection-model, design section 9): the test client of scripts/poc/public-server-logs.sh.
// One relayed call against coturn, and everything around it that makes coturn or the signaling server write a
// log line: Binding, a rejected credential, an allocation with a REST credential, permissions (allowed, denied
// private peer, denied host address), a channel bind, relayed data in both directions, a refresh, and an HTTP
// and a WebSocket probe of the signaling server. Plain Node 22 (dgram, crypto, fetch, WebSocket), no dependencies.
//
// Env: TURN_HOST, TURN_SECRET (required), SIGNALING_URL (optional, http://host:8787), DENIED_HOST (TURN_HOST),
// CRED_TTL_S (60).
// Prints "BINDING_RESPONSE_BYTES <n>" and "VERDICT PASS" (exit 0) or "VERDICT FAIL" (exit 1).
// Protocol: RFC 8656 (TURN), RFC 8489 (STUN); coturn REST scheme https://github.com/coturn/coturn/blob/master/README.turnserver
import { createHash, createHmac, randomBytes } from "node:crypto";
import dgram from "node:dgram";
import { networkInterfaces } from "node:os";

const HOST = process.env.TURN_HOST ?? "";
const SECRET = process.env.TURN_SECRET ?? "";
const SIGNALING_URL = process.env.SIGNALING_URL ?? "";
// The host's own address that --denied-peer-ip lists (the TURN host unless the control run says otherwise).
const DENIED_HOST = process.env.DENIED_HOST ?? HOST;
const CRED_TTL_S = Number(process.env.CRED_TTL_S ?? "60");
if (!HOST || !SECRET) {
  console.error("public-server-logs-client: TURN_HOST and TURN_SECRET are required");
  process.exit(2);
}

const MAGIC = 0x2112a442;
const M = { binding: 0x001, allocate: 0x003, refresh: 0x004, createPermission: 0x008 };
const A = {
  username: 0x0006,
  integrity: 0x0008,
  errorCode: 0x0009,
  lifetime: 0x000d,
  xorPeer: 0x0012,
  data: 0x0013,
  realm: 0x0014,
  nonce: 0x0015,
  xorRelayed: 0x0016,
  transport: 0x0019,
};
const failures = [];
const fail = (m) => {
  failures.push(m);
  console.log(`FAIL ${m}`);
};
const log = (m) => console.log(m);

const pad4 = (n) => (4 - (n % 4)) % 4;
const attr = (type, value) => {
  const h = Buffer.alloc(4);
  h.writeUInt16BE(type, 0);
  h.writeUInt16BE(value.length, 2);
  return Buffer.concat([h, value, Buffer.alloc(pad4(value.length))]);
};
const xorAddr = (ip, port) => {
  const b = Buffer.alloc(8);
  b[1] = 1;
  b.writeUInt16BE(port ^ (MAGIC >>> 16), 2);
  b.writeUInt32BE((ip.split(".").reduce((a, o) => (a << 8) | Number(o), 0) ^ MAGIC) >>> 0, 4);
  return b;
};
const header = (type, len, txid) => {
  const h = Buffer.alloc(20);
  h.writeUInt16BE(type, 0);
  h.writeUInt16BE(len, 2);
  h.writeUInt32BE(MAGIC, 4);
  txid.copy(h, 8);
  return h;
};
function build(type, attrs, auth) {
  const txid = randomBytes(12);
  let body = Buffer.concat(attrs);
  if (auth) {
    const key = createHash("md5").update(`${auth.user}:${auth.realm}:${auth.pass}`).digest();
    const pre = Buffer.concat([header(type, body.length + 24, txid), body]);
    body = Buffer.concat([body, attr(A.integrity, createHmac("sha1", key).update(pre).digest())]);
  }
  return { txid, msg: Buffer.concat([header(type, body.length, txid), body]) };
}
function parse(msg) {
  if (msg.length < 20 || msg.readUInt32BE(4) !== MAGIC) return null;
  const attrs = new Map();
  let off = 20;
  const end = 20 + msg.readUInt16BE(2);
  while (off + 4 <= end) {
    const t = msg.readUInt16BE(off);
    const l = msg.readUInt16BE(off + 2);
    attrs.set(t, msg.subarray(off + 4, off + 4 + l));
    off += 4 + l + pad4(l);
  }
  const ec = attrs.get(A.errorCode);
  return {
    type: msg.readUInt16BE(0),
    txid: msg.subarray(8, 20).toString("hex"),
    attrs,
    error: ec ? ec[2] * 100 + ec[3] : 0,
  };
}

const myIp = Object.values(networkInterfaces())
  .flat()
  .find((i) => i && i.family === "IPv4" && !i.internal)?.address;
if (!myIp) {
  console.error("public-server-logs-client: no non-loopback IPv4 address");
  process.exit(2);
}
console.log(`CLIENT_ADDRESS ${myIp}`);

const credential = (ttl, secret) => {
  const user = `${Math.floor(Date.now() / 1000) + ttl}:${randomBytes(16).toString("base64url")}`;
  return { user, pass: createHmac("sha1", secret).update(user).digest("base64") };
};

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
const UDP = Buffer.from([17, 0, 0, 0]);

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
  ["wrong secret", credential(CRED_TTL_S, randomBytes(8).toString("hex"))],
  ["expired", credential(-30, SECRET)],
]) {
  const s = await open();
  const r = await new Client(s, cred).txn(M.allocate, [attr(A.transport, UDP)]);
  if (ok(r)) fail(`allocation with a ${label} credential succeeded`);
  else log(`allocation with a ${label} credential refused (${r.error}), as expected`);
  s.close();
}

// 3. The relayed call.
const peerSock = await open();
const peer = { ip: myIp, port: peerSock.address().port };
peerSock.on("message", (m, r) => peerSock.send(m, r.port, r.address)); // echo
const sock = await open();
const c = new Client(sock, credential(CRED_TTL_S, SECRET));
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
  const rf = await c.txn(M.refresh, [attr(A.lifetime, Buffer.from([0, 0, 0, 60]))]);
  if (!ok(rf)) fail(`refresh refused (${rf.error})`);
  const rel = await c.txn(M.refresh, [attr(A.lifetime, Buffer.from([0, 0, 0, 0]))]);
  if (!ok(rel)) fail(`release refused (${rel.error})`);
}
sock.close();
peerSock.close();

// 4. The signaling server: a plain request and a WebSocket that sends garbage.
if (SIGNALING_URL) {
  try {
    const r = await fetch(`${SIGNALING_URL}/signaling/healthz`);
    log(`signaling /healthz: ${r.status} ${(await r.text()).trim()}`);
    if (!r.ok) fail("signaling /healthz not 200");
  } catch (e) {
    fail(`signaling /healthz: ${e}`);
  }
  await new Promise((resolve) => {
    const ws = new WebSocket(`${SIGNALING_URL.replace(/^http/, "ws")}/signaling`);
    const done = () => resolve();
    ws.onopen = () => {
      ws.send('{"type":"hello","garbage":true}');
      setTimeout(() => ws.close(), 500);
    };
    ws.onclose = done;
    ws.onerror = done;
    setTimeout(done, 3000);
  });
  log("signaling websocket probe done");
}

log(failures.length ? `VERDICT FAIL (${failures.length})` : "VERDICT PASS");
process.exit(failures.length ? 1 : 0);
