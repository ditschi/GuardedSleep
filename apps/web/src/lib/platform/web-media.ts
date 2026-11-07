import type { IceServer } from "@guardedsleep/core-wasm";
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
  /** The user-entered TURN entry (with its credentials, memory only); appended to every attempt. */
  userIceServers: RTCIceServer[];
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
    createAttempt(offerer, listener, coreIceServers) {
      return createRtcAttempt(
        {
          PeerConnection: deps.PeerConnection,
          iceServers: [...coreIceServers.map(toRtcIceServer), ...deps.userIceServers],
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

/** The core's entry as the browser takes it: the three fields, nothing added or dropped. */
function toRtcIceServer(server: IceServer): RTCIceServer {
  const entry: RTCIceServer = { urls: [...server.urls] };
  if (server.username !== undefined) entry.username = server.username;
  if (server.credential !== undefined) entry.credential = server.credential;
  return entry;
}
