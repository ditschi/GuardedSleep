// Task 10.2 (alpha-connection-model, design section 9): the test client of scripts/poc/public-server-turns.sh.
// Plain Node 22 (tls, dgram, crypto), no dependencies; STUN helpers from lib/stun.mjs. TURN over TLS carries the
// same STUN messages on a byte stream (RFC 8656 section 3.1, RFC 8489 section 6.2.2: the length field in the
// 20-byte header frames each message; ChannelData is not used here).
//
// MODE=nocert  coturn runs without a certificate: TLS on 5349 must be refused, and turn: over UDP must still allocate.
// MODE=tls     TLS handshake against CA_FILE with servername SERVER_NAME; the served leaf's serial must equal
//              EXPECT_SERIAL (hex); then allocate, permit the peer, and relay both ways through the TLS stream.
// MODE=old     a TLS 1.1-only handshake must fail (TLS 1.2 is the floor).
// Env: TURN_HOST, TURN_SECRET (REST secret), SERVER_NAME, CA_FILE, EXPECT_SERIAL.
// Prints "VERDICT PASS" (exit 0) or "VERDICT FAIL" (exit 1).
import { readFileSync } from "node:fs";
import dgram from "node:dgram";
import tls from "node:tls";
import {
  A,
  M,
  UDP_TRANSPORT,
  attr,
  build,
  ownIPv4,
  parse,
  restCredential,
  xorAddr,
  parseXorAddr,
} from "./lib/stun.mjs";

const HOST = process.env.TURN_HOST ?? "";
const SECRET = process.env.TURN_SECRET ?? "";
const MODE = process.env.MODE ?? "tls";
const SERVER_NAME = process.env.SERVER_NAME ?? "gs.mydata.rocks";
if (!HOST || !SECRET) {
  console.error("public-server-turns-client: TURN_HOST and TURN_SECRET are required");
  process.exit(2);
}
const failures = [];
const fail = (m) => {
  failures.push(m);
  console.log(`FAIL ${m}`);
};
const log = (m) => console.log(m);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const ok = (r) => !r.error && (r.type & 0x0110) === 0x0100;

// A STUN transport: send(buf) and "message" events with whole STUN messages.
function udpTransport(port) {
  const sock = dgram.createSocket("udp4");
  const t = {
    send: (b) => sock.send(b, port, HOST),
    close: () => sock.close(),
    on: (cb) => sock.on("message", cb),
  };
  return new Promise((r) => sock.bind(0, "0.0.0.0", () => r(t)));
}
function tlsTransport(opts) {
  return new Promise((resolve, reject) => {
    const sock = tls.connect({ host: HOST, port: 5349, ...opts });
    let buf = Buffer.alloc(0);
    let cb = () => {};
    sock.on("data", (d) => {
      buf = Buffer.concat([buf, d]);
      while (buf.length >= 20) {
        const total = 20 + buf.readUInt16BE(2);
        if (buf.length < total) break;
        cb(buf.subarray(0, total));
        buf = buf.subarray(total);
      }
    });
    sock.once("error", reject);
    sock.once("secureConnect", () =>
      resolve({ sock, send: (b) => sock.write(b), close: () => sock.end(), on: (f) => (cb = f) }),
    );
  });
}

