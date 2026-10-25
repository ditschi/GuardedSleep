// Measures every active entry of data/nostr-relays.json (NIP-11, connect, delivery, sizes,
// retention, created_at tolerance, burst) and every active STUN server of data/stun-servers.json.
//   node scripts/spike/relays-check.mjs [--data data] [--out .cache/relays-check] [--only host,host]
//                                       [--events 20] [--quick] [--concurrency 4] [--no-stun]
// Writes relays-check.json and relays-check.md into --out and prints the table. Exit code 1 when an
// active relay or STUN server fails (see judge() in relay-probe.mjs); re-vet it and mark it
// `suspended` in data/ (docs/contributing/relay-soak.md). A failing relay is probed a second time
// before it counts, because one lost packet is not churn.
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { fetchNip11 } from "./nip11.mjs";
import { judge, probeRelay } from "./relay-probe.mjs";
import { probeStun } from "./stun-probe.mjs";
import { parseArgs, readData, root, table, writeReport } from "./lib.mjs";

const args = parseArgs(process.argv.slice(2), {
  data: { default: "data" },
  out: { default: ".cache/relays-check" },
  only: { default: "" },
  events: { default: 20, number: true },
  concurrency: { default: 4, number: true },
  quick: { flag: true, default: false },
  "no-stun": { flag: true, default: false },
});

/** The signer is the Rust core's event builder; build it once (host cargo or the build image). */
function ensureSigner() {
  if (process.env.GS_SOAK_BIN) return;
  for (const profile of ["release", "debug"]) {
    if (existsSync(join(root, "target", profile, "gs-soak"))) return;
  }
  console.error("building gs-soak (the signer) ...");
  execFileSync(join(root, "scripts", "cargo.sh"), ["build", "-q", "-p", "gs-soak"], {
    stdio: "inherit",
    cwd: root,
    timeout: 540_000,
  });
}

/** Runs `worker` over `items` with at most `limit` in flight. */
async function pool(items, limit, worker) {
  const results = new Array(items.length);
  let next = 0;
  const lanes = Array.from({ length: Math.min(limit, items.length) }, async () => {
    while (next < items.length) {
      const i = next++;
      results[i] = await worker(items[i]);
    }
  });
  await Promise.all(lanes);
  return results;
}

async function checkRelay(entry) {
  const url = `wss://${entry.host}`;
  const options = { events: args.events, quick: args.quick };
  const nip11 = await fetchNip11(entry.host);
  let report = await probeRelay(url, options);
  let verdict = judge(report);
  let attempts = 1;
  if (!verdict.ok && entry.status === "active") {
    report = await probeRelay(url, options);
    verdict = judge(report);
    attempts = 2;
  }
  console.error(
    `${verdict.ok ? "ok  " : "FAIL"} ${entry.host}${verdict.ok ? "" : `: ${verdict.why}`}`,
  );
  return {
    host: entry.host,
    operator: entry.operator,
    status: entry.status,
    listed_software: entry.software,
    nip11,
    attempts,
    ...report,
    verdict,
  };
}

const cell = (v) => (v === null || v === undefined ? "n/a" : String(v));
const mark = (v) => (v === true ? "yes" : v === false ? "no" : "?");

function relayTable(results) {
  const rows = results.map((r) => {
    const d = r.delivery;
    const sizes = r.sizes
      ? Object.entries(r.sizes)
          .map(([bytes, s]) => `${bytes / 1024}K ${s.accepted ? "ok" : "no"}`)
          .join(", ")
      : "n/a";
    const window = r.created_at
      ? Object.entries(r.created_at)
          .filter(([, s]) => s.accepted === false)
          .map(([offset]) => offset)
          .join(" ") || "all accepted"
      : "n/a";
    return [
      r.host,
      r.nip11?.software
        ? `${String(r.nip11.software).split("/").pop()} ${r.nip11.version ?? ""}`.trim()
        : cell(r.listed_software),
      cell(r.connect_ms),
      d ? `${d.delivered}/${d.sent}` : "n/a",
      d ? `${cell(d.p50_ms)} / ${cell(d.p95_ms)}` : "n/a",
      sizes,
      r.retention?.served_to_new_subscription === undefined
        ? "?"
        : mark(r.retention.served_to_new_subscription),
      window,
      r.burst
        ? `${r.burst.accepted}/${r.burst.sent}${r.burst.first_refused_index === null ? "" : ` (refused from #${r.burst.first_refused_index + 1})`}`
        : "skipped",
      r.verdict.ok ? "ok" : `FAIL: ${r.verdict.why}`,
    ];
  });
  return table(
    [
      "Relay",
      "Software",
      "Connect ms",
      "Delivered",
      "p50 / p95 ms",
      "Sizes",
      "Retains",
      "created_at refused (s)",
      "Burst of 12",
      "Verdict",
    ],
    rows,
  );
}

function stunTable(results) {
  return table(
    ["STUN server", "Result", "RTT ms"],
    results.map((r) => [`${r.host}:${r.port}`, r.ok ? "ok" : `FAIL (${r.reason})`, cell(r.rtt_ms)]),
  );
}

ensureSigner();
const only = args.only ? args.only.split(",") : null;
const relays = readData(args.data, "nostr-relays.json").relays.filter(
  (r) => r.status === "active" && (!only || only.includes(r.host)),
);
const stunServers = args["no-stun"]
  ? []
  : readData(args.data, "stun-servers.json").servers.filter(
      (s) => s.status === "active" && (!only || only.includes(s.host)),
    );

const started = new Date().toISOString();
const [relayResults, stunResults] = await Promise.all([
  pool(relays, args.concurrency, checkRelay),
  Promise.all(stunServers.map((s) => probeStun(s))),
]);

const failedRelays = relayResults.filter((r) => !r.verdict.ok);
const failedStun = stunResults.filter((r) => !r.ok);
const summary = {
  relays: relayResults.length,
  relays_ok: relayResults.length - failedRelays.length,
  stun: stunResults.length,
  stun_ok: stunResults.length - failedStun.length,
  failed: [...failedRelays.map((r) => r.host), ...failedStun.map((r) => `${r.host}:${r.port}`)],
};
const json = {
  tool: "relays-check",
  node: process.version,
  started,
  finished: new Date().toISOString(),
  summary,
  relays: relayResults,
  stun: stunResults,
};
const markdown = [
  `# Relay check ${started}`,
  "",
  `${summary.relays_ok} of ${summary.relays} relays and ${summary.stun_ok} of ${summary.stun} STUN servers ok.`,
  summary.failed.length ? `Failing: ${summary.failed.join(", ")}.` : "Nothing failing.",
  "",
  "## Relays",
  "",
  relayTable(relayResults),
  "## STUN",
  "",
  stunTable(stunResults),
].join("\n");
writeReport(resolve(root, args.out), "relays-check", json, markdown);
console.log(markdown);
console.error(`report: ${join(args.out, "relays-check.json")}`);
process.exitCode = summary.failed.length ? 1 : 0;
