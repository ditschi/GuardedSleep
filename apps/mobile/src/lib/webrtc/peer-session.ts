import {
  encodeHeartbeat,
  encodeListen,
  encodeNoiseAlert,
  encodeOpenSettings,
  encodePairProof,
  type HeartbeatEncodeOpts,
  type HeartbeatPayload,
  parseHeartbeat,
  parseListenCommand,
  parseNoiseAlert,
  parseOpenSettings,
  parsePairProof,
  type UnitRole,
} from "@guardedsleep/protocol";
import { computePairProofMac } from "$lib/pairing/pair-mac.js";
import { buildPathCandidates, selectSignalingPath } from "../signaling/path-selector.js";
import { connectSignaling, type SignalingConnection } from "../signaling/ws-client.js";

const DEFAULT_ICE: RTCIceServer[] = [
  { urls: "stun:stun.sipgate.de:3478" },
  { urls: "stun:stun.linphone.org:3478" },
];

export type PeerSessionState = "idle" | "connecting" | "connected" | "failed";

export type ConnectionQuality = "good" | "medium" | "poor" | "unknown";

export type PeerSessionHandlers = {
  onState: (s: PeerSessionState) => void;
  onDataMessage?: (text: string) => void;
  /** Baby: user-visible explanation when microphone capture fails (permission, HTTPS, hardware). */
  onLocalMediaError?: (message: string) => void;
  onLocalCapture?: (stream: MediaStream) => void;
  onDcHeartbeat?: (payload: HeartbeatPayload) => void;
  onDcNoiseAlert?: (ev: { rms: number; t: number }) => void;
  /** Baby: parent requested opening /settings on this device. */
  onOpenSettingsRequest?: () => void;
  onRemoteStream?: (stream: MediaStream) => void;
  onConnectionQuality?: (q: ConnectionQuality) => void;
};

export type PeerSessionOptions = {
  getHeartbeatExtras?: () => HeartbeatEncodeOpts;
  /** If set, DataChannel exchanges `pair_proof` (PSK + DTLS fingerprints) before app traffic. */
  pairing?: { roomId: string; pskKey: Uint8Array };
  /**
   * Higher-priority signaling URLs (e.g. LAN `ws://` from NSD / embedded listener), tried
   * before `signalingUrl` and `fallbackSignalingUrls`. See ADR-012 path ranking.
   */
  prependSignalingUrls?: readonly string[];
  /**
   * Ordered list of fallback signaling URLs (tried in order if the primary URL is
   * unreachable). See [`docs/decisions/ADR-012`](../../../../../docs/decisions/ADR-012-connection-model-offline-lan-and-handoff.md)
   * §2 "Path ranking". Empty / undefined preserves legacy behavior (primary only).
   */
  fallbackSignalingUrls?: readonly string[];
  /** Per-URL probe timeout for the path-selector (default 4000 ms). */
  pathProbeTimeoutMs?: number;
  /** Notified when the orchestrator selected a non-primary path. UI-only signal. */
  onPathSelected?: (info: { url: string; isFallback: boolean; index: number }) => void;
};

type SignalPayload =
  | { type: "sdp"; sdp: RTCSessionDescriptionInit }
  | { type: "ice"; candidate: RTCIceCandidateInit };

function formatGetUserMediaError(err: unknown): string {
  if (err instanceof DOMException) {
    switch (err.name) {
      case "NotAllowedError":
        return "Microphone access was denied. Allow the microphone for this site in your browser settings, then try again.";
      case "NotFoundError":
        return "No microphone was found on this device.";
      case "NotReadableError":
      case "AbortError":
        return "The microphone is in use or could not be opened. Close other apps using the mic and try again.";
      case "SecurityError":
        return "Microphone is blocked on insecure (non-HTTPS) pages. Open the site over HTTPS or localhost.";
      case "OverconstrainedError":
        return "Microphone constraints could not be satisfied. Try another browser or device.";
      default:
        break;
    }
    if (err.message) return err.message;
  }
  if (err instanceof Error && err.message) return err.message;
  return "Could not access the microphone. Use HTTPS (or localhost), allow the permission, and try again.";
}

