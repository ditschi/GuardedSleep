// Keeps packages/core-wasm/pkg{,-node} (gitignored wasm-bindgen output) in step with the Rust sources.
//   node scripts/lib/core-wasm-fresh.mjs ensure   # rebuild only when the inputs changed since the last build
//   node scripts/lib/core-wasm-fresh.mjs build    # always rebuild (the @guardedsleep/core-wasm build script)
//   node scripts/lib/core-wasm-fresh.mjs status   # print fresh/stale and exit 0/1, builds nothing
// Freshness is a SHA-256 over the build inputs, stored next to the output after a successful build. A content
// hash rather than mtimes: checkouts and rebases touch mtimes without changing content, and a pkg copied from
// another tree may be newer than sources it was not built from. Hashing the inputs (< 1 MB) takes milliseconds.
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative, sep } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
export const ROOT = join(here, "..", "..");

// Files or directories (walked recursively). A missing path is hashed as absent, so adding e.g. a build.rs
// later invalidates the stamp without editing this list. This file is listed so a changed list rebuilds.
export const INPUTS = [
  "Cargo.toml",
  "Cargo.lock",
  "rust-toolchain.toml",
  "crates/guardedsleep-core/Cargo.toml",
  "crates/guardedsleep-core/build.rs",
  "crates/guardedsleep-core/src",
  "crates/guardedsleep-core/vectors",
  "crates/guardedsleep-wasm/Cargo.toml",
  "crates/guardedsleep-wasm/build.rs",
  "crates/guardedsleep-wasm/src",
  "scripts/core-build.sh",
  "packages/core-wasm/scripts/copy-pkg.mjs",
  "scripts/lib/core-wasm-fresh.mjs",
];

export const OUTPUTS = [
  "packages/core-wasm/pkg/guardedsleep_wasm_bg.wasm",
  "packages/core-wasm/pkg-node/guardedsleep_wasm.js",
];

// Inside pkg-node, so `copy-pkg.mjs` (which recreates the directory) drops it before a new one is written.
export const STAMP = "packages/core-wasm/pkg-node/.inputs.sha256";

function listFiles(root, rel) {
  const abs = join(root, rel);
  if (!existsSync(abs)) return [];
  if (!statSync(abs).isDirectory()) return [rel];
  return readdirSync(abs, { recursive: true, withFileTypes: true })
    .filter((e) => e.isFile())
    .map((e) => relative(root, join(e.parentPath, e.name)).split(sep).join("/"));
}

export function inputsHash(root = ROOT, inputs = INPUTS) {
  const h = createHash("sha256");
  for (const input of [...inputs].sort()) {
    const files = listFiles(root, input).sort();
    h.update(`${input}\0${files.length === 0 ? "absent" : files.length}\0`);
    for (const file of files) {
      const data = readFileSync(join(root, file));
      h.update(`${file}\0${data.length}\0`);
      h.update(data);
    }
  }
  return h.digest("hex");
}

export function readStamp(root = ROOT) {
  try {
    return readFileSync(join(root, STAMP), "utf8").trim();
  } catch {
    return null;
  }
}

export function writeStamp(root, hash) {
  writeFileSync(join(root, STAMP), `${hash}\n`);
}

/** `{ fresh, reason }`; `reason` says why a rebuild is needed. */
export function freshness(root = ROOT, inputs = INPUTS) {
  const missing = OUTPUTS.filter((o) => !existsSync(join(root, o)));
  if (missing.length > 0) return { fresh: false, reason: `missing ${missing.join(", ")}` };
  const stamp = readStamp(root);
  if (stamp === null) return { fresh: false, reason: "no input stamp (built before stamping)" };
  if (stamp !== inputsHash(root, inputs)) return { fresh: false, reason: "Rust inputs changed" };
  return { fresh: true, reason: "up to date" };
}

function run(cmd, args, cwd) {
  const r = spawnSync(cmd, args, { cwd, stdio: "inherit" });
  if (r.error) throw r.error;
  if (r.status !== 0) process.exit(r.status ?? 1);
}

// The hash is taken before the build: a source edit during the build then leaves a stale stamp (one extra
// rebuild next time) instead of a fresh stamp over an old pkg.
export function build(root = ROOT) {
  const hash = inputsHash(root);
  run("bash", ["scripts/core-build.sh", "wasm"], root);
  run("node", ["packages/core-wasm/scripts/copy-pkg.mjs"], root);
  writeStamp(root, hash);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const cmd = process.argv[2];
  if (cmd === "build") {
    build();
  } else if (cmd === "ensure" || cmd === "status") {
    const { fresh, reason } = freshness();
    if (cmd === "status") {
      console.log(`core-wasm: ${fresh ? "fresh" : `stale (${reason})`}`);
      process.exit(fresh ? 0 : 1);
    }
    if (!fresh) {
      console.log(`core-wasm: rebuilding, ${reason}`);
      build();
    }
  } else {
    console.error("usage: core-wasm-fresh.mjs ensure|build|status");
    process.exit(2);
  }
}
