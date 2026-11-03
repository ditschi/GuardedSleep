import assert from "node:assert/strict";
import { chmodSync, existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const script = join(root, "scripts", "lib", "reap.sh");

// Fake docker: fixtures come from FAKE_PS / FAKE_NETS ("id|name|image|old|young" lines), calls that
// change state are appended to FAKE_DOCKER_LOG. Fake ps prints nothing so no real process is listed.
const fakeDocker = `#!/usr/bin/env bash
created() { # $1 = id; ids starting with "y" are young
  case "$1" in y*) date -u --iso-8601=seconds ;; *) date -u -d '-3 hours' --iso-8601=seconds ;; esac
}
case "$1 $2" in
  "ps --format") printf '%b' "$FAKE_PS" ;;
  "network ls") printf '%b' "$FAKE_NETS" ;;
  "network inspect") created "\${@: -1}" ;;
  "network rm") echo "network rm $3" >>"$FAKE_DOCKER_LOG"; [ "$3" != "nfail" ] ;;
  "inspect -f") created "\${@: -1}" ;;
  "stop "*|"stop") shift; echo "stop $*" >>"$FAKE_DOCKER_LOG" ;;
  *) exit 0 ;;
esac
`;

const run = (ps, nets, args = []) => {
  const dir = mkdtempSync(join(tmpdir(), "reap-"));
  const bin = join(dir, "bin");
  spawnSync("mkdir", ["-p", bin]);
  writeFileSync(join(bin, "ps"), "#!/usr/bin/env bash\nexit 0\n");
  writeFileSync(join(bin, "docker"), fakeDocker);
  chmodSync(join(bin, "ps"), 0o755);
  chmodSync(join(bin, "docker"), 0o755);
  const log = join(dir, "docker.log");
  const lines = (rows) =>
    rows
      .map((r) => r.join("\t") + "\n")
      .join("")
      .replaceAll("\t", "\\t")
      .replaceAll("\n", "\\n");
  const r = spawnSync("bash", [script, ...args], {
    cwd: root,
    encoding: "utf8",
    timeout: 60000,
    env: {
      ...process.env,
      PATH: `${bin}:${process.env.PATH}`,
      FAKE_PS: lines(ps),
      FAKE_NETS: lines(nets),
      FAKE_DOCKER_LOG: log,
    },
  });
  return { ...r, log: existsSync(log) ? readFileSync(log, "utf8") : "" };
};

// Ids: "o*" old (3 h), "y*" young (just now).
const psAll = [
  ["obaby", "gs-gate-baby", "img:1"],
  ["opoc", "gs-poc-turn", "img:1"],
  ["otl", "gs-turnlife-coturn-abc", "img:1"],
  ["olocal", "guardedsleep-local-web-1", "img:1"],
  ["oimg", "x", "ghcr.io/foo/guardedsleep-signaling:dev"],
  ["yturn", "gs-gate-turn", "img:1"],
  ["opg", "postgres", "postgres:16"],
  ["oother", "gs-other", "img:1"],
];
const netsAll = [
  ["b1", "bridge"],
  ["h1", "host"],
  ["n1", "none"],
  ["onet", "gs-turnlife-net-abc"],
  ["nfail", "guardedsleep-local_default"],
  ["ynet", "gs-gate-young"],
  ["oproxy", "proxy"],
];

test("list mode shows old matching containers and networks only", () => {
  const r = run(psAll, netsAll);
  assert.equal(r.status, 0);
  const out = r.stdout.split("\n").filter(Boolean);
  const kinds = out
    .filter((l) => /^(container|network) /.test(l))
    .map((l) => l.split(" ").slice(0, 2).join(" "));
  assert.deepEqual(kinds, [
    "container obaby",
    "container opoc",
    "container otl",
    "container olocal",
    "container oimg",
    "network onet",
    "network nfail",
  ]);
  assert.match(out.at(-1), /^listed only; run 'just reap --stop'/);
  assert.equal(r.log, "");
});

test("--stop stops the containers, removes the networks and tolerates one in use", () => {
  const r = run(psAll, netsAll, ["--stop"]);
  assert.equal(r.status, 0);
  const calls = r.log.split("\n").filter(Boolean);
  const stop = calls.find((l) => l.startsWith("stop "));
  assert.deepEqual(stop.slice(5).split(" ").sort(), ["obaby", "oimg", "olocal", "opoc", "otl"]);
  assert.deepEqual(calls.filter((l) => l.startsWith("network rm ")).sort(), [
    "network rm nfail",
    "network rm onet",
  ]);
  assert.match(r.stdout, /network guardedsleep-local_default still in use; not removed/);
  assert.doesNotMatch(r.stdout, /network gs-turnlife-net-abc still in use/);
  assert.match(r.stdout, /stopped\n$/);
});

test("nothing matching prints nothing to reap", () => {
  const r = run(
    [
      ["opg", "postgres", "postgres:16"],
      ["oother", "gs-other", "img:1"],
      ["yturn", "gs-gate-turn", "img:1"],
    ],
    [
      ["b1", "bridge"],
      ["oproxy", "proxy"],
    ],
  );
  assert.equal(r.status, 0);
  assert.equal(r.stdout, "nothing to reap\n");
  assert.equal(r.log, "");
});
