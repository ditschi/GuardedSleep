import { ScanError, type CodeScanner } from "../contract";
import type { Timers } from "./clock";

/** The `BarcodeDetector` of Chromium-based browsers (Shape Detection API); absent elsewhere. */
export interface BarcodeDetectorLike {
  detect(image: CanvasImageSource): Promise<Array<{ rawValue: string }>>;
}
export type BarcodeDetectorCtor = new (options: { formats: string[] }) => BarcodeDetectorLike;

export interface ScannerEnv {
  /** Absent on a page that is not a secure context. */
  mediaDevices: Pick<MediaDevices, "getUserMedia"> | undefined;
  /** The native detector where the browser has one. */
  barcodeDetector: BarcodeDetectorCtor | undefined;
  /** Loads the JavaScript decoder used where the native one is missing (code-split on purpose). */
  loadFallback(): Promise<(canvas: HTMLCanvasElement) => string | null>;
  createCanvas(): HTMLCanvasElement;
  timers: Timers;
}

/** About five frames per second is plenty for a QR code and light on a phone's battery. */
export const SCAN_INTERVAL_MS = 200;
/** Frames are scaled down to this width before decoding. */
const FRAME_WIDTH = 640;

function failureOf(error: unknown): ScanError {
  if (error instanceof ScanError) return error;
  const name = error instanceof DOMException ? error.name : "";
  if (name === "NotAllowedError" || name === "SecurityError") return new ScanError("denied");
  return new ScanError("no_camera");
}

/**
 * Camera scanner: native `BarcodeDetector` where available, `@zxing/browser` otherwise. Each
 * frame is tried as it is and inverted, because the baby device shows its code light on dark.
 * The camera is opened only by `start`, which the screen calls after "Allow camera".
 */
export function createQrScanner(env: ScannerEnv): CodeScanner {
  let stream: MediaStream | null = null;
  let cancelLoop: (() => void) | null = null;
  let generation = 0;

  const stop = (): void => {
    generation++;
    cancelLoop?.();
    cancelLoop = null;
    for (const track of stream?.getTracks() ?? []) track.stop();
    stream = null;
  };

  return {
    async start(video, onCode) {
      stop();
      const mine = generation;
      if (!env.mediaDevices) throw new ScanError("unsupported");
      let media: MediaStream;
      try {
        media = await env.mediaDevices.getUserMedia({
          video: { facingMode: { ideal: "environment" } },
          audio: false,
        });
      } catch (error) {
        throw failureOf(error);
      }
      if (mine !== generation) {
        // Stopped while the browser asked for permission.
        for (const track of media.getTracks()) track.stop();
        return;
      }
      stream = media;
      video.srcObject = media;
      video.muted = true;
      await video.play().catch(() => undefined);

      const native = env.barcodeDetector
        ? new env.barcodeDetector({ formats: ["qr_code"] })
        : undefined;
      const decodeFallback = native ? undefined : await env.loadFallback();
      if (mine !== generation) return;

      const canvas = env.createCanvas();
      const ctx = canvas.getContext("2d", { willReadFrequently: true });
      let busy = false;
      const decode = async (): Promise<string | null> => {
        if (native) {
          const hit = (await native.detect(canvas))[0];
          return hit ? hit.rawValue : null;
        }
        return decodeFallback ? decodeFallback(canvas) : null;
      };
      const tick = async (): Promise<void> => {
        if (busy || mine !== generation || !ctx || video.videoWidth === 0) return;
        busy = true;
        try {
          canvas.width = Math.min(FRAME_WIDTH, video.videoWidth);
          canvas.height = Math.round((canvas.width * video.videoHeight) / video.videoWidth);
          ctx.globalCompositeOperation = "source-over";
          ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
          let text = await decode().catch(() => null);
          if (text === null) {
            // White minus the frame inverts it: light-on-dark codes become dark-on-light.
            ctx.globalCompositeOperation = "difference";
            ctx.fillStyle = "#fff";
            ctx.fillRect(0, 0, canvas.width, canvas.height);
            text = await decode().catch(() => null);
          }
          if (text !== null && mine === generation) {
            stop();
            onCode(text);
          }
        } finally {
          busy = false;
        }
      };
      cancelLoop = env.timers.every(() => void tick(), SCAN_INTERVAL_MS);
    },
    stop,
  };
}
