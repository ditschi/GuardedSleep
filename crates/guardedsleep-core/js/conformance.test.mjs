// AC-5 (ADR-015): the WASM build of the core must reproduce the golden vectors exactly.
// Run after scripts/core-build.sh wasm:  node --test crates/guardedsleep-core/js/*.test.mjs
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { test } from "node:test";

const require = createRequire(import.meta.url);
const wasm = require("../../../target/wasm-pkg/node/guardedsleep_wasm.js");
const vectors = JSON.parse(
  readFileSync(new URL("../vectors/conformance.json", import.meta.url), "utf8"),
);

const traceDir = new URL("../vectors/traces/", import.meta.url);
for (const f of readdirSync(traceDir)
  .filter((n) => n.endsWith(".json"))
  .sort()) {
  test(`wasm trace replay: ${f}`, () => {
    const trace = JSON.parse(readFileSync(new URL(f, traceDir), "utf8"));
    const got = JSON.parse(wasm.conformanceEval(JSON.stringify(trace)));
    assert.deepStrictEqual(got, trace.expect);
  });
}

// Typed replay: the trace inputs go through WasmLink.handle (serde lift of LinkConfig/LinkInput in the
// wasm build, not the conformanceEval pass-through) and every returned action list must equal the
// committed expectation.
for (const f of readdirSync(traceDir)
  .filter((n) => n.endsWith(".json"))
  .sort()) {
  test(`wasm typed link replay: ${f}`, () => {
    const trace = JSON.parse(readFileSync(new URL(f, traceDir), "utf8"));
    const link = new wasm.WasmLink(JSON.stringify(trace.config));
    const got = trace.steps.map((s) => JSON.parse(link.handle(s.now, JSON.stringify(s.input))));
    assert.deepStrictEqual(got, trace.expect);
  });
}

for (const v of vectors) {
  test(`wasm conformance: ${v.name}`, () => {
    const { expect, ...input } = v;
    const got = JSON.parse(wasm.conformanceEval(JSON.stringify(input)));
    assert.deepStrictEqual(got, expect);
  });
}

/** 32-byte colon-separated upper-case fingerprint, as in an SDP. */
const fingerprint = (seed) =>
  Array.from({ length: 32 }, (_, i) =>
    ((seed + i) & 0xff).toString(16).padStart(2, "0").toUpperCase(),
  ).join(":");

test("wasm session: wrong PSK is rejected end to end", () => {
  const cfg = (psk) =>
    JSON.stringify({
      role: "parent",
      room_id: "r",
      psk_b64url: psk,
      heartbeat_interval_ms: 5000,
      heartbeat_stale_ms: 15000,
      pairing_timeout_ms: 15000,
      noise_threshold: 0.2,
      noise_hold_ms: 300,
    });
  const s = new wasm.WasmSession(cfg("AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8"));
  const step = (input) => JSON.parse(s.handle(Date.now(), JSON.stringify(input)));
  step({ type: "signaling_message", text: '{"type":"welcome","clientId":"p","roomId":"r"}' });
  step({
    type: "signaling_message",
    text: JSON.stringify({
      type: "signal",
      from: "b",
      payload: {
        type: "sdp",
        aid: "00112233445566778899aabbccddeeff",
        sdp: { type: "offer", sdp: `a=fingerprint:sha-256 ${fingerprint(0xaa)}\r\n` },
      },
    }),
  });
  step({
    type: "local_description",
    sdp_type: "answer",
    sdp: `a=fingerprint:sha-256 ${fingerprint(0xbb)}\r\n`,
  });
  step({ type: "data_channel_open" });
  const out = step({
    type: "data_channel_message",
    text: JSON.stringify({ type: "pair_proof", v: 3, mac: "f".repeat(64) }),
  });
  assert.ok(
    out.some(
      (a) =>
        a.type === "emit" && a.event.type === "failed" && a.event.reason === "pair_proof_mismatch",
    ),
  );
});

test("wasm link: parent joins, asks the baby for an offer and exposes the PSK-derived room", () => {
  const psk = "q83vEjRWeJCrze8SNFZ4kKvN7xI0VniQq83vEjRWeJA";
  const link = new wasm.WasmLink(
    JSON.stringify({
      role: "parent",
      psk_b64url: psk,
      paired_before: false,
      heartbeat_interval_ms: 10000,
      alarm_after_ms: 60000,
      noise_threshold: 0.2,
      noise_hold_ms: 300,
    }),
  );
  assert.equal(link.roomId(), "J8uOkdP8JbRE_-Q9zX1rxA");
  const step = (now, input) => JSON.parse(link.handle(now, JSON.stringify(input)));
  const first = step(0, { type: "start", entropy: 1 });
  assert.equal(first[0].type, "open_signaling");
  step(1, { type: "signaling_opened" });
  step(2, {
    type: "signaling_message",
    text: JSON.stringify({ type: "welcome", clientId: "p", roomId: link.roomId() }),
  });
  const out = step(3, {
    type: "signaling_message",
    text: JSON.stringify({
      type: "members",
      peers: [
        { clientId: "b", role: "baby" },
        { clientId: "p", role: "parent" },
      ],
    }),
  });
  const sent = JSON.parse(out.find((a) => a.type === "send_signaling").text);
  assert.equal(sent.to, "b");
  assert.equal(sent.payload.type, "request_offer");
  assert.match(sent.payload.aid, /^[0-9a-f]{32}$/);
});

test("wasm: message keys and room derivation", () => {
  const keys = JSON.parse(wasm.messageKeys());
  for (const k of [
    "pairing_key_mismatch",
    "update_required",
    "server_unreachable",
    "peer_absent",
    "media_blocked",
    "wrong_key_attempt",
  ]) {
    assert.ok(keys.includes(k), k);
  }
  assert.equal(
    wasm.deriveRoomId(Uint8Array.from({ length: 32 }, (_, i) => i)),
    "FrcBBWi_uGBOkaphqPhl_w",
  );
});
