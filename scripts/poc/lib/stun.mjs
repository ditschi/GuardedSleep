// STUN/TURN wire helpers shared by the PoC clients (scripts/poc/turn-lifetime-client.mjs,
// public-server-logs-client.mjs and public-server-turns-client.mjs); StunSession is the request/response
// session over any transport (UDP socket or TLS stream) used by the last two.
// Plain Node 22 ESM (crypto, os), no dependencies; not part of the shipped product.
// Protocol: RFC 8489 (STUN, long-term credentials), RFC 8656 (TURN); coturn REST credentials
// https://github.com/coturn/coturn/blob/master/README.turnserver (--use-auth-secret).
import { createHash, createHmac, randomBytes } from "node:crypto";
import { networkInterfaces } from "node:os";

export const MAGIC = 0x2112a442;
export const M = {
  binding: 0x001,
  allocate: 0x003,
  refresh: 0x004,
  createPermission: 0x008,
  channelBind: 0x009,
  sendIndication: 0x016,
  dataIndication: 0x017,
};
export const A = {
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
export const UDP_TRANSPORT = Buffer.from([17, 0, 0, 0]); // REQUESTED-TRANSPORT: protocol 17 (UDP), RFC 8656 section 18.2

export const pad4 = (n) => (4 - (n % 4)) % 4;
export function attr(type, value) {
  const h = Buffer.alloc(4);
  h.writeUInt16BE(type, 0);
  h.writeUInt16BE(value.length, 2);
  return Buffer.concat([h, value, Buffer.alloc(pad4(value.length))]);
}
export const u32 = (n) => {
  const b = Buffer.alloc(4);
  b.writeUInt32BE(n >>> 0);
  return b;
};
export function xorAddr(ip, port) {
  const b = Buffer.alloc(8);
  b[1] = 1;
  b.writeUInt16BE(port ^ (MAGIC >>> 16), 2);
  b.writeUInt32BE((ip.split(".").reduce((a, o) => (a << 8) | Number(o), 0) ^ MAGIC) >>> 0, 4);
  return b;
}
export function parseXorAddr(v) {
  const port = v.readUInt16BE(2) ^ (MAGIC >>> 16);
  const ip = (v.readUInt32BE(4) ^ MAGIC) >>> 0;
  return { ip: [24, 16, 8, 0].map((s) => (ip >>> s) & 255).join("."), port };
}
export function header(type, len, txid) {
  const h = Buffer.alloc(20);
  h.writeUInt16BE(type, 0);
  h.writeUInt16BE(len, 2);
  h.writeUInt32BE(MAGIC, 4);
  txid.copy(h, 8);
  return h;
}
export function build(type, attrs, auth) {
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
export function parse(msg) {
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

export const ok = (r) => !r.error && (r.type & 0x0110) === 0x0100; // a success response class

// One STUN client session over a transport {send(buf), on(cb)}: cb receives every whole STUN message (the caller
// frames a stream). Requests are retransmitted `tries` times `intervalMs` apart (UDP defaults; a reliable stream
// passes tries 1 and a longer interval), a 401 with realm and nonce is answered once with long-term credentials
// (RFC 8489 section 9.2), and anything that matches no pending request (indications, ChannelData) goes to
// onOther(raw, parsed-or-null).
export class StunSession {
  constructor(transport, cred, { tries = 3, intervalMs = 1500 } = {}) {
    this.t = transport;
    this.cred = cred;
    this.tries = tries;
    this.intervalMs = intervalMs;
    this.realm = "";
    this.nonce = "";
    this.pending = new Map();
    this.onOther = () => {};
    transport.on((m) => {
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
      let sent = 0;
      const tx = txid.toString("hex");
      const timer = setInterval(() => {
        if (++sent >= this.tries) {
          clearInterval(timer);
          this.pending.delete(tx);
          rej(new Error("no response"));
        } else this.t.send(msg);
      }, this.intervalMs);
      this.pending.set(tx, (p, raw) => {
        clearInterval(timer);
        this.pending.delete(tx);
        res({ ...p, raw });
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

// REST credential: <expiry unix s>:<16 random bytes base64url>, password base64(HMAC-SHA1(secret, name)).
export function restCredential(ttlS, secret) {
  const user = `${Math.floor(Date.now() / 1000) + ttlS}:${randomBytes(16).toString("base64url")}`;
  return { user, pass: createHmac("sha1", secret).update(user).digest("base64") };
}

// The first non-loopback IPv4 address of this host, or undefined.
export function ownIPv4() {
  return Object.values(networkInterfaces())
    .flat()
    .find((i) => i && i.family === "IPv4" && !i.internal)?.address;
}
