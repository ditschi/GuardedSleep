/**
 * Whether this build lets the core accept loopback mailboxes (`l:127.0.0.1:...`, `localhost`).
 * Only the Vite dev server (`just dev`) and the browser E2E build (`vite build --mode e2e`, see
 * `apps/e2e/tests/utils/prod-server.ts`) do. The release build, including the web image
 * (`docker/web/Dockerfile` builds with `--mode production`), never does: a release client must
 * refuse an invite that points at loopback.
 * Vite modes: https://vite.dev/guide/env-and-mode#modes
 */
export function loopbackAllowed(env: { DEV: boolean; MODE: string }): boolean {
  return env.DEV || env.MODE === "e2e";
}

/**
 * Release gate for the public Nostr relays (alpha-connection-model task 3.6, security review of
 * 3.5, M7): whether this build puts the curated `n:` relays into the invites it creates. On for the
 * dev server and the E2E build, off for the release build and the web image (`--mode production`).
 * The disclosure and the off switch (tasks 5.3 and 6.2) do not exist yet, and a release client must
 * not send connection metadata to a third party before the user can see and refuse it; the task
 * that adds them flips this to on (design.md section 8). This is the only switch.
 */
export function publicRelaysEnabled(env: { DEV: boolean; MODE: string }): boolean {
  return env.DEV || env.MODE === "e2e";
}
