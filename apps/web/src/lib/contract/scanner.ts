/*
 * The QR scanner port of the pairing screen. The camera is I/O, so the platform layer implements
 * it (`platform/qr-scanner.ts`); the screen only sees this interface. It has no counterpart in
 * UiContract.kt (Android scans with its own camera component) and is not part of `AppController`.
 */

/** Why the camera could not start. */
export type ScanFailure = "denied" | "no_camera" | "unsupported";

export class ScanError extends Error {
  constructor(readonly kind: ScanFailure) {
    super(`scan failed: ${kind}`);
    this.name = "ScanError";
  }
}

export interface CodeScanner {
  /**
   * Starts the camera into `video` and reports the first QR code it reads (`onCode` runs at most
   * once per `start`). Rejects with `ScanError` when the camera cannot be used.
   */
  start(video: HTMLVideoElement, onCode: (text: string) => void): Promise<void>;
  /** Stops the camera; safe to call at any time. */
  stop(): void;
}