function parseSignalPayload(payload: unknown): SignalPayload | null {
  if (!payload || typeof payload !== "object") return null;
  const o = payload as Record<string, unknown>;
  if (o.type === "sdp" && o.sdp && typeof o.sdp === "object") {
    return { type: "sdp", sdp: o.sdp as RTCSessionDescriptionInit };
  }
  if (o.type === "ice" && o.candidate && typeof o.candidate === "object") {
    return { type: "ice", candidate: o.candidate as RTCIceCandidateInit };
  }
  return null;
}

/**
 * WebRTC: baby captures audio (muted until listen/alert), parent plays.
 * DataChannel: heartbeat, noise_alert, listen, optional pair_proof (QR PSK + DTLS binding).
 */
export class PeerSession {
  private pc: RTCPeerConnection | null = null;
  private chan: RTCDataChannel | null = null;
  private sig: SignalingConnection | null = null;
  private seq = 0;
  private localStream: MediaStream | null = null;
  private localAudioTrack: MediaStreamTrack | null = null;
  private babyRemotePeerId: string | null = null;
  private signalingTargetId: string | null = null;
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  private qualityTimer: ReturnType<typeof setInterval> | null = null;
  private parentListenDesired = false;
  private noiseStreaming = false;
  private cleanupDone = false;
  private pairingVerified = false;
  private expectedPairMac: string | null = null;
  private pairingTimeout: ReturnType<typeof setTimeout> | null = null;

  constructor(
    private readonly role: UnitRole,
    private readonly roomId: string,
    private readonly signalingUrl: string,
    private readonly handlers: PeerSessionHandlers,
    private readonly options?: PeerSessionOptions,
  ) {}

  start(): void {
    this.cleanupDone = false;
    void this.bootstrap();
  }

  private shutdown(final: "idle" | "failed"): void {
    if (this.cleanupDone) return;
    this.cleanupDone = true;
    if (this.pairingTimeout != null) {
      clearTimeout(this.pairingTimeout);
      this.pairingTimeout = null;
    }
    if (this.heartbeatTimer != null) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
    this.stopQualityPoll();
    this.localStream?.getTracks().forEach((t) => t.stop());
    this.localStream = null;
    this.localAudioTrack = null;
    this.sig?.close();
    this.chan?.close();
    this.pc?.close();
    this.sig = null;
    this.chan = null;
    this.pc = null;
    this.babyRemotePeerId = null;
    this.signalingTargetId = null;
    this.parentListenDesired = false;
    this.noiseStreaming = false;
    this.pairingVerified = false;
    this.expectedPairMac = null;
    this.handlers.onState(final);
  }

  private applyBabyAudioGate(): void {
    if (this.role !== "baby" || !this.localAudioTrack) return;
    this.localAudioTrack.enabled = this.parentListenDesired || this.noiseStreaming;
  }

  setParentListen(enabled: boolean): void {
    if (this.role !== "baby") return;
    this.parentListenDesired = enabled;
    this.applyBabyAudioGate();
  }

  setNoiseAlertStreaming(enabled: boolean): void {
    if (this.role !== "baby") return;
    this.noiseStreaming = enabled;
    this.applyBabyAudioGate();
  }

  sendListen(enabled: boolean): void {
    if (this.role !== "parent") return;
    if (this.chan?.readyState !== "open") return;
    if (this.options?.pairing && !this.pairingVerified) return;
    this.chan.send(encodeListen(enabled));
  }

  /** Ask the baby unit to open app settings (in-app navigation). */
  sendOpenBabySettings(): void {
    if (this.role !== "parent") return;
    if (this.chan?.readyState !== "open") return;
    if (this.options?.pairing && !this.pairingVerified) return;
    this.chan.send(encodeOpenSettings());
  }

  sendNoiseAlert(rms: number): void {
    if (this.role !== "baby") return;
    if (this.chan?.readyState !== "open") return;
    if (this.options?.pairing && !this.pairingVerified) return;
    this.chan.send(encodeNoiseAlert(rms));
  }

