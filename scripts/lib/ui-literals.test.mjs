import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const script = join(dirname(fileURLToPath(import.meta.url)), "..", "lint-ui-literals.sh");

/** Runs the literal lint on a temporary tree holding the given apps/web/src files. */
function lint(files) {
  const root = mkdtempSync(join(tmpdir(), "ui-literals-"));
  try {
    for (const [name, content] of Object.entries(files)) {
      const path = join(root, "apps", "web", "src", name);
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, content);
    }
    const r = spawnSync("bash", [script], {
      env: { ...process.env, UI_LITERALS_ROOT: root },
      encoding: "utf8",
    });
    return { status: r.status, stderr: r.stderr };
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("svelte: text from the catalogue, ids and code pass", () => {
  const r = lint({
    "Ok.svelte": `<script lang="ts">
  const label = "Plain words in code are fine";
</script>
<style>.a::after { content: "Words in CSS"; }</style>
<!-- A comment with words -->
<main data-testid="parent_listen" aria-label={$t("a.b")}>
  <h1>{$t("a.title")}</h1>
  <p>{count > 1 ? "x" : "y"}</p>
  <button type="button" title="">{$t("a.go")}</button>
  <span> 42 </span>
</main>`,
    "Ok.test.svelte": "<p>Literal words in a test fixture</p>",
  });
  assert.equal(r.status, 0, r.stderr);
});

test("svelte: literal text between tags fails", () => {
  const r = lint({ "Bad.svelte": "<main>\n  <h1>Welcome home</h1>\n</main>" });
  assert.equal(r.status, 1);
  assert.match(r.stderr, /apps\/web\/src\/Bad\.svelte: Welcome home/);
});

test("svelte: literal accessibility attributes fail", () => {
  for (const attr of ["aria-label", "title", "placeholder", "alt", "aria-valuetext"]) {
    const r = lint({ "Bad.svelte": `<img ${attr}="Baby device" />` });
    assert.equal(r.status, 1, attr);
    assert.match(r.stderr, /Baby device/);
  }
});

test("svelte: multi-line text and nested mustaches are handled", () => {
  const ok = lint({ "A.svelte": "<p>\n  {#if a}{$t('x', { n: 1 })}{/if}\n</p>" });
  assert.equal(ok.status, 0, ok.stderr);
  const bad = lint({ "B.svelte": "<p>\n  {#if a}\n    Hello there\n  {/if}\n</p>" });
  assert.equal(bad.status, 1);
});
