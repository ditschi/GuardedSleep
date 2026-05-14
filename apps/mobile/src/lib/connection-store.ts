export type SavedConnection = {
  id: string;
  roomId: string;
  signalingUrl: string;
  /**
   * Optional ordered list of signaling URLs to try when the primary `signalingUrl`
   * is unreachable -- e.g. a user-run LAN Docker signaling, a Tailscale URL, or
   * (in Phase 2 of ADR-012) a baby-side embedded WS listener whose host:port was
   * resolved via NSD. Validation: each entry must be ws:// or wss://; persistence
   * is short-lived per ADR-012 §6 "Security/Privacy" (no long-lived IP histories).
   *
   * See `docs/decisions/ADR-012-connection-model-offline-lan-and-handoff.md` §2.
   */
  fallbackSignalingUrls?: string[];
  lastUsedAt: number;
  label?: string;
};

const STORAGE_KEY = "guardedsleep:saved-connections";
const MAX_CONNECTIONS = 10;

function newId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `${Date.now()}-${Math.random().toString(36).slice(2, 11)}`;
}

function sanitizeFallbacks(input: unknown): string[] | undefined {
  if (!Array.isArray(input)) return undefined;
  const cleaned = input
    .filter((u): u is string => typeof u === "string")
    .map((u) => u.trim())
    .filter((u) => u !== "" && (u.startsWith("ws://") || u.startsWith("wss://")));
  return cleaned.length > 0 ? cleaned : undefined;
}

export function listConnections(): SavedConnection[] {
  if (typeof localStorage === "undefined") return [];
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw == null || raw === "") return [];
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter(
        (row): row is SavedConnection =>
          row != null &&
          typeof row === "object" &&
          typeof (row as SavedConnection).id === "string" &&
          typeof (row as SavedConnection).roomId === "string" &&
          typeof (row as SavedConnection).signalingUrl === "string" &&
          typeof (row as SavedConnection).lastUsedAt === "number",
      )
      .map((row) => {
        const fb = sanitizeFallbacks((row as SavedConnection).fallbackSignalingUrls);
        return fb ? { ...row, fallbackSignalingUrls: fb } : row;
      })
      .sort((a, b) => b.lastUsedAt - a.lastUsedAt);
  } catch {
    return [];
  }
}

function persist(list: SavedConnection[]): void {
  if (typeof localStorage === "undefined") return;
  localStorage.setItem(STORAGE_KEY, JSON.stringify(list.slice(0, MAX_CONNECTIONS)));
}

export function upsertConnection(entry: {
  roomId: string;
  signalingUrl: string;
  label?: string;
  fallbackSignalingUrls?: string[];
}): SavedConnection {
  const roomId = entry.roomId.trim();
  const signalingUrl = entry.signalingUrl.trim();
  const list = listConnections();
  const idx = list.findIndex((c) => c.roomId === roomId && c.signalingUrl === signalingUrl);
  const now = Date.now();
  const fallbacks = sanitizeFallbacks(entry.fallbackSignalingUrls);
  let row: SavedConnection;
  if (idx >= 0) {
    const prev = list[idx]!;
    row = {
      ...prev,
      lastUsedAt: now,
      label: entry.label ?? prev.label,
      fallbackSignalingUrls: fallbacks ?? prev.fallbackSignalingUrls,
    };
    list.splice(idx, 1);
  } else {
    row = {
      id: newId(),
      roomId,
      signalingUrl,
      lastUsedAt: now,
      label: entry.label,
      ...(fallbacks ? { fallbackSignalingUrls: fallbacks } : {}),
    };
  }
  persist([row, ...list]);
  return row;
}

export function setFallbackSignalingUrls(id: string, fallbacks: string[]): void {
  const list = listConnections();
  const idx = list.findIndex((c) => c.id === id);
  if (idx < 0) return;
  const prev = list[idx]!;
  const cleaned = sanitizeFallbacks(fallbacks);
  const updated: SavedConnection = cleaned
    ? { ...prev, fallbackSignalingUrls: cleaned }
    : (() => {
        const { fallbackSignalingUrls: _omit, ...rest } = prev;
        return rest;
      })();
  list[idx] = updated;
  persist(list);
}

export function touchConnection(id: string): void {
  const list = listConnections();
  const idx = list.findIndex((c) => c.id === id);
  if (idx < 0) return;
  const [row] = list.splice(idx, 1);
  if (!row) return;
  row.lastUsedAt = Date.now();
  persist([row, ...list]);
}

export function removeConnection(id: string): void {
  persist(listConnections().filter((c) => c.id !== id));
}