  sendHeartbeatNow(): void {
    if (this.role !== "baby") return;
    if (this.chan?.readyState !== "open") return;
    if (this.options?.pairing && !this.pairingVerified) return;
    this.seq += 1;
    const extras = this.options?.getHeartbeatExtras?.();
    this.chan.send(encodeHeartbeat(this.seq, extras));
  }

  private async bootstrap(): Promise<void> {
    this.handlers.onState("connecting");
    this.pairingVerified = !this.options?.pairing;
    this.expectedPairMac = null;

    const prepends = this.options?.prependSignalingUrls ?? [];
    const fallbacks = this.options?.fallbackSignalingUrls ?? [];
    const candidates = buildPathCandidates(prepends, this.signalingUrl, fallbacks);
    if (candidates.length === 0) {
      this.shutdown("failed");
      return;
    }
    let effectiveSignalingUrl = this.signalingUrl;
    if (candidates.length > 1) {
      const selected = await selectSignalingPath(candidates, {
        perUrlTimeoutMs: this.options?.pathProbeTimeoutMs ?? 4000,
      });
      if (this.cleanupDone) return;
      if (!selected) {
        this.shutdown("failed");
        return;
      }
      effectiveSignalingUrl = selected.url;
      this.options?.onPathSelected?.({
        url: selected.url,
        isFallback: selected.index > 0,
        index: selected.index,
      });
    } else {
      effectiveSignalingUrl = candidates[0]?.url ?? this.signalingUrl;
    }

    if (this.cleanupDone) return;

    this.pc = new RTCPeerConnection({ iceServers: DEFAULT_ICE });

    this.pc.onicecandidate = (ev) => {
      if (!ev.candidate) return;
      const to = this.signalTo();
      this.sig?.sendSignal(
        { type: "ice", candidate: ev.candidate.toJSON() } satisfies SignalPayload,
        to,
      );
    };

    this.pc.onconnectionstatechange = () => {
      const st = this.pc?.connectionState;
      if (st === "connected") {
        this.handlers.onState("connected");
        if (this.role === "parent") this.startQualityPoll();
      }
      if (st === "failed") {
        this.stopQualityPoll();
        this.shutdown("failed");
      }
    };

    if (this.role === "baby") {
      if (typeof navigator === "undefined" || !navigator.mediaDevices?.getUserMedia) {
        this.handlers.onLocalMediaError?.(
          "This browser does not expose a microphone API. Try a current Chrome/Firefox/Safari or the Android app.",
        );
        this.shutdown("failed");
        return;
      }
      try {
        this.localStream = await navigator.mediaDevices.getUserMedia({
          audio: {
            echoCancellation: true,
            noiseSuppression: true,
            autoGainControl: true,
          },
          video: false,
        });
      } catch (err) {
        this.handlers.onLocalMediaError?.(formatGetUserMediaError(err));
        this.shutdown("failed");
        return;
      }
      if (this.cleanupDone || this.pc == null) {
        this.localStream.getTracks().forEach((t) => t.stop());
        this.localStream = null;
        return;
      }
      this.localAudioTrack = this.localStream.getAudioTracks()[0] ?? null;
      if (this.localAudioTrack) {
        this.localAudioTrack.enabled = false;
        this.pc.addTrack(this.localAudioTrack, this.localStream);
      }
      this.handlers.onLocalCapture?.(this.localStream);
      const ch = this.pc.createDataChannel("guardedsleep", { ordered: true });
      this.chan = ch;
      this.wireChannel(ch);
    } else {
      this.pc.ontrack = (ev) => {
        const ms = ev.streams[0];
        if (ms) this.handlers.onRemoteStream?.(ms);
      };
      this.pc.ondatachannel = (ev) => {
        this.chan = ev.channel;
        this.wireChannel(this.chan);
      };
    }

    if (this.cleanupDone || this.pc == null) {
      this.localStream?.getTracks().forEach((t) => t.stop());
      this.localStream = null;
      this.localAudioTrack = null;
      return;
    }

    this.sig = connectSignaling(effectiveSignalingUrl, this.roomId, this.role, {
      onWelcome: () => {},
      onClose: () => {
        if (this.cleanupDone) return;
        this.stopQualityPoll();
        this.shutdown("failed");
      },
      onPeer: (peerId) => {
        if (this.role === "baby") {
          if (!peerId || !this.pc) return;
          if (this.babyRemotePeerId !== null && this.babyRemotePeerId !== peerId) {
            return;
          }
          if (this.babyRemotePeerId === null) {
            this.babyRemotePeerId = peerId;
            void this.sendOffer();
          }
          return;
        }
        if (peerId && this.signalingTargetId === null) {
          this.signalingTargetId = peerId;
        }
      },
      onSignal: (_from, payload) => {
        void this.onSignalPayload(payload);
      },
      onError: () => {
        this.stopQualityPoll();
        this.shutdown("failed");
      },
    });
  }

