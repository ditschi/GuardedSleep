import { GuardedSleepNative, isNativeAndroid } from "$lib/android/guarded-sleep-native.js";
import { readAppPreferences } from "$lib/app-preferences.js";

export type AlertAudioPipeline = {
  disconnect: () => void;
};

const MIN_GAIN_FLOOR = 0.8;

export function resolveAlertPlaybackGain(isAlertActive: boolean): number {
  const prefs = readAppPreferences();
  let gain = prefs.alertPlaybackGain;
  if (isAlertActive && prefs.alertMinGainDuringAlert && gain < MIN_GAIN_FLOOR) {
    gain = MIN_GAIN_FLOOR;
  }
  return Math.min(2, Math.max(0.5, gain));
}

export async function applyNativeAlertAudioRouting(): Promise<void> {
  if (!isNativeAndroid()) return;
  const prefs = readAppPreferences();
  await GuardedSleepNative.applyAlertAudioRouting({
    mode: prefs.alertAudioOutput,
    gain: resolveAlertPlaybackGain(true),
  });
}

export async function releaseNativeAlertAudioRouting(): Promise<void> {
  if (!isNativeAndroid()) return;
  await GuardedSleepNative.releaseAlertAudioRouting();
}

/**
 * Routes alert playback through Web Audio gain + optional native speaker routing (ADR-011).
 * Listen-mode continuous audio should use the raw stream without this pipeline.
 */
export async function attachAlertAudioPipeline(
  stream: MediaStream,
  audioEl: HTMLAudioElement,
): Promise<AlertAudioPipeline> {
  await applyNativeAlertAudioRouting();

  const gain = resolveAlertPlaybackGain(true);
  const AC =
    window.AudioContext ??
    (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
  const ctx = new AC();
  if (ctx.state === "suspended") {
    await ctx.resume().catch(() => {});
  }
  const source = ctx.createMediaStreamSource(stream);
  const gainNode = ctx.createGain();
  gainNode.gain.value = gain;
  const dest = ctx.createMediaStreamDestination();
  source.connect(gainNode);
  gainNode.connect(dest);

  audioEl.srcObject = dest.stream;
  audioEl.muted = false;

  return {
    disconnect: () => {
      source.disconnect();
      gainNode.disconnect();
      audioEl.srcObject = stream;
      void ctx.close().catch(() => {});
      void releaseNativeAlertAudioRouting();
    },
  };
}
