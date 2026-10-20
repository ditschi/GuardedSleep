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

/**
 * Routes alert playback through Web Audio gain.
 * Listen-mode continuous audio should use the raw stream without this pipeline.
 */
export async function attachAlertAudioPipeline(
  stream: MediaStream,
  audioEl: HTMLAudioElement,
): Promise<AlertAudioPipeline> {
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
    },
  };
}
