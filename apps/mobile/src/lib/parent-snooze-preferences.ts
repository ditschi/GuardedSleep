const KEY = "guardedsleep:parent:snooze-last-minutes";

export function readLastSnoozeMinutes(): number {
  if (typeof localStorage === "undefined") return 10;
  const n = Number.parseInt(localStorage.getItem(KEY) ?? "", 10);
  if (Number.isNaN(n) || n < 1 || n > 60) return 10;
  return n;
}

export function persistLastSnoozeMinutes(minutes: number): void {
  if (typeof localStorage === "undefined") return;
  localStorage.setItem(KEY, String(Math.min(60, Math.max(1, Math.round(minutes)))));
}
