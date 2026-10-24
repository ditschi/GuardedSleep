import { loadCore } from "@guardedsleep/core-wasm/web";
// Vite emits the wasm file and hands over its URL; the glue code would otherwise guess it.
import wasmUrl from "@guardedsleep/core-wasm/pkg/guardedsleep_wasm_bg.wasm?url";
import type { AppController, CodeScanner } from "../contract";
import { AlertPlayer, createSoundGate } from "./audio";
import { watchBattery, type BatteryManagerLike } from "./battery";
import { createClock, createTimers } from "./clock";
import { createPageWatch, type WakeLockApi } from "./page-watch";
import { createQrScanner, type BarcodeDetectorCtor } from "./qr-scanner";
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

// One clock for the controller and for the screens' countdowns, so both agree.
const clock = createClock();

/** Monotonic milliseconds as the controller counts them (the screens' ticker reads this). */
export function elapsedNow(): number {
  return clock.elapsedMs();
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
  // Dev builds (`just dev`, the browser E2E) run the signaling server on loopback, which the invite
  // codec refuses in a release build.
  const core = await loadCore(wasmUrl, { allowDevLoopback: import.meta.env.DEV });
  const timers = createTimers();
  const gate = createSoundGate(() => new AudioContext({ latencyHint: "interactive" }));
  const battery = watchBattery(
    navigator as unknown as { getBattery?: () => Promise<BatteryManagerLike> },
  );
  const page = createPageWatch({
    doc: document,
    win: window,
    wakeLock: (navigator as unknown as { wakeLock?: WakeLockApi }).wakeLock,
    clock,
    timers,
  });
  return new WebController({
    core,
    clock,
    page,
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
    battery: battery.state,
    deviceStatus: () => {
      const unit_warnings = page.unitWarnings();
      return { ...battery.deviceStatus(), ...(unit_warnings.length > 0 ? { unit_warnings } : {}) };
    },
    defaultSignalingUrls: () => defaultSignalingUrls(window.location, import.meta.env.DEV),
  });
}

/** The camera scanner of the parent pairing screen (native `BarcodeDetector`, zxing otherwise). */
export function createWebScanner(): CodeScanner {
  return createQrScanner({
    // Absent on a page that is not a secure context (plain http on a LAN address).
    mediaDevices: navigator.mediaDevices,
    barcodeDetector: (window as unknown as { BarcodeDetector?: BarcodeDetectorCtor })
      .BarcodeDetector,
    // Loaded only when the browser has no native detector, so most visitors never fetch it.
    loadFallback: async () => {
      const { BrowserQRCodeReader } = await import("@zxing/browser");
      const reader = new BrowserQRCodeReader();
      return (canvas) => {
        try {
          return reader.decodeFromCanvas(canvas).getText();
        } catch {
          // "No QR code in this frame" is an exception in zxing; the next frame tries again.
          return null;
        }
      };
    },
    createCanvas: () => document.createElement("canvas"),
    timers: createTimers(),
  });
}
