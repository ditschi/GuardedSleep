import type { IceCandidate, Role } from "@guardedsleep/core-wasm";
import { readRemoteDtlsFingerprint } from "./dtls-stats.js";

export const DATA_CHANNEL_LABEL = "guardedsleep";

/** Public STUN servers see only connection metadata (AGENTS.md §7.1); media never touches them. */
export const DEFAULT_ICE_SERVERS: RTCIceServer[] = [
  { urls: "stun:stun.sipgate.de:3478" },
  { urls: "stun:stun.linphone.org:3478" },
];

/** Delay before the peer connection is really closed, so a final `stop_notice` can flush. */
export const CLOSE_FLUSH_MS = 250;

export type AttemptCallbacks = {
  onLocalIce: (candidate: IceCandidate) => void;
  onDataOpen: () => void;
  onDataMessage: (text: string) => void;
  onDataClosed: () => void;
  onPcState: (state: string) => void;
  onRemoteStream: (stream: MediaStream) => void;
};

export type AttemptInit = {
  handle: number;
  offerer: boolean;
  role: Role;
  /** Baby: the outgoing audio track (a clone of the microphone track). */
  outgoingTrack: MediaStreamTrack | null;
  callbacks: AttemptCallbacks;
};

/** One RTCPeerConnection + DataChannel for one attempt handle of the core's `Link`. */
export interface AttemptPeer {
  /** Resolves once the local description is set; returns it for the core. */
  createOffer(): Promise<{ type: string; sdp: string }>;
  createAnswer(): Promise<{ type: string; sdp: string }>;
  setRemoteDescription(type: string, sdp: string): Promise<void>;
  /** Buffered until the remote description is set. */
  addIceCandidate(candidate: IceCandidate): void;
  send(text: string): boolean;
  audioSender(): RTCRtpSender | null;
  /** SHA-256 fingerprint of the remote DTLS certificate (WebRTC stats); null if not listed yet. */
  remoteDtlsFingerprint(): Promise<string | null>;
  close(): void;
  /** Dev / E2E only. */
  readonly pc: RTCPeerConnection;
  dataChannel(): RTCDataChannel | null;
}

export type AttemptFactory = (init: AttemptInit) => AttemptPeer;

export function createAttemptPeer(
  init: AttemptInit,
  iceServers: RTCIceServer[] = DEFAULT_ICE_SERVERS,
): AttemptPeer {
  const { callbacks: cb } = init;
  const pc = new RTCPeerConnection({ iceServers });
  let dc: RTCDataChannel | null = null;
  let sender: RTCRtpSender | null = null;
  let closed = false;
  let remoteSet = false;
  let pendingIce: IceCandidate[] = [];

  const wireChannel = (channel: RTCDataChannel): void => {
    dc = channel;
    channel.onopen = () => {
      if (!closed) cb.onDataOpen();
    };
    channel.onmessage = (ev) => {
      if (!closed && typeof ev.data === "string") cb.onDataMessage(ev.data);
    };
    channel.onclose = () => {
      if (!closed) cb.onDataClosed();
    };
  };

  pc.onicecandidate = (ev) => {
    if (closed || !ev.candidate) return;
    cb.onLocalIce({
      candidate: ev.candidate.candidate,
      sdp_mid: ev.candidate.sdpMid ?? null,
      sdp_m_line_index: ev.candidate.sdpMLineIndex ?? null,
    });
  };
  pc.onconnectionstatechange = () => {
    if (!closed) cb.onPcState(pc.connectionState);
  };
  pc.ontrack = (ev) => {
    if (closed) return;
    cb.onRemoteStream(ev.streams[0] ?? new MediaStream([ev.track]));
  };

  if (init.offerer) {
    // The baby only ever sends: a `sendonly` transceiver means no remote audio can be played.
    const tx = init.outgoingTrack
      ? pc.addTransceiver(init.outgoingTrack, { direction: "sendonly" })
      : pc.addTransceiver("audio", { direction: "sendonly" });
    sender = tx.sender;
    wireChannel(pc.createDataChannel(DATA_CHANNEL_LABEL, { ordered: true }));
  } else {
    pc.ondatachannel = (ev) => {
      if (!closed && ev.channel.label === DATA_CHANNEL_LABEL) wireChannel(ev.channel);
    };
  }

  const flushIce = (): void => {
    const list = pendingIce;
    pendingIce = [];
    for (const c of list) void applyIce(c);
  };
  const applyIce = async (c: IceCandidate): Promise<void> => {
    try {
      await pc.addIceCandidate({
        candidate: c.candidate,
        sdpMid: c.sdp_mid,
        sdpMLineIndex: c.sdp_m_line_index,
      });
    } catch {
      /* a stale or malformed candidate must not break the attempt */
    }
  };
  const localDescription = (): { type: string; sdp: string } => {
    const d = pc.localDescription;
    if (!d) throw new Error("no local description");
    return { type: d.type, sdp: d.sdp };
  };

  return {
    pc,
    dataChannel: () => dc,
    async createOffer() {
      await pc.setLocalDescription(await pc.createOffer());
      return localDescription();
    },
    async createAnswer() {
      await pc.setLocalDescription(await pc.createAnswer());
      return localDescription();
    },
    async setRemoteDescription(type, sdp) {
      await pc.setRemoteDescription({ type: type as RTCSdpType, sdp });
      remoteSet = true;
      flushIce();
    },
    addIceCandidate(candidate) {
      if (closed) return;
      if (remoteSet) void applyIce(candidate);
      else pendingIce.push(candidate);
    },
    send(text) {
      if (!dc || dc.readyState !== "open") return false;
      try {
        dc.send(text);
        return true;
      } catch {
        return false;
      }
    },
    audioSender: () => sender,
    remoteDtlsFingerprint: () => readRemoteDtlsFingerprint(pc),
    close() {
      if (closed) return;
      closed = true;
      pendingIce = [];
      setTimeout(() => {
        try {
          dc?.close();
          pc.close();
        } catch {
          /* already closed */
        }
      }, CLOSE_FLUSH_MS);
    },
  };
}
