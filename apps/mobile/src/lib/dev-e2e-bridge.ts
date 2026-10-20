import type { StreamRmsSample } from "$lib/audio/sample-stream-rms.js";
import type { BabyMonitorState, ParentMonitorState } from "$lib/link/ui-contract.js";

export type SampleRemoteAudioRmsOptions = {
  windowMs?: number;
  intervalMs?: number;
};

/** Facade with the names the Playwright helpers have always used (`utils/probes.ts`). */
export type E2eSessionFacade = {
  getE2ePeerConnection: () => RTCPeerConnection | null;
  getE2eDataChannel: () => RTCDataChannel | null;
  /** True while the core reports `connected` (the pairing proof has been verified). */
  getE2ePairingVerified: () => boolean;
  /** Baby: `enabled` flag of the outgoing track; null on the parent. */
  getE2eBabyLocalAudioTrackEnabled: () => boolean | null;
  sendListen: (on: boolean) => void;
};

export type GuardedsleepDevTestApi = {
  readonly role: "baby" | "parent";
  getSession: () => E2eSessionFacade | undefined;
  /** Baby: the pairing code payload currently shown. */
  getInviteJson?: () => string;
  /** Baby: report a synthetic microphone level for a while (deterministic noise alerts). */
  injectAudioLevel?: (rms: number, durationMs: number) => void;
  setKioskLocked?: (locked: boolean) => void;
  isKioskLocked?: () => boolean;
  /** Parent: resolves on the next `noise_alert` event. */
  waitNextNoiseAlert?: (timeoutMs?: number) => Promise<number>;
  getNoiseAlertCount?: () => number;
  resetNoiseAlertCount?: () => void;
  /** Parent: sample RMS on the inbound WebRTC audio stream. */
  sampleRemoteAudioRms?: (opts?: SampleRemoteAudioRmsOptions) => Promise<StreamRmsSample>;
  sendProbeTest?: () => void;
  getLinkSnapshot?: () => { state: ParentMonitorState | BabyMonitorState; nowElapsed: number };
  resetListenAudioForE2e?: () => Promise<void>;
};

declare global {
  interface Window {
    __guardedsleep_test?: GuardedsleepDevTestApi;
  }
}

export {};
