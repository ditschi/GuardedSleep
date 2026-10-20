export {};

declare global {
  interface Window {
    __guardedsleep_test?: {
      getSession?: () =>
        | {
            getE2ePeerConnection: () => RTCPeerConnection | null;
            getE2eDataChannel: () => RTCDataChannel | null;
            getE2ePairingVerified: () => boolean;
            getE2eBabyLocalAudioTrackEnabled: () => boolean | null;
            sendListen: (on: boolean) => void;
          }
        | undefined;
      getInviteJson?: () => string;
      injectAudioLevel?: (rms: number, durationMs: number) => void;
      waitNextNoiseAlert?: (timeoutMs?: number) => Promise<number>;
      getNoiseAlertCount?: () => number;
      resetNoiseAlertCount?: () => void;
    };
  }
}
