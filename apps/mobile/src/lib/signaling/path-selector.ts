/**
 * Path selector for offline-LAN-capable signaling.
 *
 * Tries an ordered list of WebSocket URLs and resolves with the first one that opens
 * within the per-URL timeout. Returns `null` if none of them open in time.
 *
 * Design rationale: [`docs/decisions/ADR-012`](../../../../../docs/decisions/ADR-012-connection-model-offline-lan-and-handoff.md)
 * §2 "Path ranking". The selector is intentionally framework-free so it can run in plain
 * Vitest (jsdom) and so the orchestrator stays in `lib/signaling/` per the layered
 * architecture (see `docs/technical/sw-architecture.md`).
 */
export type SignalingPathCandidate = {
  url: string;
  /** Optional human-readable label for UI/logs ("LAN host", "Internet", "NAS Docker"...). */
  label?: string;
};

export type SelectedSignalingPath = {
  url: string;
  label?: string;
  /** Index into the original candidate list, useful for tests and diagnostics. */
  index: number;
};

export type PathSelectorOptions = {
  /** Per-URL connect timeout in ms (default 4000). */
  perUrlTimeoutMs?: number;
  /**
   * Hook to construct a WebSocket-like object. Defaults to global `WebSocket`. Tests
   * inject a fake constructor; we keep this typed loosely to avoid pulling DOM types
   * into core.
   */
  webSocketFactory?: (url: string) => SignalingProbeSocket;
  /** Inject a clock for deterministic tests (`Date.now`). */
  now?: () => number;
  /** Inject `setTimeout`/`clearTimeout` for deterministic tests. */
  schedule?: {
    setTimeout: (cb: () => void, ms: number) => unknown;
    clearTimeout: (handle: unknown) => void;
  };
};

/** Minimal interface that both `WebSocket` and the test fake implement. */
export type SignalingProbeSocket = {
  addEventListener: (event: "open" | "close" | "error", cb: () => void) => void;
  removeEventListener?: (event: "open" | "close" | "error", cb: () => void) => void;
  close: () => void;
  readyState: number;
};

const READY_STATE_OPEN = 1;

/**
 * Probes one URL by opening a WebSocket and waiting for `open` (success) or
 * `error`/`close` (failure). Times out after `timeoutMs`.
 *
 * On success the underlying socket is **immediately closed** -- the path-selector is a
 * reachability probe, not the actual signaling channel. The caller opens a fresh
 * WebSocket for the selected URL.
 *
 * Why open-then-close vs reuse the open socket: the existing `connectSignaling()`
 * owns the WS lifecycle (hello message, message dispatch). Reusing a probe socket
 * would duplicate that ownership and complicate cancellation.
 */
function probeUrl(
  url: string,
  opts: Required<Pick<PathSelectorOptions, "perUrlTimeoutMs">> &
    Pick<PathSelectorOptions, "webSocketFactory" | "schedule">,
): Promise<boolean> {
  return new Promise((resolve) => {
    let settled = false;
    const finish = (ok: boolean): void => {
      if (settled) return;
      settled = true;
      try {
        ws.close();
      } catch {
        /* swallow -- already closing */
      }
      resolve(ok);
    };

    const ws = opts.webSocketFactory
      ? opts.webSocketFactory(url)
      : (new WebSocket(url) as unknown as SignalingProbeSocket);

    const timeoutHandle = opts.schedule
      ? opts.schedule.setTimeout(() => finish(false), opts.perUrlTimeoutMs)
      : setTimeout(() => finish(false), opts.perUrlTimeoutMs);

    const cleanupTimeout = (): void => {
      if (opts.schedule) opts.schedule.clearTimeout(timeoutHandle);
      else clearTimeout(timeoutHandle as ReturnType<typeof setTimeout>);
    };

    ws.addEventListener("open", () => {
      cleanupTimeout();
      finish(true);
    });
    ws.addEventListener("error", () => {
      cleanupTimeout();
      finish(false);
    });
    ws.addEventListener("close", () => {
      cleanupTimeout();
      if (ws.readyState !== READY_STATE_OPEN) finish(false);
    });
  });
}

/**
 * Try `candidates` in order, return the first that opens within `perUrlTimeoutMs`.
 * Returns `null` if all candidates fail or time out. Empty list returns `null`.
 *
 * Candidates are deduplicated by URL (case-insensitive on host); duplicate later
 * entries are dropped.
 */
export async function selectSignalingPath(
  candidates: ReadonlyArray<SignalingPathCandidate>,
  options: PathSelectorOptions = {},
): Promise<SelectedSignalingPath | null> {
  const perUrlTimeoutMs = options.perUrlTimeoutMs ?? 4000;
  const seen = new Set<string>();
  const probeOpts = {
    perUrlTimeoutMs,
    webSocketFactory: options.webSocketFactory,
    schedule: options.schedule,
  };

  for (let i = 0; i < candidates.length; i += 1) {
    const candidate = candidates[i];
    if (!candidate) continue;
    const key = candidate.url.trim().toLowerCase();
    if (key === "" || seen.has(key)) continue;
    seen.add(key);

    const ok = await probeUrl(candidate.url, probeOpts);
    if (ok) {
      return { url: candidate.url, label: candidate.label, index: i };
    }
  }
  return null;
}

/**
 * Convenience helper: builds the ordered candidate list from a (primary, fallbacks)
 * tuple, filtering out empty entries. Used by the connection orchestrator.
 */
export function buildCandidates(
  primary: string,
  fallbacks: ReadonlyArray<string> | undefined,
  primaryLabel = "primary",
): SignalingPathCandidate[] {
  const list: SignalingPathCandidate[] = [];
  if (primary.trim() !== "") list.push({ url: primary.trim(), label: primaryLabel });
  for (const u of fallbacks ?? []) {
    const t = u.trim();
    if (t !== "") list.push({ url: t, label: "fallback" });
  }
  return list;
}

/**
 * Ordered list for ADR-012 path ranking: LAN prepends first, then primary, then fallbacks.
 * Drops duplicate URLs (case-insensitive) keeping the first occurrence (LAN wins over same URL
 * listed again as primary/fallback).
 */
export function buildPathCandidates(
  prepends: ReadonlyArray<string> | undefined,
  primary: string,
  fallbacks: ReadonlyArray<string> | undefined,
): SignalingPathCandidate[] {
  const tail = buildCandidates(primary, fallbacks, "primary");
  const seen = new Set<string>();
  const out: SignalingPathCandidate[] = [];
  const push = (url: string, label?: string): void => {
    const key = url.trim().toLowerCase();
    if (key === "" || seen.has(key)) return;
    seen.add(key);
    out.push({ url: url.trim(), label });
  };
  for (const u of prepends ?? []) {
    push(u, "lan");
  }
  for (const c of tail) {
    push(c.url, c.label);
  }
  return out;
}
