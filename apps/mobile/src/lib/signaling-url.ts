/** localStorage key for user override; build-time default remains import.meta.env.VITE_SIGNALING_URL */
export const SIGNALING_URL_STORAGE_KEY = "guardedsleep:signaling-url";

const DEFAULT_FALLBACK = "ws://127.0.0.1:8787";

function viteSignalingDefault(): string | undefined {
  const v = import.meta.env.VITE_SIGNALING_URL;
  if (typeof v !== "string") return undefined;
  const t = v.trim();
  return t === "" ? undefined : t;
}

export function getStoredSignalingUrl(): string | null {
  if (typeof localStorage === "undefined") return null;
  const raw = localStorage.getItem(SIGNALING_URL_STORAGE_KEY);
  if (raw == null || raw.trim() === "") return null;
  return raw.trim();
}

/** Stored URL wins, then Vite env at build time, then local dev default. */
export function getEffectiveSignalingUrl(): string {
  return getStoredSignalingUrl() ?? viteSignalingDefault() ?? DEFAULT_FALLBACK;
}

export function setStoredSignalingUrl(url: string): void {
  if (typeof localStorage === "undefined") return;
  localStorage.setItem(SIGNALING_URL_STORAGE_KEY, url.trim());
}

export function clearStoredSignalingUrl(): void {
  if (typeof localStorage === "undefined") return;
  localStorage.removeItem(SIGNALING_URL_STORAGE_KEY);
}

export function signalingUrlDraftInitial(): string {
  return getStoredSignalingUrl() ?? viteSignalingDefault() ?? DEFAULT_FALLBACK;
}

export function validateSignalingUrl(url: string): string | null {
  const t = url.trim();
  if (t === "") return "URL must not be empty";
  try {
    const u = new URL(t);
    if (u.protocol !== "ws:" && u.protocol !== "wss:") return "Only ws:// or wss:// is allowed";
    return null;
  } catch {
    return "Invalid URL";
  }
}

/** True when the WebSocket host is loopback — fine for single-machine dev, unreachable from another phone on LAN. */
export function isLoopbackSignalingHost(url: string): boolean {
  const err = validateSignalingUrl(url);
  if (err !== null) return false;
  try {
    const host = new URL(url.trim()).hostname.toLowerCase();
    return host === "localhost" || host === "127.0.0.1" || host === "::1" || host === "[::1]";
  } catch {
    return false;
  }
}

/** Per-session URL from invite (`?signal=`), without changing global default. */
export function resolveSignalingUrlForSession(signalFromInvite: string | null | undefined): string {
  if (signalFromInvite == null) return getEffectiveSignalingUrl();
  const t = signalFromInvite.trim();
  if (t === "") return getEffectiveSignalingUrl();
  return validateSignalingUrl(t) === null ? t : getEffectiveSignalingUrl();
}
