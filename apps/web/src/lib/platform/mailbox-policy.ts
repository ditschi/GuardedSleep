/**
 * Which mailbox addresses this page may use. The pairing record is the only source of the set
 * (invite and mailbox updates); this module only decides what the page lets through:
 *
 * addresses outside the deployment CSP `connect-src` are ignored (version skew, design section 5:
 * the browser would block them anyway, so the client does not even try and does not tell the host).
 * The off switch ("Only same Wi-Fi or my own server") is the core's, not this module's.
 */

/** The `connect-src` source list of a policy text; empty when it has none. */
export function connectSources(csp: string): string[] {
  for (const directive of csp.split(";")) {
    const [name, ...sources] = directive.trim().split(/\s+/);
    if (name === "connect-src") return sources;
  }
  return [];
}

/** The page's own origin, as far as `'self'` and the dev wildcards need it. */
export interface PageOrigin {
  protocol: string;
  hostname: string;
  port: string;
}

function defaultPort(protocol: string): string {
  return protocol === "wss:" || protocol === "https:" ? "443" : "80";
}

/**
 * Whether `url` matches one of the `connect-src` sources: `'self'` (the page's host and port, with
 * `ws(s)` standing for `http(s)`), `ws(s)://host[:port|:*]` entries. A scheme-less or wildcard-host
 * source is not used by this project and never matches here.
 */
export function cspAllows(sources: readonly string[], page: PageOrigin, url: string): boolean {
  let target: URL;
  try {
    target = new URL(url);
  } catch {
    return false;
  }
  if (target.protocol !== "ws:" && target.protocol !== "wss:") return false;
  const targetPort = target.port === "" ? defaultPort(target.protocol) : target.port;
  const pagePort = page.port === "" ? defaultPort(page.protocol) : page.port;
  const sameScheme =
    (page.protocol === "https:" && target.protocol === "wss:") ||
    (page.protocol === "http:" && target.protocol === "ws:");
  for (const source of sources) {
    if (source === "'self'") {
      if (sameScheme && target.hostname === page.hostname && targetPort === pagePort) return true;
      continue;
    }
    const m = /^(wss?:)\/\/([A-Za-z0-9.-]+|\[[0-9A-Fa-f:.]+\])(?::(\d{1,5}|\*))?$/.exec(source);
    if (!m) continue;
    const [, scheme, host, port] = m;
    if (scheme !== target.protocol || host !== target.hostname) continue;
    if (port === "*" || (port ?? defaultPort(scheme)) === targetPort) return true;
  }
  return false;
}

/** The `connect-src` of the policy the page was served with (its `<meta>` fallback), or null. */
export function deploymentConnectSources(doc: {
  querySelector(selector: string): { getAttribute(name: string): string | null } | null;
}): string[] | null {
  const meta = doc.querySelector('meta[http-equiv="Content-Security-Policy"]');
  const content = meta?.getAttribute("content");
  return content ? connectSources(content) : null;
}

/**
 * Where a new pairing's invite points (baby only) when the project server is not offered:
 * `/signaling` on the server that serves the page (`--serve-web`, the web image, a reverse proxy),
 * or the dev server's companion on 8787. Never a third party.
 */
export function defaultMailboxUrls(
  location: { protocol: string; hostname: string; host: string },
  dev: boolean,
): string[] {
  if (dev) return [`ws://${location.hostname}:8787`];
  // `/signaling` matches the one-domain reverse-proxy layout (docker/public-server/README.md);
  // the server and the bundled nginx image accept the upgrade on any path.
  return [`${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/signaling`];
}
