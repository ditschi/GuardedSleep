// Shared helpers of the relay measurement scripts (alpha-connection-model task 4.1).
// Node >= 22 only: global WebSocket and fetch, no dependencies. Every wait has a deadline.
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");

/** Nearest-rank percentile (p in 0..100) of numbers; null for an empty list. */
export function percentile(values, p) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const rank = Math.max(1, Math.ceil((p / 100) * sorted.length));
  return sorted[rank - 1];
}

export const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

export function readData(dataDir, name) {
  return JSON.parse(readFileSync(join(dataDir, name), "utf8"));
}

/** Minimal `--flag value` / `--flag` parser; unknown flags are an error. */
export function parseArgs(argv, spec) {
  const out = {};
  for (const [key, def] of Object.entries(spec)) out[key] = def.default;
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    const key = arg.startsWith("--") ? arg.slice(2) : null;
    const def = key && spec[key];
    if (!def) throw new Error(`unknown argument ${arg}`);
    if (def.flag) {
      out[key] = true;
    } else {
      const value = argv[++i];
      if (value === undefined) throw new Error(`${arg} needs a value`);
      out[key] = def.number ? Number(value) : value;
    }
  }
  return out;
}

export function writeReport(dir, name, json, markdown) {
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, `${name}.json`), `${JSON.stringify(json, null, 2)}\n`);
  writeFileSync(join(dir, `${name}.md`), markdown);
}

// ---- signing through the core -------------------------------------------------------------

/**
 * The signer is `gs-soak sign` (BIP-340 through the Rust core, so no second Schnorr implementation
 * exists here). `GS_SOAK_BIN` points at a built binary; otherwise target/{release,debug}/gs-soak,
 * otherwise `scripts/cargo.sh run` (slow the first time, builds in the Docker image if needed).
 */
export function signerCommand() {
  if (process.env.GS_SOAK_BIN) return [process.env.GS_SOAK_BIN, []];
  for (const profile of ["release", "debug"]) {
    const bin = join(root, "target", profile, "gs-soak");
    if (existsSync(bin)) return [bin, []];
  }
  return [join(root, "scripts", "cargo.sh"), ["run", "-q", "-p", "gs-soak", "--"]];
}

/** Signed throwaway events: `specs` are "BYTES" or "BYTES:OFFSET_S"; returns the parsed lines. */
export function signEvents(tag, specs) {
  const [cmd, prefix] = signerCommand();
  const args = [...prefix, "sign", ...(tag ? ["--tag", tag] : []), ...specs];
  const out = execFileSync(cmd, args, { encoding: "utf8", timeout: 120_000, cwd: root });
  return out
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

// ---- WebSocket ----------------------------------------------------------------------------

/** One relay connection; frames go to the handlers as parsed arrays. */
export class Conn {
  constructor(url) {
    this.url = url;
    this.handlers = [];
    this.closed = false;
    this.openedAt = null;
  }

  /** Resolves with the connect time in ms, rejects after `timeoutMs`. */
  open(timeoutMs = 10_000) {
    return new Promise((resolve, reject) => {
      const started = performance.now();
      let settled = false;
      const done = (fn, value) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        fn(value);
      };
      const timer = setTimeout(() => {
        this.close();
        done(reject, new Error("connect timeout"));
      }, timeoutMs);
      try {
        this.ws = new WebSocket(this.url);
      } catch (e) {
        done(reject, new Error(`connect: ${e.message}`));
        return;
      }
      this.ws.addEventListener("open", () => {
        this.openedAt = performance.now();
        done(resolve, this.openedAt - started);
      });
      this.ws.addEventListener("error", () => done(reject, new Error("connect error")));
      this.ws.addEventListener("close", () => {
        this.closed = true;
        done(reject, new Error("closed before open"));
        for (const h of this.handlers) h(null);
      });
      this.ws.addEventListener("message", (event) => {
        let frame = null;
        try {
          frame = JSON.parse(String(event.data));
        } catch {
          return;
        }
        for (const h of this.handlers) h(frame, performance.now());
      });
    });
  }

  /** `handler(frame, atMs)`; `frame` is null when the socket closed. Returns an unsubscribe. */
  on(handler) {
    this.handlers.push(handler);
    return () => {
      this.handlers = this.handlers.filter((h) => h !== handler);
    };
  }

  send(text) {
    if (this.closed) return false;
    this.ws.send(text);
    return true;
  }

  close() {
    this.closed = true;
    try {
      this.ws?.close();
    } catch {
      // Already closed.
    }
  }

  /** Waits for a frame the predicate accepts; resolves null on timeout or close. */
  waitFor(predicate, timeoutMs) {
    return new Promise((resolve) => {
      const off = this.on((frame, at) => {
        if (frame === null) {
          done(null);
        } else if (predicate(frame)) {
          done({ frame, at });
        }
      });
      const timer = setTimeout(() => done(null), timeoutMs);
      const done = (value) => {
        clearTimeout(timer);
        off();
        resolve(value);
      };
    });
  }
}

/** Subscribes (kind + `#t` tag, optionally an author) and waits for EOSE; false when it fails. */
export async function subscribe(conn, subId, kind, tag, { author, sinceOffsetS = -60 } = {}) {
  const filter = {
    kinds: [kind],
    "#t": [tag],
    since: Math.floor(Date.now() / 1000) + sinceOffsetS,
  };
  if (author) filter.authors = [author];
  const wait = conn.waitFor((f) => f[0] === "EOSE" && f[1] === subId, 8_000);
  conn.send(JSON.stringify(["REQ", subId, filter]));
  return (await wait) !== null;
}

// ---- output -------------------------------------------------------------------------------

export function table(header, rows) {
  const line = (cells) => `| ${cells.join(" | ")} |`;
  return [line(header), line(header.map(() => "---")), ...rows.map(line)].join("\n") + "\n";
}
