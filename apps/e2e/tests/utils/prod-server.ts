import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import { existsSync } from "node:fs";
import { createServer } from "node:net";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

/*
 * The production build of apps/web served by `guardedsleep-server --serve-web`, the way the
 * offline PC mode serves it: with the CSP header, no Vite dev client. The binary is built by
 * `just dev` (the Playwright webServer) before the first test runs.
 */

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "../../../..");
const binary = join(repoRoot, "target/debug/guardedsleep-server");
const dist = join(repoRoot, "apps/web/dist");

export interface ProdServer {
  baseUrl: string;
  stop(): void;
}

async function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const probe = createServer();
    probe.once("error", reject);
    probe.listen(0, "127.0.0.1", () => {
      const address = probe.address();
      const port = typeof address === "object" && address ? address.port : 0;
      probe.close(() => resolve(port));
    });
  });
}

/** Builds apps/web (once per call) and starts the server on a free loopback port. */
export async function startProdServer(): Promise<ProdServer> {
  if (!existsSync(binary)) throw new Error(`${binary} is missing: run \`just dev\` once`);
  execFileSync("pnpm", ["--filter", "@guardedsleep/web", "build"], {
    cwd: repoRoot,
    stdio: "pipe",
    timeout: 240_000,
  });
  const port = await freePort();
  const child: ChildProcess = spawn(
    binary,
    ["--serve-web", dist, "--bind", "127.0.0.1", "--port", String(port), "--no-stun"],
    {
      stdio: "ignore",
      env: {
        ...process.env,
        // Many pairings per minute from one IP, as in playwright.config.ts.
        SIGNALING_HELLO_RATE_PER_MIN: "0",
        SIGNALING_NEW_ROOM_RATE_PER_MIN: "0",
      },
    },
  );
  const baseUrl = `http://127.0.0.1:${port}`;
  for (let i = 0; i < 100; i++) {
    try {
      if ((await fetch(`${baseUrl}/healthz`)).ok) return { baseUrl, stop: () => child.kill() };
    } catch {
      // not listening yet
    }
    await new Promise((r) => setTimeout(r, 100));
  }
  child.kill();
  throw new Error("guardedsleep-server did not start");
}
