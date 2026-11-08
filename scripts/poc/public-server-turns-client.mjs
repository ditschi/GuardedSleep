// Task 10.2 (alpha-connection-model, design section 9): the test client of scripts/poc/public-server-turns.sh.
// Plain Node 22 (tls, dgram, crypto), no dependencies; STUN helpers from lib/stun.mjs. TURN over TLS carries the
// same STUN messages on a byte stream (RFC 8656 section 3.1, RFC 8489 section 6.2.2: the length field in the
// 20-byte header frames each message; ChannelData is not used here).
//
// MODE=nocert  coturn runs without a certificate: TLS on 5349 must be refused, and turn: over UDP must still allocate.
// MODE=tls     TLS handshake against CA_FILE with servername SERVER_NAME; the served leaf's serial must equal
//              EXPECT_SERIAL (hex); then allocate, permit the peer, and relay both ways through the TLS stream.
// MODE=old     a TLS 1.1-only handshake must fail (TLS 1.2 is the floor).
// MODE=badcred  a REST credential signed with the wrong secret must be rejected (401) over TLS.
// Env: TURN_HOST, TURN_SECRET (REST secret), SERVER_NAME, CA_FILE, EXPECT_SERIAL.
// Extras for scripts/poc/public-server-turns-traefik.sh (TLS ends at Traefik there):
//   PORT        TLS port (default 5349).
//   SERVER_NAME empty: no SNI and no chain verification (the certificate is only compared by serial), to see which
//               certificate a client without SNI gets.
//   HOLD_S      after the first relay check keep the connection for that many seconds (Refresh every 15 s), then
//               relay again; HOLD_IDLE=1 sends nothing meanwhile (idle TCP connection); EXPECT_BREAK=1 inverts the verdict: the proxy must have closed the connection.
// Prints "VERDICT PASS" (exit 0) or "VERDICT FAIL" (exit 1).
import { readFileSync } from "node:fs";
import dgram from "node:dgram";
import tls from "node:tls";
import {
  A,
  M,
  StunSession,
  UDP_TRANSPORT,
  attr,
  build,
  ok,
  ownIPv4,
  restCredential,
  u32,
  xorAddr,
  parseXorAddr,
} from "./lib/stun.mjs";

const HOST = process.env.TURN_HOST ?? "";
const SECRET = process.env.TURN_SECRET ?? "";
const MODE = process.env.MODE ?? "tls";
const PORT = Number(process.env.PORT ?? 5349);
const HOLD_S = Number(process.env.HOLD_S ?? 0);
const HOLD_IDLE = process.env.HOLD_IDLE === "1";
const EXPECT_BREAK = process.env.EXPECT_BREAK === "1";
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
    const sock = tls.connect({ host: HOST, port: PORT, ...opts });
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
    sock.once("secureConnect", () => {
      const t = {
        sock,
        closedAt: 0,
        send: (b) => sock.write(b),
        close: () => sock.end(),
        on: (f) => (cb = f),
      };
      // After the handshake an error (a reset by the proxy) must not crash the client; the close is the signal.
      sock.on("error", () => {});
      sock.on("close", () => (t.closedAt ||= Date.now()));
      resolve(t);
    });
  });
}