  private async waitForBothSdps(maxMs: number): Promise<boolean> {
    const pc = this.pc;
    if (!pc) return false;
    const deadline = Date.now() + maxMs;
    while (Date.now() < deadline) {
      if (pc.localDescription?.sdp && pc.remoteDescription?.sdp) return true;
      await new Promise((r) => setTimeout(r, 40));
    }
    return false;
  }

  private async afterDataChannelOpen(ch: RTCDataChannel): Promise<void> {
    const cfg = this.options?.pairing;
    if (!cfg) {
      this.pairingVerified = true;
      if (this.role === "baby") this.startBabyHeartbeats(ch);
      return;
    }
    const ok = await this.waitForBothSdps(12_000);
    if (!ok || !this.pc?.localDescription?.sdp || !this.pc.remoteDescription?.sdp) {
      this.shutdown("failed");
      return;
    }
    const mac = await computePairProofMac(
      cfg.pskKey,
      cfg.roomId,
      this.pc.localDescription.sdp,
      this.pc.remoteDescription.sdp,
    );
    if (!mac) {
      this.shutdown("failed");
      return;
    }
    this.expectedPairMac = mac;
    ch.send(encodePairProof(mac));
    this.pairingTimeout = setTimeout(() => {
      if (!this.pairingVerified) this.shutdown("failed");
    }, 15_000);
  }

  private onPairProofOk(ch: RTCDataChannel): void {
    if (this.pairingVerified) return;
    this.pairingVerified = true;
    if (this.pairingTimeout != null) {
      clearTimeout(this.pairingTimeout);
      this.pairingTimeout = null;
    }
    if (this.role === "baby") this.startBabyHeartbeats(ch);
  }

  private startBabyHeartbeats(ch: RTCDataChannel): void {
    if (this.role !== "baby" || this.heartbeatTimer != null) return;
    const sendHb = (): void => {
      this.seq += 1;
      const extras = this.options?.getHeartbeatExtras?.();
      if (ch.readyState === "open") ch.send(encodeHeartbeat(this.seq, extras));
    };
    sendHb();
    this.heartbeatTimer = setInterval(sendHb, 10_000);
  }

  private signalTo(): string | undefined {
    if (this.role === "baby") return this.babyRemotePeerId ?? undefined;
    return this.signalingTargetId ?? undefined;
  }

  private wireChannel(ch: RTCDataChannel): void {
    ch.onopen = () => {
      void this.afterDataChannelOpen(ch);
    };

    ch.onmessage = (ev) => {
      if (typeof ev.data !== "string") return;
      const raw = ev.data;

      const proof = parsePairProof(raw);
      if (this.options?.pairing) {
        if (proof != null) {
          if (!this.expectedPairMac || proof !== this.expectedPairMac) {
            this.shutdown("failed");
            return;
          }
          this.onPairProofOk(ch);
          return;
        }
        if (!this.pairingVerified) return;
      }

      if (this.role === "baby") {
        const listen = parseListenCommand(raw);
        if (listen !== undefined) {
          this.parentListenDesired = listen;
          this.applyBabyAudioGate();
          return;
        }
        if (parseOpenSettings(raw)) {
          this.handlers.onOpenSettingsRequest?.();
          return;
        }
      }

      const alert = parseNoiseAlert(raw);
      if (alert) {
        this.handlers.onDcNoiseAlert?.(alert);
        return;
      }

      const hb = parseHeartbeat(raw);
      if (hb) {
        this.handlers.onDcHeartbeat?.(hb);
        return;
      }

      this.handlers.onDataMessage?.(raw);
    };
  }