class Session {
  constructor(t, cred) {
    this.t = t;
    this.cred = cred;
    this.realm = "";
    this.nonce = "";
    this.pending = new Map();
    this.indications = [];
    t.on((m) => {
      const p = parse(m);
      if (p && this.pending.has(p.txid)) this.pending.get(p.txid)(p);
      else if (p) this.indications.push(p);
    });
  }
  async once(type, attrs, auth) {
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
      const tx = txid.toString("hex");
      const timer = setTimeout(() => rej(new Error("no response")), 5000);
      this.pending.set(tx, (p) => {
        clearTimeout(timer);
        res(p);
      });
      this.t.send(msg);
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

if (MODE === "nocert") {
  try {
    await tlsTransport({ rejectUnauthorized: false });
    fail("TLS on 5349 accepted a connection although coturn has no certificate");
  } catch (e) {
    log(`TLS on 5349 refused without a certificate (${e.code ?? e.message}), as expected`);
  }
  const s = new Session(await udpTransport(3478), restCredential(60, SECRET));
  const r = await s.txn(M.allocate, [attr(A.transport, UDP_TRANSPORT)]);
  if (ok(r)) log("turn: allocation over UDP works without a certificate");
  else fail(`turn: allocation over UDP failed without a certificate (${r.error})`);
  s.t.close();
} else if (MODE === "old") {
  try {
    await tlsTransport({
      ca: readFileSync(process.env.CA_FILE),
      servername: SERVER_NAME,
      minVersion: "TLSv1",
      maxVersion: "TLSv1.1",
      ciphers: "DEFAULT:@SECLEVEL=0",
    });
    fail("a TLS 1.1 handshake succeeded");
  } catch (e) {
    log(`TLS 1.1 handshake refused (${e.code ?? e.message}), as expected`);
  }
} else {
  const myIp = ownIPv4();
  const tt = await tlsTransport({
    ca: readFileSync(process.env.CA_FILE),
    servername: SERVER_NAME,
    minVersion: "TLSv1.2",
  });
  const leaf = tt.sock.getPeerCertificate();
  const serial = String(leaf.serialNumber).toLowerCase().replace(/^0+/, "");
  log(
    `TLS ${tt.sock.getProtocol()} ${tt.sock.getCipher().name}, leaf serial ${serial}, verified against the test CA`,
  );
  if (serial !== (process.env.EXPECT_SERIAL ?? "").toLowerCase().replace(/^0+/, ""))
    fail(`served serial ${serial}, expected ${process.env.EXPECT_SERIAL}`);
  const s = new Session(tt, restCredential(60, SECRET));
  const peerSock = dgram.createSocket("udp4");
  await new Promise((r) => peerSock.bind(0, "0.0.0.0", r));
  const peerGot = new Set();
  peerSock.on("message", (m) => peerGot.add(m.toString()));
  const peer = { ip: myIp, port: peerSock.address().port };
  const alloc = await s.txn(M.allocate, [attr(A.transport, UDP_TRANSPORT)]);
  if (!ok(alloc)) fail(`allocation over TLS refused (${alloc.error})`);
  else {
    const relay = parseXorAddr(alloc.attrs.get(A.xorRelayed));
    log(`allocation over TLS succeeded (relay port ${relay.port})`);
    const perm = await s.txn(M.createPermission, [attr(A.xorPeer, xorAddr(peer.ip, 0))]);
    if (!ok(perm)) fail(`permission refused (${perm.error})`);
    // client -> relay -> peer: a Send indication through the TLS stream
    for (let i = 0; i < 3; i++) {
      tt.send(
        build(
          0x0016,
          [attr(A.xorPeer, xorAddr(peer.ip, peer.port)), attr(A.data, Buffer.from(`out-${i}`))],
          null,
        ).msg,
      );
      await sleep(150);
    }
    // peer -> relay -> client: arrives as a Data indication on the TLS stream
    const toRelay = relay.ip === "0.0.0.0" ? HOST : relay.ip;
    for (let i = 0; i < 3; i++) {
      peerSock.send(`in-${i}`, relay.port, toRelay);
      await sleep(150);
    }
    await sleep(500);
    const outN = [0, 1, 2].filter((i) => peerGot.has(`out-${i}`)).length;
    const inN = s.indications.filter(
      (p) => p.type === 0x0017 && /^in-\d$/.test(String(p.attrs.get(A.data))),
    ).length;
    log(`relayed through TLS: client to peer ${outN}/3, peer to client ${inN}/3`);
    if (outN < 2 || inN < 2) fail("relayed data through the TLS stream is incomplete");
    const rel = await s.txn(M.refresh, [attr(0x000d, Buffer.from([0, 0, 0, 0]))]);
    if (!ok(rel)) fail(`release refused (${rel.error})`);
  }
  tt.close();
  peerSock.close();
}
log(failures.length ? `VERDICT FAIL (${failures.length})` : "VERDICT PASS");
process.exit(failures.length ? 1 : 0);