if (MODE === "nocert") {
  try {
    await tlsTransport({ rejectUnauthorized: false });
    fail("TLS on 5349 accepted a connection although coturn has no certificate");
  } catch (e) {
    log(`TLS on 5349 refused without a certificate (${e.code ?? e.message}), as expected`);
  }
  const s = new StunSession(await udpTransport(3478), restCredential(60, SECRET));
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
  const noSni = SERVER_NAME === "";
  const tt = await tlsTransport(
    noSni
      ? { rejectUnauthorized: false, minVersion: "TLSv1.2" }
      : { ca: readFileSync(process.env.CA_FILE), servername: SERVER_NAME, minVersion: "TLSv1.2" },
  );
  const leaf = tt.sock.getPeerCertificate();
  const serial = String(leaf.serialNumber).toLowerCase().replace(/^0+/, "");
  log(
    `TLS ${tt.sock.getProtocol()} ${tt.sock.getCipher().name}, leaf serial ${serial}, ${noSni ? "no SNI, not verified" : "verified against the test CA"}`,
  );
  if (serial !== (process.env.EXPECT_SERIAL ?? "").toLowerCase().replace(/^0+/, ""))
    fail(`served serial ${serial}, expected ${process.env.EXPECT_SERIAL}`);
  const bad = MODE === "badcred";
  const s = new StunSession(tt, restCredential(HOLD_S + 120, bad ? `${SECRET}x` : SECRET), {
    tries: 1,
    intervalMs: 5000,
  });
  const indications = [];
  s.onOther = (_raw, p) => p && indications.push(p);
  const peerSock = dgram.createSocket("udp4");
  await new Promise((r) => peerSock.bind(0, "0.0.0.0", r));
  const peerGot = new Set();
  peerSock.on("message", (m) => peerGot.add(m.toString()));
  const peer = { ip: myIp, port: peerSock.address().port };
  const alloc = await s.txn(M.allocate, [attr(A.transport, UDP_TRANSPORT)]);
  if (bad) {
    if (ok(alloc)) fail("an allocation with a wrong credential was accepted");
    else log(`allocation with a wrong credential rejected (${alloc.error}), as expected`);
  } else if (!ok(alloc)) fail(`allocation over TLS refused (${alloc.error})`);
  else {
    const relay = parseXorAddr(alloc.attrs.get(A.xorRelayed));
    log(`allocation over TLS succeeded (relay port ${relay.port})`);
    const perm = await s.txn(M.createPermission, [attr(A.xorPeer, xorAddr(peer.ip, 0))]);
    if (!ok(perm)) fail(`permission refused (${perm.error})`);
    const toRelay = relay.ip === "0.0.0.0" ? HOST : relay.ip;
    let round = 0;
    const relayCheck = async () => {
      const r = round++;
      // client -> relay -> peer: a Send indication through the TLS stream
      for (let i = 0; i < 3; i++) {
        tt.send(
          build(
            M.sendIndication,
            [
              attr(A.xorPeer, xorAddr(peer.ip, peer.port)),
              attr(A.data, Buffer.from(`out-${r}-${i}`)),
            ],
            null,
          ).msg,
        );
        await sleep(150);
      }
      // peer -> relay -> client: arrives as a Data indication on the TLS stream
      for (let i = 0; i < 3; i++) {
        peerSock.send(`in-${r}-${i}`, relay.port, toRelay);
        await sleep(150);
      }
      await sleep(500);
      const outN = [0, 1, 2].filter((i) => peerGot.has(`out-${r}-${i}`)).length;
      const inN = indications.filter(
        (p) => p.type === M.dataIndication && String(p.attrs.get(A.data)).startsWith(`in-${r}-`),
      ).length;
      log(`relayed through TLS (round ${r}): client to peer ${outN}/3, peer to client ${inN}/3`);
      return outN >= 2 && inN >= 2;
    };
    const first = await relayCheck();
    if (!first) fail("relayed data through the TLS stream is incomplete");
    if (HOLD_S > 0) {
      const t0 = Date.now();
      while (!tt.closedAt && Date.now() - t0 < HOLD_S * 1000) {
        await sleep(Math.min(15000, HOLD_S * 1000 - (Date.now() - t0)));
        if (tt.closedAt) break;
        if (HOLD_IDLE) continue;
        const rf = await s
          .txn(M.refresh, [attr(A.lifetime, u32(120))])
          .catch((e) => ({ error: e.message }));
        log(`t+${Math.round((Date.now() - t0) / 1000)}s refresh: ${ok(rf) ? "ok" : rf.error}`);
      }
      const held = Math.round(((tt.closedAt || Date.now()) - t0) / 1000);
      if (tt.closedAt)
        log(`HOLD connection closed by the peer side after ${held}s (asked for ${HOLD_S}s)`);
      else log(`HOLD connection still open after ${held}s`);
      if (EXPECT_BREAK) {
        if (!tt.closedAt) fail(`the connection survived ${held}s although a break was expected`);
      } else if (tt.closedAt) fail(`the connection was closed after ${held}s`);
      else if (!(await relayCheck())) fail(`no relay after ${held}s`);
    }
    if (!tt.closedAt) {
      const rel = await s.txn(M.refresh, [attr(A.lifetime, u32(0))]);
      if (!ok(rel)) fail(`release refused (${rel.error})`);
    }
  }
  tt.close();
  peerSock.close();
}
log(failures.length ? `VERDICT FAIL (${failures.length})` : "VERDICT PASS");
process.exit(failures.length ? 1 : 0);
