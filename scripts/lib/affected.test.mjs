import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import {
  classify,
  findVitestPackages,
  inArea,
  makePlan,
  parseFilters,
  parseWorkflowPaths,
} from "./affected.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const read = (p) => readFileSync(join(root, p), "utf8");
const filters = parseFilters(read(".github/path-filters.yml"));
const vitestPackages = ["@guardedsleep/core-wasm", "@guardedsleep/mobile", "@guardedsleep/web"];
const classifyGlobs = () =>
  Object.fromEntries(Object.entries(filters).filter(([n]) => n !== "code"));
const plan = (files) => makePlan({ files, filters, mergeBase: "abc123", vitestPackages });
const cmds = (p) => p.commands.map((c) => c.join(" "));

test("loads the real path map", () => {
  for (const n of ["code", "android", "rust", "web", "e2e", "gate", "global", "docs"]) {
    assert.ok(filters[n]?.length > 0, n);
  }
  assert.deepEqual(filters.code, ["**", "!docs/**", "!**/*.md", "!LICENSE"]);
});

test("parseFilters handles comments, quotes and rejects other syntax", () => {
  assert.deepEqual(parseFilters("# c\na:\n  - \"x/**\" # why\n  - y\n\nb:\n  - 'z'\n"), {
    a: ["x/**", "y"],
    b: ["z"],
  });
  assert.throws(() => parseFilters("a: [x]\n"));
});

test("glob matching", () => {
  assert.ok(inArea("crates/a/b.rs", ["crates/**"]));
  assert.ok(inArea("README.md", filters.docs));
  assert.ok(inArea("a/b/c.md", filters.docs));
  assert.ok(!inArea("newdir/x", Object.values(classifyGlobs()).flat()));
  assert.ok(!inArea("docs/a.txt", ["!docs/**"]));
  assert.ok(inArea("justfile", ["justfile"]));
  assert.ok(!inArea("scripts/other.sh", ["scripts/lib/**"]));
});

test("classify skips the code filter", () => {
  assert.equal(classify(["x"], filters).code, undefined);
});

test("plan: nothing, docs only", () => {
  assert.deepEqual(plan([]).commands, []);
  const p = plan(["docs/a.md", "openspec/x/y.md"]);
  assert.deepEqual(p.commands, []);
  assert.ok(p.notes.some((n) => n.includes("documentation only")));
});

test("plan: web only uses Vitest --changed per package", () => {
  const p = plan(["apps/mobile/src/x.ts"]);
  assert.deepEqual(cmds(p), [
    "just _js-deps",
    "just _js-gen",
    "pnpm --filter @guardedsleep/core-wasm exec vitest run --changed abc123 --passWithNoTests",
    "pnpm --filter @guardedsleep/mobile exec vitest run --changed abc123 --passWithNoTests",
    "pnpm --filter @guardedsleep/web exec vitest run --changed abc123 --passWithNoTests",
  ]);
  assert.ok(p.notes.some((n) => n.startsWith("e2e:")));
});

test("plan: apps/web belongs to the web and e2e areas", () => {
  const p = plan(["apps/web/src/lib/router.ts"]);
  assert.deepEqual(p.areas.web, ["apps/web/src/lib/router.ts"]);
  assert.deepEqual(p.areas.e2e, ["apps/web/src/lib/router.ts"]);
  assert.ok(cmds(p).some((c) => c.startsWith("pnpm --filter @guardedsleep/web exec vitest")));
});

test("plan: locales run the full web tests", () => {
  assert.ok(cmds(plan(["locales/en.json"])).includes("just test-web"));
});

test("plan: crates run rust, wasm, web and android", () => {
  const p = plan(["crates/guardedsleep-core/src/lib.rs"]);
  assert.deepEqual(cmds(p), [
    "just _js-deps",
    "just test-rust",
    "just test-wasm",
    "just test-web",
    "just test-android",
  ]);
  assert.ok(p.notes.some((n) => n.startsWith("gate:")));
});

test("plan: android only", () => {
  assert.deepEqual(cmds(plan(["apps/android/app/src/A.kt"])), ["just test-android"]);
});

test("plan: unmapped path and global path fall back to the full run", () => {
  for (const f of ["newdir/x", "justfile", ".github/workflows/ci.yml"]) {
    const p = plan([f, "docs/a.md"]);
    assert.deepEqual(cmds(p), ["just test"], f);
    assert.ok(p.notes[0].startsWith("full:"));
  }
});

test("plan: unknown base ref falls back to the full run", () => {
  const p = makePlan({
    files: [],
    filters,
    mergeBase: "",
    vitestPackages,
    fullReason: "base ref x not found",
  });
  assert.deepEqual(cmds(p), ["just test"]);
});

test("the gate filter equals on.pull_request.paths of android-native.yml", () => {
  const wf = parseWorkflowPaths(read(".github/workflows/android-native.yml"), "pull_request");
  assert.ok(wf.length > 5);
  assert.deepEqual(filters.gate, wf);
});

test("finds the Vitest packages of the workspace", () => {
  const found = findVitestPackages();
  assert.ok(found.includes("@guardedsleep/core-wasm"));
  assert.ok(!found.includes("@guardedsleep/e2e"));
});
