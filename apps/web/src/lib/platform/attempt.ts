import type { IceCandidate } from "@guardedsleep/core-wasm";
import type { Timers } from "./clock";
import type { AttemptListener, AttemptPort, InboundAudioStats } from "./media-engine";
import { FINGERPRINT_RETRY_MS, FINGERPRINT_TRIES } from "./policy";

/** The part of `RTCPeerConnection` an attempt uses; `RTCPeerConnection` itself satisfies it. */
export type PeerConnectionCtor = new (config: RTCConfiguration) => RTCPeerConnection;

export interface AttemptDeps {
  PeerConnection: PeerConnectionCtor;
  /** The core's ICE servers of this attempt plus the user-entered TURN, in that order. */
  iceServers: RTCIceServer[];
  timers: Timers;
  /** Baby: the outgoing microphone stream; the m-line is `sendonly` (the core refuses others). */
  send: { track: MediaStreamTrack; stream: MediaStream } | null;
  /** Parent: the received baby audio. */
  onRemoteAudio(track: MediaStreamTrack, stream: MediaStream | undefined): void;
  /**
   * Baby: the receiver of the talk line. It arrives disabled (its `enabled` is false before this
   * call); only the playback lease turns it on.
   */
  onTalkAudio?(track: MediaStreamTrack, stream: MediaStream | undefined): void;
  /**
   * Parent: whether this browser may answer the talk line `sendonly`. False (iOS Safari) leaves it
   * `recvonly`, which the baby's core reads as "unsupported".
   */
  talkSend?: boolean;
}

const CHANNEL_LABEL = "guardedsleep";

/**
 * One PeerConnection + DataChannel for one core attempt handle. It contains no decisions: the
 * core asks for an offer, an answer or a close, and hears back what the connection did.
 */
