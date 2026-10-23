// Copies the wasm-bindgen output of `scripts/core-build.sh wasm` into this package (both gitignored); run by
// scripts/lib/core-wasm-fresh.mjs, which stamps the result with a hash of the Rust inputs.
//   target/wasm-pkg/web  -> pkg/       (browser ESM, loaded by ./web)
//   target/wasm-pkg/node -> pkg-node/  (CommonJS, loaded by ./node for Vitest and Node tools)
import { cpSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const pkgRoot = join(here, "..");
const wasmOut = join(pkgRoot, "..", "..", "target", "wasm-pkg");

for (const [from, to] of [
  ["web", "pkg"],
  ["node", "pkg-node"],
]) {
  const dest = join(pkgRoot, to);
  rmSync(dest, { recursive: true, force: true });
  mkdirSync(dest, { recursive: true });
  cpSync(join(wasmOut, from), dest, { recursive: true });
}
// The package is "type": "module"; the nodejs target emits CommonJS.
writeFileSync(join(pkgRoot, "pkg-node", "package.json"), '{ "type": "commonjs" }\n');
console.log("core-wasm: copied wasm-pkg/web -> pkg, wasm-pkg/node -> pkg-node");
