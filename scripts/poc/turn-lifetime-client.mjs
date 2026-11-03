// Task 9.1 spike client (alpha-connection-model, design section 7): one TURN allocation made with a
// 60-second coturn REST credential must keep relaying past --stale-nonce (600 s) and long past the
// credential expiry. Runs inside a container on the same Docker network as coturn
// (scripts/poc/turn-lifetime.sh); not part of the shipped product.
//
// Plain Node 22 (dgram, crypto), no dependencies. Env: TURN_HOST, TURN_SECRET, DURATION_S (default
// 700), CRED_TTL_S (60), RENEW_S (90), PING_S (10). Exit 0 = all assertions hold.
// Protocol: RFC 8656 (TURN), RFC 8489 (STUN long-term credentials), coturn REST scheme
// https://github.com/coturn/coturn/blob/master/README.turnserver (--use-auth-secret).
import { createHash, createHmac, randomBytes } from "node:crypto";
import dgram from "node:dgram";
import { networkInterfaces } from "node:os";

const env = (k, d) => process.env[k] ?? d;
const HOST = env("TURN_HOST", "");
const SECRET = env("TURN_SECRET", "");
const DURATION_S = Number(env("DURATION_S", "700"));
const CRED_TTL_S = Number(env("CRED_TTL_S", "60"));
const RENEW_S = Number(env("RENEW_S", "90"));
const PING_S = Number(env("PING_S", "10"));
if (!HOST || !SECRET) {
  console.error("turn-lifetime-client: TURN_HOST and TURN_SECRET are required");
  process.exit(2);
}

const MAGIC = 0x2112a442;
const M = { allocate: 0x003, refresh: 0x004, createPermission: 0x008, channelBind: 0x009 };
const A = {
  username: 0x0006,
  integrity: 0x0008,
  errorCode: 0x0009,
  channelNumber: 0x000c,
  lifetime: 0x000d,
  xorPeer: 0x0012,
  data: 0x0013,
  realm: 0x0014,
  nonce: 0x0015,
  xorRelayed: 0x0016,
  transport: 0x0019,
};
const CHANNEL = 0x4000;

const t0 = Date.now();
const elapsed = () => (Date.now() - t0) / 1000;
const log = (m) => console.log(`[t=${elapsed().toFixed(1).padStart(6)}s] ${m}`);
const failures = [];
const fail = (m) => {
  failures.push(m);
  log(`FAIL ${m}`);
};

const pad4 = (n) => (4 - (n % 4)) % 4;
function attr(type, value) {
  const h = Buffer.alloc(4);
  h.writeUInt16BE(type, 0);
  h.writeUInt16BE(value.length, 2);
  return Buffer.concat([h, value, Buffer.alloc(pad4(value.length))]);
}
const u32 = (n) => {
  const b = Buffer.alloc(4);
  b.writeUInt32BE(n >>> 0);
  return b;
};
function xorAddr(ip, port) {
  const b = Buffer.alloc(8);
  b[1] = 1;
  b.writeUInt16BE(port ^ (MAGIC >>> 16), 2);
  b.writeUInt32BE((ip.split(".").reduce((a, o) => (a << 8) | Number(o), 0) ^ MAGIC) >>> 0, 4);
  return b;
}
function parseXorAddr(v) {
  const port = v.readUInt16BE(2) ^ (MAGIC >>> 16);
  const ip = (v.readUInt32BE(4) ^ MAGIC) >>> 0;
  return { ip: [24, 16, 8, 0].map((s) => (ip >>> s) & 255).join("."), port };
}
function header(type, len, txid) {
  const h = Buffer.alloc(20);
  h.writeUInt16BE(type, 0);
  h.writeUInt16BE(len, 2);
  h.writeUInt32BE(MAGIC, 4);
  txid.copy(h, 8);
  return h;
}
function build(type, attrs, auth) {
  const txid = randomBytes(12);
  let body = Buffer.concat(attrs);
  if (auth) {
    const key = createHash("md5").update(`${auth.user}:${auth.realm}:${auth.pass}`).digest();
    const pre = Buffer.concat([header(type, body.length + 24, txid), body]);
    const mac = createHmac("sha1", key).update(pre).digest();
    body = Buffer.concat([body, attr(A.integrity, mac)]);
  }
  return { txid, msg: Buffer.concat([header(type, body.length, txid), body]) };
}
function parse(msg) {
  if (msg.length < 20 || msg.readUInt32BE(4) !== MAGIC) return null;
  const type = msg.readUInt16BE(0);
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
    type,
    txid: msg.subarray(8, 20).toString("hex"),
    attrs,
    error: ec ? ec[2] * 100 + ec[3] : 0,
  };
}

