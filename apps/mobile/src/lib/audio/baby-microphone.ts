import { isRuntimeSecureContext } from "$lib/runtime/capabilities.js";

const BABY_AUDIO_CONSTRAINTS: MediaStreamConstraints = {
  audio: {
    echoCancellation: true,
    noiseSuppression: true,
    autoGainControl: true,
  },
  video: false,
};

export type BabyMicrophoneBlockReason = "insecure" | "no-api";

export function babyMicrophoneBlockReason(): BabyMicrophoneBlockReason | null {
  if (typeof navigator === "undefined") return "no-api";
  if (!isRuntimeSecureContext()) return "insecure";
  if (!navigator.mediaDevices?.getUserMedia) return "no-api";
  return null;
}

export function formatBabyMicrophoneError(err: unknown): string {
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
  }
  return "Could not access the microphone. Use HTTPS (or localhost), allow the permission, and try again.";
}

/** Opens the baby-unit microphone. Call from a user gesture when the first attempt was blocked. */
export async function requestBabyMicrophone(): Promise<MediaStream> {
  const blocked = babyMicrophoneBlockReason();
  if (blocked === "insecure") {
    throw new DOMException("Insecure context", "SecurityError");
  }
  if (blocked === "no-api") {
    throw new DOMException("getUserMedia unavailable", "NotSupportedError");
  }
  return navigator.mediaDevices.getUserMedia(BABY_AUDIO_CONSTRAINTS);
}

export function stopBabyMicrophoneStream(stream: MediaStream | null | undefined): void {
  stream?.getTracks().forEach((t) => t.stop());
}

let parkedForSettings: MediaStream | null = null;
let preserveMicOnTeardown = false;

/** Keep the mic open while the baby unit visits /settings and returns. */
export function parkBabyMicrophoneForSettings(stream: MediaStream | null | undefined): void {
  parkedForSettings = stream ?? null;
  preserveMicOnTeardown = stream != null;
}

export function takeParkedBabyMicrophone(): MediaStream | null {
  const stream = parkedForSettings;
  parkedForSettings = null;
  preserveMicOnTeardown = false;
  return stream;
}

export function shouldPreserveBabyMicrophoneOnTeardown(): boolean {
  return preserveMicOnTeardown;
}

export function clearBabyMicrophonePreservation(): void {
  preserveMicOnTeardown = false;
  parkedForSettings = null;
}

let prewarmedBabyMic: MediaStream | null = null;

/** Call from a click handler when the user picks the baby/child role — keeps the user-gesture chain. */
export async function prewarmBabyMicrophoneFromUserGesture(): Promise<MediaStream | null> {
  const blocked = babyMicrophoneBlockReason();
  if (blocked) return null;
  if (prewarmedBabyMic?.getAudioTracks().some((t) => t.readyState === "live")) {
    return prewarmedBabyMic;
  }
  stopBabyMicrophoneStream(prewarmedBabyMic);
  prewarmedBabyMic = null;
  try {
    prewarmedBabyMic = await requestBabyMicrophone();
    return prewarmedBabyMic;
  } catch {
    prewarmedBabyMic = null;
    return null;
  }
}

export function takePrewarmedBabyMicrophone(): MediaStream | null {
  const stream = prewarmedBabyMic;
  prewarmedBabyMic = null;
  return stream;
}

export function clearPrewarmedBabyMicrophone(): void {
  stopBabyMicrophoneStream(prewarmedBabyMic);
  prewarmedBabyMic = null;
}
