import type { IceCandidate } from "@guardedsleep/core-wasm";

/**
 * Callbacks of one peer connection (one "attempt" of the core). They may fire at any time after
 * `close()` was requested elsewhere; the driver drops events of attempts it no longer knows.
 */
export interface AttemptListener {
  onLocalDescription(type: string, sdp: string): void;
  onLocalIce(candidate: IceCandidate): void;
  onDataOpen(): void;
  onDataMessage(text: string): void;
  onDataClosed(): void;
  /** Lowercase `RTCPeerConnectionState`: new, connecting, connected, disconnected, failed, closed. */
  onConnectionState(state: string): void;
  /** SHA-256 fingerprint of the remote DTLS certificate (colon-separated hex), once connected. */
  onDtlsFingerprint(fingerprint: string): void;
}

export interface InboundAudioStats {
  packetsReceived: number;
  packetsLost: number;
}

/** One RTCPeerConnection + DataChannel lifetime. */
export interface AttemptPort {
  createOffer(): void;
  setRemoteDescription(type: string, sdp: string): void;
  /** Runs after the remote description is applied, even if requested earlier. */
  createAnswer(): void;
  addRemoteIce(candidate: IceCandidate): void;
  sendData(text: string): boolean;
  /** Parent: inbound audio counters; null when no audio receiver exists yet. */
  inboundAudioStats(): Promise<InboundAudioStats | null>;
  close(): void;
}

/** Microphone/playout side of a link: one engine per monitoring run. */
export interface MediaEngine {
  createAttempt(offerer: boolean, listener: AttemptListener): AttemptPort;
  /** Baby: gate the outgoing track (the core opens it for noise, listen and probe). */
  setMicEnabled(enabled: boolean): void;
  /** Baby: mix a synthetic test tone into the outgoing stream; never played locally. */
  startProbeTone(durationMs: number): void;
  /** Parent: playout muted ("Stop sound") without stopping decode or the meter. */
  setPlaybackMuted(muted: boolean): void;
  /** Parent: playback gain in percent (25..150). */
  setPlaybackGainPct(pct: number): void;
  /** RMS 0..1 of the microphone since the previous call; null when none (parent, or no audio). */
  drainMicRms(): number | null;
  /** RMS 0..1 of the received baby audio since the previous call; null when none (baby). */
  drainRemoteRms(): number | null;
  release(): void;
}
