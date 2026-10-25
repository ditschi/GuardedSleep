/*
 * The Content-Security-Policy of the web client, in one place (rebuild-web-client design.md,
 * "Browser security"). Two hosts send it as a header (nginx in docker/web, `--serve-web` in
 * crates/guardedsleep-server/src/csp.rs) and index.html carries it as a <meta> fallback; this
 * module builds the <meta> value, and `csp.test.ts` fails when either header drifts from it.
 *
 * `connect-src` is 'self' plus the curated public relays (`generated/relay-origins.json`, written
 * by `public-lists web` from data/nostr-relays.json), the origins listed in `connect-origins.json`
 * and the build-time variable GS_CSP_CONNECT_ORIGINS (comma separated, the own relays of a
 * deployment). A page may only open WebSockets to those origins: a signaling address that is
 * neither the serving host nor listed here is blocked. A blanket `wss:` is never used.
 */
import type { Plugin } from "vite";
import fileOrigins from "./connect-origins.json";
// Generated and git-ignored by `lists:gen` (apps/web); the web scripts run it before dev, build,
// check and test.
import relayOrigins from "./generated/relay-origins.json";

/** The Trusted Types policies the page may create: ours, and the one Svelte creates itself. */
export const TRUSTED_TYPES_POLICIES = ["guardedsleep", "svelte-trusted-html"] as const;

export interface CspOptions {
  /** Extra `ws://` / `wss://` origins for `connect-src`. */
  connectOrigins?: readonly string[];
  /** The Vite dev server: inline styles from its client and WebSockets to localhost are allowed. */
  dev?: boolean;
  /** A <meta> policy: `frame-ancestors` is not allowed there, the headers carry it. */
  meta?: boolean;
}

/** `wss://host[:port]` or `ws://host[:port]`, nothing else (the value goes into a header). */
const ORIGIN = /^wss?:\/\/[A-Za-z0-9.-]+(:\d{1,5})?$/;

export function validateConnectOrigins(origins: readonly string[]): string[] {
  for (const origin of origins) {
    if (!ORIGIN.test(origin)) {
      throw new Error(`CSP connect origin must look like wss://host[:port]: ${origin}`);
    }
  }
  return [...origins];
}

export function buildCsp(options: CspOptions = {}): string {
  const { connectOrigins = [], dev = false, meta = false } = options;
  const connect = dev
    ? ["'self'", "ws://localhost:*", "ws://127.0.0.1:*", ...validateConnectOrigins(connectOrigins)]
    : ["'self'", ...validateConnectOrigins(connectOrigins)];
  const directives = [
    "default-src 'self'",
    "script-src 'self' 'wasm-unsafe-eval'",
    ...(dev ? ["style-src 'self' 'unsafe-inline'"] : []),
    `connect-src ${connect.join(" ")}`,
    "img-src 'self' data: blob:",
    "object-src 'none'",
    "base-uri 'none'",
    ...(meta ? [] : ["frame-ancestors 'none'"]),
    "require-trusted-types-for 'script'",
    `trusted-types ${TRUSTED_TYPES_POLICIES.join(" ")}`,
  ];
  return directives.join("; ");
}

/** The curated relay origins, then those of `connect-origins.json` and `GS_CSP_CONNECT_ORIGINS`. */
export function configuredConnectOrigins(env: Record<string, string | undefined>): string[] {
  const fromEnv = (env.GS_CSP_CONNECT_ORIGINS ?? "")
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s !== "");
  return validateConnectOrigins([
    ...new Set([...relayOrigins.origins, ...fileOrigins.origins, ...fromEnv]),
  ]);
}

/**
 * The build writes the origins it used next to the page under this name; `guardedsleep-server
 * --serve-web` reads it so that its header allows exactly what the <meta> allows.
 */
export const CONNECT_ORIGINS_FILE = "csp-connect-origins.json";

/**
 * The nginx image sends the same policy as a header. nginx cannot compute it, so the build writes
 * the `add_header` line next to the page and the Dockerfile moves it to /etc/nginx (it is not
 * meant to be served).
 */
export const NGINX_CSP_FILE = "csp-header.conf";

export function nginxCspHeader(connectOrigins: readonly string[]): string {
  return `add_header Content-Security-Policy "${buildCsp({ connectOrigins })}" always;\n`;
}

/** Puts the policy into index.html as the first element of <head>. */
export function cspPlugin(): Plugin {
  return {
    name: "guardedsleep-csp",
    generateBundle() {
      const origins = configuredConnectOrigins(process.env);
      this.emitFile({
        type: "asset",
        fileName: CONNECT_ORIGINS_FILE,
        source: `${JSON.stringify({ origins })}\n`,
      });
      this.emitFile({ type: "asset", fileName: NGINX_CSP_FILE, source: nginxCspHeader(origins) });
    },
    transformIndexHtml: {
      order: "pre",
      handler(html, ctx) {
        const value = buildCsp({
          connectOrigins: configuredConnectOrigins(process.env),
          dev: ctx.server !== undefined,
          meta: true,
        });
        // The meta element comes before every script and stylesheet so that it applies to them.
        return html.replace(
          "<head>",
          `<head>\n    <meta http-equiv="Content-Security-Policy" content="${value}" />`,
        );
      },
    },
  };
}