  private async sendOffer(): Promise<void> {
    if (!this.pc || this.role !== "baby") return;
    try {
      const offer = await this.pc.createOffer();
      await this.pc.setLocalDescription(offer);
      this.sig?.sendSignal(
        { type: "sdp", sdp: this.pc.localDescription ?? offer } satisfies SignalPayload,
        this.signalTo(),
      );
    } catch {
      this.shutdown("failed");
    }
  }

  private async onSignalPayload(payload: unknown): Promise<void> {
    if (!this.pc) return;
    const p = parseSignalPayload(payload);
    if (!p) return;
    if (p.type === "sdp" && p.sdp) {
      try {
        if (this.role === "parent") {
          await this.pc.setRemoteDescription(p.sdp);
          const answer = await this.pc.createAnswer();
          await this.pc.setLocalDescription(answer);
          this.sig?.sendSignal(
            { type: "sdp", sdp: this.pc.localDescription ?? answer } satisfies SignalPayload,
            this.signalTo(),
          );
        } else {
          await this.pc.setRemoteDescription(p.sdp);
        }
      } catch {
        this.shutdown("failed");
      }
    }
    if (p.type === "ice" && p.candidate) {
      try {
        await this.pc.addIceCandidate(p.candidate);
      } catch {
        /* ignore */
      }
    }
  }

  private startQualityPoll(): void {
    this.stopQualityPoll();
    this.qualityTimer = setInterval(() => void this.pollQuality(), 3000);
    void this.pollQuality();
  }

  private stopQualityPoll(): void {
    if (this.qualityTimer != null) {
      clearInterval(this.qualityTimer);
      this.qualityTimer = null;
    }
  }

  private async pollQuality(): Promise<void> {
    const pc = this.pc;
    if (!pc || this.role !== "parent") return;
    let q: ConnectionQuality = "good";
    try {
      const report = await pc.getStats();
      report.forEach((rep) => {
        if (rep.type !== "inbound-rtp") return;
        const r = rep as RTCInboundRtpStreamStats;
        if (r.kind !== "audio") return;
        const lost = r.packetsLost ?? 0;
        const recv = r.packetsReceived ?? 0;
        const tot = lost + recv;
        if (tot < 40) return;
        const ratio = lost / tot;
        if (ratio > 0.08) q = "poor";
        else if (ratio > 0.02 && q !== "poor") q = "medium";
      });
    } catch {
      q = "unknown";
    }
    this.handlers.onConnectionQuality?.(q);
  }

  /** Dev / browser E2E only — returns `null` in production builds. */
  getE2ePeerConnection(): RTCPeerConnection | null {
    return import.meta.env.DEV ? this.pc : null;
  }

  /** Dev / browser E2E only — returns `null` in production builds. */
  getE2eDataChannel(): RTCDataChannel | null {
    return import.meta.env.DEV ? this.chan : null;
  }

  /** Dev / browser E2E only — whether `pair_proof` (if configured) has succeeded. */
  getE2ePairingVerified(): boolean {
    return import.meta.env.DEV ? this.pairingVerified : false;
  }

  /** Dev / browser E2E only — baby outgoing mic track enabled (Listen / noise gate). */
  getE2eBabyLocalAudioTrackEnabled(): boolean | null {
    if (!import.meta.env.DEV || this.role !== "baby") return null;
    return this.localAudioTrack?.enabled ?? null;
  }

  close(): void {
    this.shutdown("idle");
  }
}
