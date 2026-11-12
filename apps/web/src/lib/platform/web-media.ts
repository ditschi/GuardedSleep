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
import { toRtcIceServer } from "./ice-servers";
import type { AttemptPort, MediaEngine, TalkMicResult } from "./media-engine";
import {
  createTalkPlayback,
  openTalkCapture,
  type TalkCapture,
  type TalkPlayback,
} from "./talk-audio";

export interface WebMediaDeps {
  PeerConnection: PeerConnectionCtor;
  /** The user-entered TURN entry (with its credentials, memory only); appended to every attempt. */
  userIceServers: RTCIceServer[];
  timers: Timers;
  gate: SoundGate;
  getUserMedia(constraints: MediaStreamConstraints): Promise<MediaStream>;
  createSink: PlaybackDeps["createSink"];
  createStream(track: MediaStreamTrack): MediaStream;
  /** Parent: false where this browser must not answer the talk line `sendonly` (iOS Safari). */
  talkSend?: boolean;
}

/**
 * The browser media engine of one monitoring run. The baby opens the microphone (this rejects
 * when the permission is refused), the parent builds the playout chain; both create their peer
 * connections through `createRtcAttempt`.
 */
export async function createWebMediaEngine(role: Role, deps: WebMediaDeps): Promise<MediaEngine> {
  let mic: MicCapture | null = null;
  let playback: RemotePlayback | null = null;
  let talkOut: TalkPlayback | null = null;
  // Parent: the capture of the talk in progress, and a counter so a late result is recognised.
  let capture: TalkCapture | null = null;
  let micCalls = 0;
  if (role === "baby") {
    mic = await openMicCapture({
      getUserMedia: deps.getUserMedia,
      gate: deps.gate,
      timers: deps.timers,
    });
    talkOut = createTalkPlayback({
      gate: deps.gate,
      timers: deps.timers,
      createSink: deps.createSink,
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
          onTalkAudio: (track, stream) =>
            talkOut?.attach(track, stream ?? deps.createStream(track)),
          talkSend: deps.talkSend ?? true,
        },
        offerer,
        listener,
      );
    },
    setMicEnabled: (enabled) => mic?.setEnabled(enabled),
    startProbeTone: (durationMs) => mic?.playProbeTone(durationMs),
    setPlaybackMuted: (muted) => playback?.setMuted(muted),
    setPlaybackDucked: (ducked) => playback?.setDucked(ducked),
    warmTalkMic() {
      capture ??= openTalkCapture(deps.getUserMedia);
      return capture.ready.then((track) => track !== null);
    },
    async setTalkMic(port: AttemptPort, enabled: boolean): Promise<TalkMicResult> {
      const call = ++micCalls;
      if (!enabled) {
        // The track goes off the sender first, then the microphone closes, in the same turn.
        const detached = port.setTalkTrack(null);
        capture?.stop();
        capture = null;
        await detached;
        return "detached";
      }
      capture ??= openTalkCapture(deps.getUserMedia);
      const opened = capture;
      const track = await opened.ready;
      if (call !== micCalls) return "stale";
      if (track === null) {
        capture = null;
        return "failed";
      }
      const attached = await port.setTalkTrack(track);
      if (call !== micCalls) return "stale";
      if (!attached) {
        opened.stop();
        capture = null;
        return "failed";
      }
      return "attached";
    },
    discardTalkMic() {
      micCalls++;
      capture?.stop();
      capture = null;
    },
    setTalkPlayback(enabled, level, fadeInMs, remainingMs) {
      if (enabled) talkOut?.open(level, fadeInMs, remainingMs);
      else talkOut?.close();
    },
    playTalkChime: (level, done) => talkOut?.playChime(level, done),
    talkLeaseOpen: () => talkOut?.isOpen ?? false,
    drainTalkLevel: () => talkOut?.drainRms() ?? null,
    setPlaybackGainPct: (pct) => playback?.setGainPct(pct),
    drainMicRms: () => mic?.drainRms() ?? null,
    drainRemoteRms: () => playback?.drainRms() ?? null,
    release() {
      mic?.release();
      playback?.release();
      talkOut?.release();
      capture?.stop();
      capture = null;
    },
  };
}