// REST credential: <expiry unix s>:<16 random bytes base64url>, password base64(HMAC-SHA1(secret, name)).
const expiry = Math.floor(Date.now() / 1000) + CRED_TTL_S;
const user = `${expiry}:${randomBytes(16).toString("base64url")}`;
const pass = createHmac("sha1", SECRET).update(user).digest("base64");

const myIp = Object.values(networkInterfaces())
  .flat()
  .find((i) => i && i.family === "IPv4" && !i.internal)?.address;
if (!myIp) {
  console.error("turn-lifetime-client: no non-loopback IPv4 address");
  process.exit(2);
}

function bind(sock) {
  return new Promise((res) => sock.bind(0, "0.0.0.0", res));
}

// One STUN client over a socket: tracks realm/nonce, retries once on 401 (first contact) and on 438
// (stale nonce) with the ORIGINAL credential, and records what it saw.
class Client {
  constructor(sock) {
    this.sock = sock;
    this.realm = "";
    this.nonce = "";
    this.pending = new Map();
    this.onOther = () => {};
    sock.on("message", (m) => {
      const p = parse(m);
      if (p && this.pending.has(p.txid)) this.pending.get(p.txid)(p);
      else this.onOther(m, p);
    });
  }
  send1(type, attrs, withAuth) {
    const auth = withAuth ? { user, pass, realm: this.realm } : null;
    const extra = withAuth
      ? [
          attr(A.username, Buffer.from(user)),
          attr(A.realm, Buffer.from(this.realm)),
          attr(A.nonce, Buffer.from(this.nonce)),
        ]
      : [];
    const { txid, msg } = build(type, [...attrs, ...extra], auth);
    return new Promise((res, rej) => {
      let tries = 0;
      const tx = txid.toString("hex");
      const timer = setInterval(() => {
        if (++tries >= 4) {
          clearInterval(timer);
          this.pending.delete(tx);
          rej(new Error("no response (timeout)"));
        } else this.sock.send(msg, 3478, HOST);
      }, 1500);
      this.pending.set(tx, (p) => {
        clearInterval(timer);
        this.pending.delete(tx);
        res(p);
      });
      this.sock.send(msg, 3478, HOST);
    });
  }
  // Returns {ok, error, retried, resp}. `retried` lists the error codes recovered from.
  async txn(method, attrs, label) {
    const retried = [];
    let r = await this.send1(method, attrs, !!this.nonce);
    for (let i = 0; i < 2 && r.error && [401, 438].includes(r.error); i++) {
      const realm = r.attrs.get(A.realm);
      const nonce = r.attrs.get(A.nonce);
      if (!realm || !nonce) break;
      if (r.error === 401 && this.nonce) break; // credential rejected with a fresh nonce: not recoverable
      this.realm = realm.toString();
      this.nonce = nonce.toString();
      retried.push(r.error);
      r = await this.send1(method, attrs, true);
    }
    const ok = !r.error && (r.type & 0x0110) === 0x0100;
    log(
      `${label}: ${ok ? "success" : `ERROR ${r.error}`}${retried.length ? ` (recovered from ${retried.join(",")} with the original credential)` : ""}`,
    );
    return { ok, error: r.error, retried, resp: r };
  }
}

const peerSock = dgram.createSocket("udp4");
const sock = dgram.createSocket("udp4");
await bind(peerSock);
await bind(sock);
const peer = { ip: myIp, port: peerSock.address().port };
// The peer echoes everything to whoever sent it (the relayed address).
peerSock.on("message", (m, r) => peerSock.send(m, r.port, r.address));

const c = new Client(sock);
// Once a channel is bound, coturn frames the peer's replies as ChannelData for both outbound paths
// (Send indication and ChannelData), so echoes are matched by payload, whatever the framing.
const echoes = new Map();
c.onOther = (m) => {
  // ChannelData: 0x4000..0x7FFF; Data indication 0x0017.
  if ((m[0] & 0xc0) === 0x40) {
    echoes.set(m.subarray(4, 4 + m.readUInt16BE(2)).toString(), elapsed());
  } else {
    const p = parse(m);
    if (p && p.type === 0x0017 && p.attrs.get(A.data))
      echoes.set(p.attrs.get(A.data).toString(), elapsed());
  }
};

// 1. Allocate (401 round trip happens inside txn).
const alloc = await c.txn(M.allocate, [attr(A.transport, Buffer.from([17, 0, 0, 0]))], "Allocate");
if (!alloc.ok) {
  fail("allocation failed");
  process.exit(1);
}
const relayed = parseXorAddr(alloc.resp.attrs.get(A.xorRelayed));
log(`relayed address ${relayed.ip}:${relayed.port}, credential expires in ${CRED_TTL_S} s`);

