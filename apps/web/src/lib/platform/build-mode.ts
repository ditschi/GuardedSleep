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
