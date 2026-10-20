/** Milliseconds after TCP connect before `hello` must complete (resource limits). */
export function getHelloTimeoutMs(): number {
  const raw = process.env.SIGNALING_HELLO_TIMEOUT_MS;
  const n = raw !== undefined ? Number.parseInt(raw, 10) : 30_000;
  if (!Number.isFinite(n) || n < 1000 || n > 120_000) return 30_000;
  return n;
}

/** Max parent units per room (one baby + N parents). Bounds DoS via room stuffing. */
export function getMaxParentsPerRoom(): number {
  const raw = process.env.SIGNALING_MAX_PARENTS;
  const n = raw !== undefined ? Number.parseInt(raw, 10) : 1;
  if (!Number.isFinite(n) || n < 1 || n > 256) return 1;
  return n;
}

/** WebSocket ping interval; a socket that missed the previous ping is terminated. */
export function getPingIntervalMs(): number {
  const n = Number.parseInt(process.env.SIGNALING_PING_INTERVAL_MS ?? "", 10);
  if (!Number.isFinite(n) || n < 2000) return 10_000;
  return n;
}

function perMinuteLimit(name: string, fallback: number): number {
  const n = Number.parseInt(process.env[name] ?? "", 10);
  if (!Number.isFinite(n) || n < 0 || n > 100_000) return fallback;
  return n;
}

/** Hello attempts allowed per client IP and minute (0 disables). */
export function getHelloRatePerMin(): number {
  return perMinuteLimit("SIGNALING_HELLO_RATE_PER_MIN", 20);
}

/** New rooms allowed per client IP and minute (0 disables). */
export function getNewRoomRatePerMin(): number {
  return perMinuteLimit("SIGNALING_NEW_ROOM_RATE_PER_MIN", 10);
}

/** Take the client IP from X-Forwarded-For (only behind a proxy that appends the peer address). */
export function getTrustProxy(): boolean {
  return process.env.SIGNALING_TRUST_PROXY === "1";
}

/**
 * Number of trusted proxies in front of the server; the client IP is the n-th hop from the right of
 * X-Forwarded-For (hops further left are client-controlled). Default 1.
 */
export function getTrustedProxyHops(): number {
  const n = Number.parseInt(process.env.SIGNALING_TRUSTED_PROXY_HOPS ?? "", 10);
  return Number.isInteger(n) && n >= 1 ? n : 1;
}

/** Fail fast in production: browser Origin allowlist must be configured (see deploy docs). */
export function assertProductionOriginAllowlist(): void {
  if (process.env.NODE_ENV !== "production") return;
  const raw = process.env.SIGNALING_ALLOWED_ORIGINS?.trim();
  if (!raw) {
    console.error(
      "signaling-server: SIGNALING_ALLOWED_ORIGINS must be set when NODE_ENV=production",
    );
    process.exit(1);
  }
}