const renewals = []; // {t, kind, ok, error, retried}
async function renew(kind, method, attrs) {
  const r = await c.txn(method, attrs, `${kind} renewal`);
  renewals.push({ t: elapsed(), kind, ok: r.ok, error: r.error, retried: r.retried });
  if (!r.ok) fail(`${kind} at t=${elapsed().toFixed(0)}s answered ${r.error}`);
}
const renewAll = async () => {
  await renew("Refresh", M.refresh, [attr(A.lifetime, u32(600))]);
  await renew("CreatePermission", M.createPermission, [attr(A.xorPeer, xorAddr(peer.ip, 0))]);
  await renew("ChannelBind", M.channelBind, [
    attr(A.channelNumber, Buffer.from([CHANNEL >> 8, CHANNEL & 255, 0, 0])),
    attr(A.xorPeer, xorAddr(peer.ip, peer.port)),
  ]);
};
await renewAll(); // initial permission and channel (also proves the first Refresh)

// Negative control: the credential must really be expired for a NEW allocation by now.
async function expiredControl() {
  const s2 = dgram.createSocket("udp4");
  await bind(s2);
  const c2 = new Client(s2);
  const r = await c2.txn(
    M.allocate,
    [attr(A.transport, Buffer.from([17, 0, 0, 0]))],
    "control Allocate with expired credential",
  );
  s2.close();
  if (r.ok)
    fail(
      "control: a new allocation with the EXPIRED credential succeeded (credential not expired, test invalid)",
    );
  else
    log(
      `control: new allocation with the expired credential is rejected (${r.error}), as expected`,
    );
}

// Ping through both paths.
const pings = [];
let seq = 0;
function ping() {
  const n = seq++;
  const sMsg = `send-${n}`;
  const cMsg = `chan-${n}`;
  const { msg } = build(
    0x0016,
    [attr(A.xorPeer, xorAddr(peer.ip, peer.port)), attr(A.data, Buffer.from(sMsg))],
    null,
  );
  sock.send(msg, 3478, HOST);
  const cd = Buffer.alloc(4);
  cd.writeUInt16BE(CHANNEL, 0);
  cd.writeUInt16BE(cMsg.length, 2);
  sock.send(Buffer.concat([cd, Buffer.from(cMsg), Buffer.alloc(pad4(cMsg.length))]), 3478, HOST);
  pings.push({ n, t: elapsed(), sMsg, cMsg });
}

let controlDone = false;
const pingTimer = setInterval(ping, PING_S * 1000);
const renewTimer = setInterval(() => void renewAll(), RENEW_S * 1000);
const ctlTimer = setInterval(() => {
  if (!controlDone && elapsed() > CRED_TTL_S + 15) {
    controlDone = true;
    void expiredControl();
  }
}, 1000);
ping();
await new Promise((r) => setTimeout(r, DURATION_S * 1000));
clearInterval(pingTimer);
clearInterval(renewTimer);
clearInterval(ctlTimer);
await new Promise((r) => setTimeout(r, 4000)); // last echoes

// Assertions.
const answered = (p, path) => echoes.has(path === "send" ? p.sMsg : p.cMsg);
// "send": Send indication out; "channel": ChannelData out.
for (const path of ["send", "channel"]) {
  const lost = pings.filter((p) => !answered(p, path));
  const late = pings.filter((p) => p.t >= 620);
  const lateLost = late.filter((p) => !answered(p, path));
  log(
    `${path} path: ${pings.length - lost.length}/${pings.length} pings echoed, ${late.length - lateLost.length}/${late.length} after t=620 s`,
  );
  if (late.length < 3) fail(`${path}: too few pings after 620 s (${late.length})`);
  if (lateLost.length) fail(`${path}: ${lateLost.length} pings lost after 620 s`);
  if (lost.length > 2) fail(`${path}: ${lost.length} pings lost overall`);
}
const post = renewals.filter((r) => r.t >= 600);
for (const kind of ["Refresh", "CreatePermission", "ChannelBind"]) {
  const k = post.filter((r) => r.kind === kind);
  log(
    `${kind}: ${renewals.filter((r) => r.kind === kind && r.ok).length}/${renewals.filter((r) => r.kind === kind).length} succeeded, ${k.length} after t=600 s, recovered 438: ${k.filter((r) => r.retried.includes(438)).length}`,
  );
  if (!k.length) fail(`${kind}: no renewal after the stale-nonce interval (600 s)`);
}
if (elapsed() < 660) fail(`run shorter than 11 minutes (${elapsed().toFixed(0)} s)`);
if (!controlDone) fail("expired-credential control did not run");

log(
  failures.length
    ? `VERDICT FAIL (${failures.length} problems)`
    : `VERDICT PASS after ${elapsed().toFixed(0)} s`,
);
process.exit(failures.length ? 1 : 0);
