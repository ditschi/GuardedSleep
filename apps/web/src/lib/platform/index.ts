import { loadCore } from "@guardedsleep/core-wasm/web";
// Vite emits the wasm file and hands over its URL; the glue code would otherwise guess it.
import wasmUrl from "@guardedsleep/core-wasm/pkg/guardedsleep_wasm_bg.wasm?url";
import type { AppController } from "../contract";
import { AlertPlayer, createSoundGate } from "./audio";
import { createClock, createTimers } from "./clock";
import { defaultSignalingUrls } from "./relay-stun";
import { createWebSocketSignaling, type WebSocketCtor } from "./signaling";
import { createWebMediaEngine } from "./web-media";
import { WebController, type NetworkWatcher } from "./web-controller";

function browserNetwork(): NetworkWatcher {
  let handler: (() => void) | null = null;
  return {
    start(onAvailable) {
      this.stop();
      handler = onAvailable;
      window.addEventListener("online", handler);
    },
    stop() {
      if (handler) window.removeEventListener("online", handler);
      handler = null;
    },
  };
}

/** 53 random bits: the core seeds its jitter with them and needs no entropy source of its own. */
function entropy53(): number {
  const [hi = 0, lo = 0] = crypto.getRandomValues(new Uint32Array(2));
  return (hi & 0x1f_ffff) * 2 ** 32 + lo;
}

/**
 * Builds the real controller on the browser APIs: the WASM core, WebSocket, RTCPeerConnection and
 * Web Audio. The only place that wires the platform together; `main.ts` calls it and hands the
 * result to the UI.
 */
export async function createWebApp(): Promise<AppController> {
  const core = await loadCore(wasmUrl);
  const timers = createTimers();
  const gate = createSoundGate(() => new AudioContext({ latencyHint: "interactive" }));
  return new WebController({
    core,
    clock: createClock(),
    timers,
    signaling: createWebSocketSignaling(WebSocket as unknown as WebSocketCtor),
    createMedia: (role, iceServers) =>
      createWebMediaEngine(role, {
        PeerConnection: RTCPeerConnection,
        iceServers,
        timers,
        gate,
        getUserMedia: (constraints) => {
          // Absent on a page that is not a secure context (plain http on a LAN address).
          if (!navigator.mediaDevices) {
            return Promise.reject(new DOMException("no media devices", "NotSupportedError"));
          }
          return navigator.mediaDevices.getUserMedia(constraints);
        },
        createSink: () => new Audio(),
        createStream: (track) => new MediaStream([track]),
      }),
    gate,
    alerts: new AlertPlayer({ gate, timers }),
    network: browserNetwork(),
    randomBytes: (length) => crypto.getRandomValues(new Uint8Array(length)),
    entropy: entropy53,
    defaultSignalingUrls: () => defaultSignalingUrls(window.location, import.meta.env.DEV),
  });
}
