/**
 * Dev / E2E only: oscillator-backed MediaStream when emulator or CI cannot inject fake mic WAV.
 */
export async function createDevSyntheticMicStream(): Promise<MediaStream> {
  const AC =
    window.AudioContext ??
    (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
  const ctx = new AC();
  if (ctx.state === "suspended") {
    await ctx.resume().catch(() => {});
  }
  const dest = ctx.createMediaStreamDestination();
  const osc = ctx.createOscillator();
  osc.type = "sine";
  osc.frequency.value = 440;
  const gain = ctx.createGain();
  gain.gain.value = 0.35;
  osc.connect(gain);
  gain.connect(dest);
  osc.start();
  return dest.stream;
}
