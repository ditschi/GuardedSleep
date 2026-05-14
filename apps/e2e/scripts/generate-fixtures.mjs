import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const fixturesDir = join(__dirname, "../tests/fixtures");

function writePcmWav(path, samples, sampleRate, bitsPerSample = 16) {
  const numChannels = 1;
  const blockAlign = (numChannels * bitsPerSample) / 8;
  const byteRate = sampleRate * blockAlign;
  const dataSize = samples.length * 2;
  const buffer = new ArrayBuffer(44 + dataSize);
  const view = new DataView(buffer);
  let o = 0;
  const writeStr = (s) => {
    for (let i = 0; i < s.length; i++) view.setUint8(o++, s.charCodeAt(i));
  };
  writeStr("RIFF");
  view.setUint32(o, 36 + dataSize, true);
  o += 4;
  writeStr("WAVE");
  writeStr("fmt ");
  view.setUint32(o, 16, true);
  o += 4;
  view.setUint16(o, 1, true);
  o += 2;
  view.setUint16(o, numChannels, true);
  o += 2;
  view.setUint32(o, sampleRate, true);
  o += 4;
  view.setUint32(o, byteRate, true);
  o += 4;
  view.setUint16(o, blockAlign, true);
  o += 2;
  view.setUint16(o, bitsPerSample, true);
  o += 2;
  writeStr("data");
  view.setUint32(o, dataSize, true);
  o += 4;
  for (let i = 0; i < samples.length; i++) {
    view.setInt16(o, samples[i], true);
    o += 2;
  }
  writeFileSync(path, Buffer.from(buffer));
}

mkdirSync(fixturesDir, { recursive: true });

const sr = 48_000;
const silence = new Int16Array(sr * 2).fill(0);
writePcmWav(join(fixturesDir, "silence-30s.wav"), silence, sr);

const pulses = new Int16Array(sr * 2);
for (let i = 0; i < pulses.length; i++) {
  pulses[i] = i % 800 < 400 ? 9000 : -9000;
}
writePcmWav(join(fixturesDir, "pulses-30s.wav"), pulses, sr);

const loud = new Int16Array(sr * 3);
for (let i = 0; i < loud.length; i++) {
  loud[i] = (i % 2 === 0 ? 1 : -1) * 32_000;
}
writePcmWav(join(fixturesDir, "loud-noise.wav"), loud, sr);

/** ~50 ms full-scale blip in the middle of silence — shorter than typical `holdMs` (e.g. 500 ms). */
const blipLen = sr * 2;
const blip = new Int16Array(blipLen).fill(0);
const blipStart = Math.floor(sr * 0.25);
const blipDur = Math.floor(0.05 * sr);
for (let i = 0; i < blipDur; i++) {
  blip[blipStart + i] = (i % 2 === 0 ? 1 : -1) * 30_000;
}
writePcmWav(join(fixturesDir, "blip-50ms.wav"), blip, sr);

console.log("e2e fixtures written to", fixturesDir);
