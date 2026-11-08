import { PROJECT_SERVER } from "../public-lists/generated/public-lists";

/**
 * Whether this build lets the core accept loopback mailboxes (`l:127.0.0.1:...`, `localhost`).
 * Only the Vite dev server (`just dev`) and the browser E2E build (`vite build --mode e2e`, see
 * `apps/e2e/tests/utils/prod-server.ts`) do. The release build, including the web image
 * (`docker/web/Dockerfile` builds with `--mode production` by default), never does: a release client
 * must refuse an invite that points at loopback. The `staging` mode (test deployment) does not either.
 * Vite modes: https://vite.dev/guide/env-and-mode#modes
 */
export function loopbackAllowed(env: { DEV: boolean; MODE: string }): boolean {
  return devOrE2e(env);
}

/** The Vite dev server and the `e2e` mode: the builds that are never a release. */
function devOrE2e(env: { DEV: boolean; MODE: string }): boolean {
  return env.DEV || env.MODE === "e2e";
}

/**
 * The `staging` mode: a non-release build of the official origin for the owner's test deployment
 * (`guardedsleep-web-staging`, design section 8). It is the web analogue of an Android debug build:
 * the project server is on, loopback stays off.
 */
function isStaging(env: { MODE: string }): boolean {
  return env.MODE === "staging";
}

/**
 * Whether this web build is the project's official deployment (`VITE_GS_OFFICIAL_DEPLOYMENT=true`
 * at build time). Only such a build lists the project server in its CSP and can use it
 * (alpha-connection-model design section 6).
 */
export function officialDeployment(env: { VITE_GS_OFFICIAL_DEPLOYMENT?: string }): boolean {
  return env.VITE_GS_OFFICIAL_DEPLOYMENT === "true";
}

/**
 * Whether new invites of this build may carry the project server (design section 8). Release
 * builds keep it off until task 6.2's release gate flips it: the disclosure and the user's off
 * switch must exist first, and `data/project-server.json` must be live. Dev, E2E and staging builds
 * (a test deployment of the official origin, not a release) can turn it on, but only together with
 * the official-deployment flag; they never reach the public server otherwise.
 */
export function projectServerEnabled(env: {
  DEV: boolean;
  MODE: string;
  VITE_GS_OFFICIAL_DEPLOYMENT?: string;
}): boolean {
  return officialDeployment(env) && (devOrE2e(env) || isStaging(env));
}

/** The project server's origin, as it goes into a CSP `connect-src`. */
export function projectServerOrigin(): string {
  return `wss://${PROJECT_SERVER.host}`;
}

/** The project server's relay address, as a mailbox URL. */
export function projectServerMailboxUrl(): string {
  return `${projectServerOrigin()}${PROJECT_SERVER.path}`;
}
