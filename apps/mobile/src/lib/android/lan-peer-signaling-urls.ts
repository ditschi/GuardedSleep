export type LanPeerEntry = {
  readonly room: string;
  readonly host: string;
  readonly port: number;
  readonly token: string;
};

/**
 * Build `ws://host:port` prepends for path selection. Opaque `gst-*` services (token set) are
 * included; legacy `gssleep-<room>` entries only when `room` matches the active session room.
 */
export function lanPrependSignalingUrls(
  peers: Readonly<Record<string, LanPeerEntry>>,
  activeRoom: string,
): string[] {
  const out: string[] = [];
  const seen = new Set<string>();
  for (const p of Object.values(peers)) {
    if (!p.host || p.port <= 0) continue;
    const url = `ws://${p.host}:${p.port}`;
    const key = url.toLowerCase();
    if (seen.has(key)) continue;
    if (p.token.length > 0) {
      seen.add(key);
      out.push(url);
    } else if (activeRoom !== "" && p.room === activeRoom) {
      seen.add(key);
      out.push(url);
    }
  }
  return out;
}
