import { Capacitor } from "@capacitor/core";

import { isNativeAndroid } from "$lib/android/guarded-sleep-native.js";

/**
 * True when this document is a secure context (HTTPS, localhost, etc.).
 * On SSR where `isSecureContext` is absent, returns true to avoid a false alarm until the client runs.
 */
export function isRuntimeSecureContext(): boolean {
  if (typeof globalThis.isSecureContext !== "boolean") return true;
  return globalThis.isSecureContext;
}

/** Installed PWA / “Add to Home Screen” standalone display, or iOS Safari standalone. */
export function isStandalonePwaDisplay(): boolean {
  if (typeof window === "undefined") return false;
  if (window.matchMedia?.("(display-mode: standalone)").matches) return true;
  const nav = window.navigator as Navigator & { standalone?: boolean };
  return nav.standalone === true;
}

/** Capacitor “web” build (browser tab or PWA) — not the native Android/iOS shell. */
export function isWebPlatformNotNative(): boolean {
  return Capacitor.getPlatform() === "web";
}

export { isNativeAndroid };
