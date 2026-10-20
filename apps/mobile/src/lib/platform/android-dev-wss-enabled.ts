import { Capacitor } from "@capacitor/core";
import { devConnectHintsEnabled } from "$lib/signaling/dev-connect-hints-enabled.js";

/** Route wss:// signaling through the Android OkHttp bridge (debug self-signed LAN). */
export function androidDevWssSignalingEnabled(): boolean {
  if (Capacitor.getPlatform() !== "android") return false;
  return devConnectHintsEnabled() || import.meta.env.VITE_ANDROID_DEV_WSS === "true";
}

export function shouldUseAndroidNativeWss(url: string): boolean {
  if (!androidDevWssSignalingEnabled()) return false;
  try {
    return new URL(url.trim()).protocol === "wss:";
  } catch {
    return false;
  }
}
