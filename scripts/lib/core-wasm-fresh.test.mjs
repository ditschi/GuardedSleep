import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import {
  INPUTS,
  OUTPUTS,
  ROOT,
  STAMP,
  freshness,
  inputsHash,
  writeStamp,
} from "./core-wasm-fresh.mjs";

const inputs = ["Cargo.lock", "crates/core/src", "crates/core/build.rs"];

function tree() {
  const root = mkdtempSync(join(tmpdir(), "core-wasm-fresh-"));
  const put = (rel, body) => {
    mkdirSync(dirname(join(root, rel)), { recursive: true });
    writeFileSync(join(root, rel), body);
  };
  put("Cargo.lock", "lock v1\n");
  put("crates/core/src/lib.rs", "pub fn a() {}\n");
  put("crates/core/src/invite/mod.rs", "pub fn parse() {}\n");
  return { root, put, done: () => rmSync(root, { recursive: true, force: true }) };
}

function built(t) {
  for (const o of OUTPUTS) t.put(o, "out");
  writeStamp(t.root, inputsHash(t.root, inputs));
}

test("every listed input exists, except optional build scripts", () => {
  const missing = INPUTS.filter((p) => !p.endsWith("build.rs") && !existsSync(join(ROOT, p)));
  assert.deepEqual(missing, []);
  assert.ok(INPUTS.includes("crates/guardedsleep-core/src"));
  assert.ok(INPUTS.includes("crates/guardedsleep-wasm/src"));
});

test("missing outputs or stamp are stale", () => {
  const t = tree();
  try {
    assert.match(freshness(t.root, inputs).reason, /missing/);
    for (const o of OUTPUTS) t.put(o, "out");
    assert.deepEqual(freshness(t.root, inputs), {
      fresh: false,
      reason: "no input stamp (built before stamping)",
    });
    built(t);
    assert.equal(freshness(t.root, inputs).fresh, true);
    rmSync(join(t.root, OUTPUTS[1]));
    assert.equal(freshness(t.root, inputs).fresh, false);
  } finally {
    t.done();
  }
});

test("content changes are stale, mtime-only changes are not", () => {
  const t = tree();
  try {
    built(t);
    const future = new Date(Date.now() + 60_000);
    utimesSync(join(t.root, "crates/core/src/lib.rs"), future, future);
    assert.equal(freshness(t.root, inputs).fresh, true);

    t.put("crates/core/src/invite/mod.rs", "pub fn parse() {}\npub fn build() {}\n");
    assert.deepEqual(freshness(t.root, inputs), { fresh: false, reason: "Rust inputs changed" });
  } finally {
    t.done();
  }
});

test("added, removed and renamed files change the hash", () => {
  const t = tree();
  try {
    const base = inputsHash(t.root, inputs);
    t.put("crates/core/build.rs", "fn main() {}\n");
    const withBuild = inputsHash(t.root, inputs);
    assert.notEqual(withBuild, base);
    rmSync(join(t.root, "crates/core/build.rs"));
    assert.equal(inputsHash(t.root, inputs), base);

    rmSync(join(t.root, "crates/core/src/invite/mod.rs"));
    t.put("crates/core/src/invite/lib.rs", "pub fn parse() {}\n");
    assert.notEqual(inputsHash(t.root, inputs), base);
  } finally {
    t.done();
  }
});

test("the stamp lives in the directory copy-pkg recreates", () => {
  assert.ok(STAMP.startsWith("packages/core-wasm/pkg-node/"));
});
