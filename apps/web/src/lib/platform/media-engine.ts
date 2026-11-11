import type { IceCandidate, IceServer } from "@guardedsleep/core-wasm";

/**
 * The talk volume of the baby unit. The same three words as the core's `TalkLevel`; a variant
 * added there no longer type-checks where the driver passes the core's value on.
 */
export type TalkLevel = "low" | "medium" | "high";

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
  /** Parent: the baby's inbound audio counters; baby: the talk line's. Null when no receiver exists yet. */
  inboundAudioStats(): Promise<InboundAudioStats | null>;
  /**
   * Parent: put a track on the talk sender (`replaceTrack`), or take it off with null. Resolves
   * false when the browser refused or this attempt has no talk sender.
   */
  setTalkTrack(track: MediaStreamTrack | null): Promise<boolean>;
  close(): void;
}

/** What became of `setTalkMic`: the core hears `talk_mic_attached` or `talk_mic_failed` for the first two. */
export type TalkMicResult = "attached" | "failed" | "detached" | "stale";

/** Microphone/playout side of a link: one engine per monitoring run. */
export interface MediaEngine {
  /**
   * `iceServers` is the core's list for this attempt (`create_attempt.ice_servers`), passed on
   * unchanged. Credentials in it live only as long as the attempt: never logged, never stored.
   */
  createAttempt(offerer: boolean, listener: AttemptListener, iceServers: IceServer[]): AttemptPort;
  /** Baby: gate the outgoing track (the core opens it for noise, listen and probe). */
  setMicEnabled(enabled: boolean): void;
  /** Baby: mix a synthetic test tone into the outgoing stream; never played locally. */
  startProbeTone(durationMs: number): void;
  /** Parent: playout muted ("Stop sound") without stopping decode or the meter. */
  setPlaybackMuted(muted: boolean): void;
  /** Parent: playback gain in percent (25..150). */
  setPlaybackGainPct(pct: number): void;
  /** Parent: mute only the baby's audio while the talk button is held (alerts are not affected). */
  setPlaybackDucked(ducked: boolean): void;
  /**
   * Parent: open the microphone for a talk with voice processing on (warm start at the press). It
   * is not sent until `setTalkMic(port, true)`. Resolves false when it could not be opened.
   */
  warmTalkMic(): Promise<boolean>;
  /**
   * Parent: attach the warm capture to `port`'s talk sender (`enabled`), or detach it, stop its
   * track and close the capture. A call that a later call overtook resolves `stale`.
   */
  setTalkMic(port: AttemptPort, enabled: boolean): Promise<TalkMicResult>;
  /** Parent: stop a warm capture that was never attached (released before the grant). Idempotent. */
  discardTalkMic(): void;
  /**
   * Baby: open or close the talk playback (a lease). `remainingMs` is the time to the deadline on
   * the core's clock; the end is scheduled on the audio clock and by a platform timer.
   */
  setTalkPlayback(enabled: boolean, level: TalkLevel, fadeInMs: number, remainingMs: number): void;
  /** Baby: play the chime at `level` through the talk output; `done` runs when it ended. */
  playTalkChime(level: TalkLevel, done: () => void): void;
  /** Baby: whether a talk lease is open right now. */
  talkLeaseOpen(): boolean;
  /** Baby: RMS 0..1 of the incoming talk audio since the previous call; null outside a lease. */
  drainTalkLevel(): number | null;
  /** RMS 0..1 of the microphone since the previous call; null when none (parent, or no audio). */
  drainMicRms(): number | null;
  /** RMS 0..1 of the received baby audio since the previous call; null when none (baby). */
  drainRemoteRms(): number | null;
  release(): void;
}
