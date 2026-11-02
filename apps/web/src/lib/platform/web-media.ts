import type { Role } from "../contract";
import { createRtcAttempt, type PeerConnectionCtor } from "./attempt";
import {
  createRemotePlayback,
  openMicCapture,
  type MicCapture,
  type PlaybackDeps,
  type RemotePlayback,
  type SoundGate,
} from "./audio";
import type { Timers } from "./clock";
import type { MediaEngine } from "./media-engine";

export interface WebMediaDeps {
  PeerConnection: PeerConnectionCtor;
  /** The relay's built-in STUN only. */
  iceServers: RTCIceServer[];
  timers: Timers;
  gate: SoundGate;
  getUserMedia(constraints: MediaStreamConstraints): Promise<MediaStream>;
  createSink: PlaybackDeps["createSink"];
  createStream(track: MediaStreamTrack): MediaStream;
}

/**
 * The browser media engine of one monitoring run. The baby opens the microphone (this rejects
 * when the permission is refused), the parent builds the playout chain; both create their peer
 * connections through `createRtcAttempt`.
 */
export async function createWebMediaEngine(role: Role, deps: WebMediaDeps): Promise<MediaEngine> {
  let mic: MicCapture | null = null;
  let playback: RemotePlayback | null = null;
  if (role === "baby") {
    mic = await openMicCapture({
      getUserMedia: deps.getUserMedia,
      gate: deps.gate,
      timers: deps.timers,
    });
  } else {
    playback = createRemotePlayback({
      gate: deps.gate,
      timers: deps.timers,
      createSink: deps.createSink,
    });
  }

  return {
    createAttempt(offerer, listener) {
      return createRtcAttempt(
        {
          PeerConnection: deps.PeerConnection,
          iceServers: deps.iceServers,
          timers: deps.timers,
          send: mic ? { track: mic.track, stream: mic.stream } : null,
          onRemoteAudio: (track, stream) => playback?.play(stream ?? deps.createStream(track)),
        },
        offerer,
        listener,
      );
    },
    setMicEnabled: (enabled) => mic?.setEnabled(enabled),
    startProbeTone: (durationMs) => mic?.playProbeTone(durationMs),
    setPlaybackMuted: (muted) => playback?.setMuted(muted),
    setPlaybackGainPct: (pct) => playback?.setGainPct(pct),
    drainMicRms: () => mic?.drainRms() ?? null,
    drainRemoteRms: () => playback?.drainRms() ?? null,
    release() {
      mic?.release();
      playback?.release();
    },
  };
}