export function createRtcAttempt(
  deps: AttemptDeps,
  offerer: boolean,
  listener: AttemptListener,
): AttemptPort {
  const pc = new deps.PeerConnection({ iceServers: deps.iceServers });
  let closed = false;
  let remoteSet = false;
  let answerRequested = false;
  let dataOpenSent = false;
  let fingerprintSent = false;
  let hasAudioReceiver = false;
  let channel: RTCDataChannel | null = null;
  /** Parent: the second audio transceiver, the talk line. */
  let talkTransceiver: RTCRtpTransceiver | null = null;
  const pendingIce: RTCIceCandidateInit[] = [];
  const cancels = new Set<() => void>();

  const fail = (): void => {
    if (!closed) listener.onConnectionState("failed");
  };

  const signalOpen = (): void => {
    if (dataOpenSent || closed) return;
    dataOpenSent = true;
    listener.onDataOpen();
  };

  const attachChannel = (dc: RTCDataChannel): void => {
    channel = dc;
    dc.onopen = signalOpen;
    dc.onclose = () => {
      if (!closed) listener.onDataClosed();
    };
    dc.onmessage = (ev: MessageEvent) => {
      if (!closed && typeof ev.data === "string") listener.onDataMessage(ev.data);
    };
    if (dc.readyState === "open") signalOpen();
  };

  const reportFingerprint = (tries: number): void => {
    if (closed || fingerprintSent) return;
    pc.getStats().then(
      (report) => {
        if (closed || fingerprintSent) return;
        const fp = remoteFingerprint(report);
        if (fp !== null) {
          fingerprintSent = true;
          listener.onDtlsFingerprint(fp);
        } else if (tries > 1) {
          const cancel = deps.timers.after(() => {
            cancels.delete(cancel);
            reportFingerprint(tries - 1);
          }, FINGERPRINT_RETRY_MS);
          cancels.add(cancel);
        }
      },
      () => undefined,
    );
  };

  pc.onicecandidate = (ev) => {
    if (closed || !ev.candidate) return;
    const c = ev.candidate;
    listener.onLocalIce({
      candidate: c.candidate,
      sdp_mid: c.sdpMid,
      sdp_m_line_index: c.sdpMLineIndex,
    });
  };
  pc.ondatachannel = (ev) => attachChannel(ev.channel);
  pc.ontrack = (ev) => {
    if (ev.track.kind !== "audio") return;
    hasAudioReceiver = true;
    if (deps.send) {
      // The baby only receives on the talk line. Silent until a lease opens it (design section 3).
      ev.track.enabled = false;
      deps.onTalkAudio?.(ev.track, ev.streams[0]);
      return;
    }
    deps.onRemoteAudio(ev.track, ev.streams[0]);
  };
  pc.onconnectionstatechange = () => {
    if (closed) return;
    listener.onConnectionState(pc.connectionState);
    if (pc.connectionState === "connected") reportFingerprint(FINGERPRINT_TRIES);
  };

  if (deps.send) {
    pc.addTransceiver(deps.send.track, { direction: "sendonly", streams: [deps.send.stream] });
    // The talk line: the baby only receives on it, and nothing flows until a parent talks. It is
    // always the second audio line (the core requires exactly two, monitoring first).
    pc.addTransceiver("audio", { direction: "recvonly" });
  }
  if (offerer) attachChannel(pc.createDataChannel(CHANNEL_LABEL, { ordered: true }));

  const setLocal = async (description: RTCSessionDescriptionInit): Promise<void> => {
    await pc.setLocalDescription(description);
    if (closed) return;
    const local = pc.localDescription;
    if (local) listener.onLocalDescription(local.type, local.sdp);
  };

  /**
   * The parent answers the talk line `sendonly` with no track: nothing is sent until a talk
   * attaches the microphone. Must run after the remote offer is applied and before the answer.
   */
  const armTalkLine = (): void => {
    if (deps.send || deps.talkSend === false || talkTransceiver) return;
    const audio = pc.getTransceivers().filter((t) => t.receiver.track.kind === "audio");
    const line = audio[1];
    if (!line) return;
    line.direction = "sendonly";
    talkTransceiver = line;
  };

  const answerNow = (): void => {
    pc.createAnswer().then(setLocal).catch(fail);
  };

  return {
    createOffer() {
      pc.createOffer().then(setLocal).catch(fail);
    },
    setRemoteDescription(type, sdp) {
      pc.setRemoteDescription({ type: type as RTCSdpType, sdp }).then(async () => {
        if (closed) return;
        remoteSet = true;
        armTalkLine();
        for (const c of pendingIce.splice(0)) await pc.addIceCandidate(c).catch(() => undefined);
        if (answerRequested) {
          answerRequested = false;
          answerNow();
        }
      }, fail);
    },
    createAnswer() {
      if (!remoteSet) {
        answerRequested = true;
        return;
      }
      answerNow();
    },
    addRemoteIce(candidate: IceCandidate) {
      const init: RTCIceCandidateInit = {
        candidate: candidate.candidate,
        sdpMid: candidate.sdp_mid ?? "0",
        sdpMLineIndex: candidate.sdp_m_line_index ?? 0,
      };
      if (!remoteSet) {
        pendingIce.push(init);
        return;
      }
      if (!closed) pc.addIceCandidate(init).catch(() => undefined);
    },
    sendData(text) {
      if (closed || channel?.readyState !== "open") return false;
      channel.send(text);
      return true;
    },
    async setTalkTrack(track) {
      const sender = talkTransceiver?.sender;
      if (closed || !sender) return false;
      try {
        await sender.replaceTrack(track);
        return true;
      } catch {
        return false;
      }
    },
    async inboundAudioStats(): Promise<InboundAudioStats | null> {
      if (closed || !hasAudioReceiver) return null;
      const report = await pc.getStats();
      let stats: InboundAudioStats | null = null;
      report.forEach((s: Record<string, unknown>) => {
        if (stats === null && s.type === "inbound-rtp" && (s.kind ?? s.mediaType) === "audio") {
          stats = {
            packetsReceived: typeof s.packetsReceived === "number" ? s.packetsReceived : 0,
            packetsLost: typeof s.packetsLost === "number" ? s.packetsLost : 0,
          };
        }
      });
      return stats;
    },
    close() {
      if (closed) return;
      closed = true;
      for (const cancel of cancels) cancel();
      cancels.clear();
      pc.onicecandidate = pc.ondatachannel = pc.ontrack = pc.onconnectionstatechange = null;
      if (channel) {
        channel.onopen = channel.onclose = channel.onmessage = null;
        channel.close();
      }
      pc.close();
    },
  };
}

/** The remote certificate's SHA-256 fingerprint from `getStats()`; null until it is available. */
export function remoteFingerprint(report: RTCStatsReport): string | null {
  let certificateId: string | null = null;
  report.forEach((s: Record<string, unknown>) => {
    if (certificateId === null && s.type === "transport") {
      const id = s.remoteCertificateId;
      if (typeof id === "string") certificateId = id;
    }
  });
  if (certificateId === null) return null;
  const cert = report.get(certificateId) as Record<string, unknown> | undefined;
  const fingerprint = cert?.fingerprint;
  const algorithm = cert?.fingerprintAlgorithm;
  if (typeof fingerprint === "string" && typeof algorithm === "string") {
    return algorithm.toLowerCase() === "sha-256" ? fingerprint : null;
  }
  return null;
}
