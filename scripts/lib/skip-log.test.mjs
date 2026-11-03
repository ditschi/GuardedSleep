import assert from "node:assert/strict";
import { chmodSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const bash = (script, env = {}) =>
  spawnSync("bash", ["-c", script], {
    cwd: root,
    encoding: "utf8",
    env: { ...process.env, GS_SKIP_LOG: "", ...env },
  });

test("gs_skip prints the line and only appends when GS_SKIP_LOG is set", () => {
  const dir = mkdtempSync(join(tmpdir(), "skip-log-"));
  const log = join(dir, "log");
  const r = bash("source scripts/lib/skip-log.sh; gs_skip 'gate needs kvm'", { GS_SKIP_LOG: log });
  assert.equal(r.stdout, "SKIPPED: gate needs kvm\n");
  assert.equal(readFileSync(log, "utf8"), "SKIPPED: gate needs kvm\n");
  const none = bash("source scripts/lib/skip-log.sh; gs_skip 'x'");
  assert.equal(none.status, 0);
  assert.equal(none.stdout, "SKIPPED: x\n");
});

test("gs_skip_summary lists distinct lines or says nothing was skipped", () => {
  const dir = mkdtempSync(join(tmpdir(), "skip-log-"));
  const log = join(dir, "log");
  writeFileSync(log, "SKIPPED: a needs b\nSKIPPED: c needs d\nSKIPPED: a needs b\n");
  const some = bash(`source scripts/lib/skip-log.sh; gs_skip_summary '${log}'`);
  assert.match(
    some.stdout,
    /Skipped tasks \(not a pass\):\n {2}SKIPPED: a needs b\n {2}SKIPPED: c needs d\n$/,
  );
  writeFileSync(log, "");
  assert.match(
    bash(`source scripts/lib/skip-log.sh; gs_skip_summary '${log}'`).stdout,
    /No tasks skipped\./,
  );
});

// A stand-in for `just`: prints a skip for lint, fails at the step named by FAIL_AT, logs the order.
function fakeJust(order) {
  const dir = mkdtempSync(join(tmpdir(), "skip-log-"));
  const bin = join(dir, "just");
  writeFileSync(
    bin,
    `#!/usr/bin/env bash
source "${root}/scripts/lib/skip-log.sh"
echo "$1" >> "${order}"
[ "$1" = lint ] && gs_skip "api-check needs docker"
[ "$1" = "\${FAIL_AT:-}" ] && exit 7
exit 0
`,
  );
  chmodSync(bin, 0o755);
  return bin;
}

test("ci.sh runs the steps in order and lists the skips at the end", () => {
  const order = join(mkdtempSync(join(tmpdir(), "skip-log-")), "order");
  const r = spawnSync(join(root, "scripts/lib/ci.sh"), [fakeJust(order)], { encoding: "utf8" });
  assert.equal(r.status, 0);
  assert.deepEqual(readFileSync(order, "utf8").trim().split("\n"), [
    "lint",
    "test",
    "build",
    "e2e",
  ]);
  assert.match(r.stdout, /Skipped tasks \(not a pass\):\n {2}SKIPPED: api-check needs docker\n/);
});

test("ci.sh stops at the first failing step, keeps its status and still prints the summary", () => {
  const order = join(mkdtempSync(join(tmpdir(), "skip-log-")), "order");
  const r = spawnSync(join(root, "scripts/lib/ci.sh"), [fakeJust(order)], {
    encoding: "utf8",
    env: { ...process.env, FAIL_AT: "test" },
  });
  assert.equal(r.status, 7);
  assert.deepEqual(readFileSync(order, "utf8").trim().split("\n"), ["lint", "test"]);
  assert.match(r.stdout, /SKIPPED: api-check needs docker/);
});
