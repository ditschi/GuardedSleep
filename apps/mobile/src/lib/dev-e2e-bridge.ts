import type { PeerSession } from "$lib/webrtc/peer-session.js";

export type GuardedsleepDevTestApi = {
  readonly role: "baby" | "parent";
  getSession: () => PeerSession | undefined;
  /** Baby: compact invite JSON for the current room when PSK is in session storage. */
  getInviteJson?: () => string;
  /** Baby: adjust noise gate without DOM interaction. */
  setNoiseGate?: (threshold: number, holdMs: number) => void;
  /** Parent: resolves on next `noise_alert` DataChannel message (dev builds only). */
  waitNextNoiseAlert?: (timeoutMs?: number) => Promise<number>;
  /** Parent: how many `noise_alert` handlers ran (dev / E2E only). */
  getNoiseAlertCount?: () => number;
  resetNoiseAlertCount?: () => void;
};

declare global {
  interface Window {
    __guardedsleep_test?: GuardedsleepDevTestApi;
  }
}

export {};
