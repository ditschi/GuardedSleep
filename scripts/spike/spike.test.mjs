// Offline tests of the pure parts of the relay measurement scripts (run by `just test-tooling`).
import assert from "node:assert/strict";
import test from "node:test";
import { parseArgs, percentile, table } from "./lib.mjs";
import { judge } from "./relay-probe.mjs";
import { isBindingSuccess } from "./stun-probe.mjs";

test("percentile is nearest rank", () => {
  assert.equal(percentile([], 50), null);
  const v = Array.from({ length: 20 }, (_, i) => i + 1);
  assert.equal(percentile(v, 50), 10);
  assert.equal(percentile(v, 95), 19);
  assert.equal(percentile([7], 95), 7);
});

test("parseArgs knows flags, values, numbers and rejects the unknown", () => {
  const spec = {
    out: { default: "x" },
    n: { default: 1, number: true },
    quick: { flag: true, default: false },
  };
  assert.deepEqual(parseArgs(["--n", "5", "--quick"], spec), { out: "x", n: 5, quick: true });
  assert.throws(() => parseArgs(["--nope"], spec), /unknown argument/);
  assert.throws(() => parseArgs(["--out"], spec), /needs a value/);
});

test("table renders a markdown table", () => {
  assert.equal(table(["a", "b"], [["1", "2"]]), "| a | b |\n| --- | --- |\n| 1 | 2 |\n");
});

function request() {
  const msg = Buffer.alloc(20);
  msg.writeUInt16BE(0x0001, 0);
  msg.writeUInt32BE(0x2112a442, 4);
  for (let i = 8; i < 20; i++) msg[i] = i;
  return msg;
}

function reply(request, type, attribute) {
  const body = Buffer.alloc(8);
  body.writeUInt16BE(attribute, 0);
  body.writeUInt16BE(4, 2);
  const head = Buffer.alloc(20);
  head.writeUInt16BE(type, 0);
  head.writeUInt16BE(body.length, 2);
  request.copy(head, 4, 4, 20);
  return Buffer.concat([head, body]);
}

test("a Binding success response with a mapped address is recognised", () => {
  const req = request();
  assert.equal(isBindingSuccess(req, reply(req, 0x0101, 0x0020)), true);
  assert.equal(isBindingSuccess(req, reply(req, 0x0101, 0x0001)), true);
  assert.equal(isBindingSuccess(req, reply(req, 0x0111, 0x0020)), false, "error response");
  assert.equal(isBindingSuccess(req, reply(req, 0x0101, 0x8022)), false, "no mapped address");
  const other = request();
  other[10] ^= 0xff;
  assert.equal(isBindingSuccess(other, reply(req, 0x0101, 0x0020)), false, "other transaction");
});

test("judge accepts a healthy relay and names the first problem otherwise", () => {
  const healthy = {
    connected: true,
    subscribed: true,
    delivery: { sent: 20, delivered: 20, p95_ms: 200 },
    sizes: { 2048: { accepted: true }, 4096: { accepted: true } },
  };
  assert.equal(judge(healthy).ok, true);
  assert.match(
    judge({ ...healthy, delivery: { sent: 20, delivered: 10, p95_ms: 200 } }).why,
    /10\/20/,
  );
  assert.match(
    judge({ ...healthy, delivery: { sent: 20, delivered: 20, p95_ms: 4000 } }).why,
    /p95/,
  );
  assert.match(
    judge({ ...healthy, sizes: { 2048: { accepted: true }, 4096: { accepted: false } } }).why,
    /4096/,
  );
  assert.equal(judge({ connected: false, error: "connect timeout" }).ok, false);
});
