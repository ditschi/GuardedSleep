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
  const n = raw !== undefined ? Number.parseInt(raw, 10) : 32;
  if (!Number.isFinite(n) || n < 1 || n > 256) return 32;
  return n;
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
