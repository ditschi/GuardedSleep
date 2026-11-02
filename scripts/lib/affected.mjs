// Delta tests: maps the changed paths to areas (.github/path-filters.yml, the same map CI uses) and
// runs only the matching test recipes (AGENTS.md section 10.1). Conservative: anything global or
// unmapped runs the full `just test`.
//   node scripts/lib/affected.mjs [--dry-run] [base]     # base default: origin/main
// Changed files = committed since the merge base with <base>, plus uncommitted and untracked files.
// No dependencies: the path map is read with a tiny line parser (flat `name:` + `- "glob"` lists),
// globs are matched with path.matchesGlob (stable without a warning on the pinned Node).
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, matchesGlob } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");

/** Strips a trailing comment and surrounding quotes of one list item. */
function unquote(raw) {
  const v = raw.trim();
  const m = /^(["'])(.*?)\1/.exec(v);
  return m ? m[2] : v.replace(/\s+#.*$/, "");
}

/** Parses `name:` headings with `- "glob"` items; comments and blank lines are skipped. */
export function parseFilters(text) {
  const filters = {};
  let current = null;
  for (const line of text.split("\n")) {
    if (/^\s*(#.*)?$/.test(line)) continue;
    const head = /^([A-Za-z0-9_-]+):\s*(#.*)?$/.exec(line);
    if (head) {
      current = head[1];
      filters[current] = [];
      continue;
    }
    const item = /^\s+-\s+(.*)$/.exec(line);
    if (item && current) {
      filters[current].push(unquote(item[1]));
      continue;
    }
    throw new Error(`unsupported line in path filters: ${line}`);
  }
  return filters;
}

/** The `paths:` list of one event of a workflow file (`on.<event>.paths`), by indentation. */
export function parseWorkflowPaths(text, event = "pull_request") {
  const lines = text.split("\n");
  const start = lines.findIndex((l) => new RegExp(`^  ${event}:\\s*$`).test(l));
  if (start < 0) throw new Error(`no on.${event} in workflow`);
  const paths = [];
  let inPaths = false;
  for (const line of lines.slice(start + 1)) {
    if (/^\s*(#.*)?$/.test(line)) continue;
    if (!/^ {3,}/.test(line)) break; // next key of `on:` or later
    if (/^ {4}paths:\s*$/.test(line)) inPaths = true;
    else if (/^ {4}\S/.test(line)) inPaths = false;
    else if (inPaths) {
      const item = /^ {6}-\s+(.*)$/.exec(line);
      if (item) paths.push(unquote(item[1]));
    }
  }
  return paths;
}

/** A path is in an area if it matches any non-negated glob (negations only matter for `code`). */
export function inArea(file, globs) {
  return globs.some((g) => !g.startsWith("!") && matchesGlob(file, g));
}

export function classify(files, filters) {
  const areas = {};
  for (const name of Object.keys(filters)) {
    if (name === "code") continue; // "everything except docs": CI only
    areas[name] = files.filter((f) => inArea(f, filters[name]));
  }
  return areas;
}

const MAPPED = ["rust", "web", "android", "e2e", "gate", "global", "docs"];

/**
 * @param {{files: string[], filters: Record<string, string[]>, mergeBase: string,
 *   vitestPackages: string[], fullReason?: string}} input
 * @returns {{areas: Record<string, string[]>, notes: string[], commands: string[][]}}
 */
export function makePlan({ files, filters, mergeBase, vitestPackages, fullReason }) {
  const areas = classify(files, filters);
  const notes = [];
  const full = (reason) => {
    notes.push(`full: ${reason}`);
    return { areas, notes, commands: [["just", "test"]] };
  };
  if (fullReason) return full(fullReason);
  if (files.length === 0) {
    notes.push("nothing to test: no changed files");
    return { areas, notes, commands: [] };
  }
  const area = (n) => areas[n] ?? [];
  if (area("global").length > 0) {
    return full(`global path changed (${area("global").slice(0, 3).join(", ")})`);
  }
  const unmapped = files.filter((f) => !MAPPED.some((n) => area(n).includes(f)));
  if (unmapped.length > 0) {
    return full(`no area for ${unmapped.slice(0, 3).join(", ")}`);
  }

  const commands = [];
  const rust = area("rust").length > 0;
  const web = area("web").length > 0;
  const android = area("android").length > 0;
  const locales = area("web").some((f) => matchesGlob(f, "locales/**"));
  if (rust || web) commands.push(["just", "_js-deps"]);
  if (rust) {
    commands.push(["just", "test-rust"], ["just", "test-wasm"]); // test-wasm runs _js-gen
  } else if (web) {
    commands.push(["just", "_js-gen"]); // Vitest imports the generated WASM package
  }
  if (rust || locales) {
    commands.push(["just", "test-web"]);
    if (!rust) notes.push("web: full Vitest because locales/** changed");
  } else if (web) {
    for (const pkg of vitestPackages) {
      commands.push([
        "pnpm",
        "--filter",
        pkg,
        "exec",
        "vitest",
        "run",
        "--changed",
        mergeBase,
        "--passWithNoTests",
      ]);
    }
  }
  if (rust) notes.push("rust: full web and android tests (the core feeds both)");
  if (android || rust) commands.push(["just", "test-android"]);
  if (android || rust) notes.push("android: needs `just build-android` in the full sweep");
  if (area("e2e").length > 0) notes.push("e2e: needs `just e2e` in the full sweep");
  if (area("gate").length > 0) notes.push("gate: needs `just gate` in the full sweep");
  if (commands.length === 0) notes.push("nothing to test: documentation only");
  return { areas, notes, commands };
}

/** Workspace packages (apps/*, packages/*) whose `test` script runs Vitest. */
export function findVitestPackages(repoRoot = root) {
  const names = [];
  for (const dir of ["apps", "packages"]) {
    const base = join(repoRoot, dir);
    if (!existsSync(base)) continue;
    for (const entry of readdirSync(base)) {
      const file = join(base, entry, "package.json");
      if (!existsSync(file)) continue;
      const pkg = JSON.parse(readFileSync(file, "utf8"));
      if (pkg.name && /\bvitest\b/.test(pkg.scripts?.test ?? "")) names.push(pkg.name);
    }
  }
  return names.sort();
}

const git = (...args) => execFileSync("git", args, { cwd: root, encoding: "utf8" });
const lines = (out) => out.split("\n").filter(Boolean);

function changedFiles(base) {
  try {
    git("rev-parse", "--verify", "--quiet", `${base}^{commit}`);
  } catch {
    return { fullReason: `base ref ${base} not found`, files: [], mergeBase: "" };
  }
  const mergeBase = git("merge-base", base, "HEAD").trim();
  const files = new Set([
    ...lines(git("diff", "--name-only", mergeBase)),
    ...lines(git("diff", "--name-only", "HEAD")),
    ...lines(git("ls-files", "--others", "--exclude-standard")),
  ]);
  return { files: [...files].sort(), mergeBase };
}

function main(argv) {
  const dryRun = argv.includes("--dry-run");
  const base = argv.find((a) => !a.startsWith("--")) ?? "origin/main";
  const filters = parseFilters(readFileSync(join(root, ".github", "path-filters.yml"), "utf8"));
  const { files, mergeBase, fullReason } = changedFiles(base);
  const plan = makePlan({
    files,
    filters,
    mergeBase,
    vitestPackages: findVitestPackages(),
    fullReason,
  });

  console.log(
    `base ${base}${mergeBase ? ` (merge base ${mergeBase.slice(0, 8)})` : ""}, ${files.length} changed files`,
  );
  console.log("areas:");
  for (const [name, hit] of Object.entries(plan.areas)) {
    if (hit.length > 0) console.log(`  ${name}: ${hit.length} file(s)`);
  }
  for (const note of plan.notes) console.log(`note: ${note}`);
  console.log("plan:");
  if (plan.commands.length === 0) console.log("  (nothing)");
  for (const c of plan.commands) console.log(`  ${c.join(" ")}`);
  if (dryRun) return 0;
  for (const [cmd, ...args] of plan.commands) {
    console.log(`\n$ ${[cmd, ...args].join(" ")}`);
    const r = spawnSync(cmd, args, { cwd: root, stdio: "inherit" });
    if (r.status !== 0) return r.status ?? 1;
  }
  return 0;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  process.exitCode = main(process.argv.slice(2));
}
