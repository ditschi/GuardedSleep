/** `{duration}` as the catalogue expects it: "45 s", "2 min", "1 h 5 min". */
export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1000));
  if (total < 60) return `${total} s`;
  const minutes = Math.floor(total / 60);
  if (minutes < 60) return `${minutes} min`;
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return m === 0 ? `${h} h` : `${h} h ${m} min`;
}

/** "not reachable for m:ss". */
export function formatClockDuration(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

export function formatClockTime(wallMs: number, locale?: string): string {
  return new Date(wallMs).toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" });
}
