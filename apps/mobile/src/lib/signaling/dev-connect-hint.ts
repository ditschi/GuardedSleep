import { isLoopbackSignalingHost } from "$lib/signaling-url.js";

export type DevSignalingConnectHint =
  | { kind: "loopback" }
  | { kind: "wss_lan"; suggestedPlainWs: string }
  | { kind: "none" };

/** Dev-only troubleshooting: common LAN / native-app signaling misconfigurations. */
export function devSignalingConnectHint(signalingUrl: string): DevSignalingConnectHint {
  const t = signalingUrl.trim();
  if (isLoopbackSignalingHost(t)) return { kind: "loopback" };
  try {
    const u = new URL(t);
    if (u.protocol === "wss:" && !isLoopbackSignalingHost(t)) {
      return { kind: "wss_lan", suggestedPlainWs: `ws://${u.hostname}:8787` };
    }
  } catch {
    // ignore invalid URL
  }
  return { kind: "none" };
}
