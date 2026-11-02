import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { bytesPerPacket, inboundAudio, rtcTrackerStub, sentAudioEnergy } from "./utils/rtc";
import { CLIPBOARD_PERMISSIONS, pairWeb } from "./utils/web";

/**
 * Audio path with a fake microphone: the baby device sends digital silence (a closed gate) until
 * the parent listens or a noise alert opens it, and the parent receives audible packets only then.
 * The probes read the browser's own WebRTC statistics; nothing in the page is instrumented beyond
 * recording the peer connections (`rtcTrackerStub`).
 *
 * Two observations make up "audible": the baby's microphone source after the gate adds energy only
 * while the gate is open, and the Opus frames the parent receives are much bigger than for silence.
 */

const pulses = join(fixturesDir, "pulses-30s.wav");
const silence = join(fixturesDir, "silence-30s.wav");

/** Source energy added per second below which the gate counts as closed. */
const GATE_CLOSED_ENERGY_PER_S = 0.02;
/** Loud frames must be at least this many times the size of silent ones. */
const LOUD_FRAME_FACTOR = 1.5;

/** Energy the baby's source adds during the next `windowMs`, per second. */
async function energyRate(page: Parameters<typeof sentAudioEnergy>[0], windowMs: number) {
  const before = await sentAudioEnergy(page);
  await page.waitForTimeout(windowMs);
  return ((await sentAudioEnergy(page)) - before) / (windowMs / 1000);
}

test("listen: audio reaches the parent only while Listen is on", async () => {
  test.setTimeout(180_000);
  const dual = await launchDualContexts({
    babyFakeWav: pulses,
    parentFakeWav: silence,
    permissions: CLIPBOARD_PERMISSIONS,
  });
  const { babyPage: baby, parentPage: parent } = dual;
  try {
    await dual.baby.addInitScript(rtcTrackerStub);
    await dual.parent.addInitScript(rtcTrackerStub);
    await pairWeb(dual);

    // The pulses are loud enough for the default threshold. This test is about Listen, so make
    // the baby device least sensitive (threshold above the pulse level): no alert takes the screen.
    await baby.getByTestId("baby_settings").click();
    await baby.getByTestId("settings_sensitivity").fill("0");
    await baby.getByTestId("settings_back").click();

    // Nobody listens: packets flow (the link is alive) but they carry no sound.
    await expect
      .poll(async () => (await inboundAudio(parent)).packets, { timeout: 30_000 })
      .toBeGreaterThan(0);
    expect(await energyRate(baby, 2_000)).toBeLessThan(GATE_CLOSED_ENERGY_PER_S);
    const quietFrames = await bytesPerPacket(parent, 2_000);

    const listen = parent.getByTestId("parent_listen");
    await expect(listen).toBeEnabled();
    await listen.focus();
    await parent.keyboard.press("Enter");
    await expect(listen).toHaveAttribute("data-listen", "locked");
    await expect
      .poll(() => energyRate(baby, 1_000), { timeout: 20_000 })
      .toBeGreaterThan(GATE_CLOSED_ENERGY_PER_S);
    await parent.waitForTimeout(1_000);
    expect(await bytesPerPacket(parent, 3_000)).toBeGreaterThan(quietFrames * LOUD_FRAME_FACTOR);

    // Listen off: the gate closes again.
    await parent.keyboard.press("Enter");
    await expect(listen).toHaveAttribute("data-listen", "off");
    await expect
      .poll(() => energyRate(baby, 1_000), { timeout: 20_000 })
      .toBeLessThan(GATE_CLOSED_ENERGY_PER_S);
    await parent.waitForTimeout(1_000);
    expect(await bytesPerPacket(parent, 3_000)).toBeLessThan(quietFrames * LOUD_FRAME_FACTOR);
  } finally {
    await dual.dispose();
  }
});

test("noise alert: the gate opens by itself and the parent hears the sound", async () => {
  test.setTimeout(180_000);
  const dual = await launchDualContexts({
    babyFakeWav: pulses,
    parentFakeWav: silence,
    permissions: CLIPBOARD_PERMISSIONS,
  });
  const { babyPage: baby, parentPage: parent } = dual;
  try {
    await dual.baby.addInitScript(rtcTrackerStub);
    await dual.parent.addInitScript(rtcTrackerStub);
    await pairWeb(dual);
    await expect
      .poll(async () => (await inboundAudio(parent)).packets, { timeout: 30_000 })
      .toBeGreaterThan(0);

    await expect(parent.getByTestId("alert_title")).toBeVisible({ timeout: 90_000 });
    await expect
      .poll(() => energyRate(baby, 1_000), { timeout: 20_000 })
      .toBeGreaterThan(GATE_CLOSED_ENERGY_PER_S);
    await parent.waitForTimeout(1_000);
    // Pulses and silence differ by far more than the factor; the alert screen has no quiet frames
    // to compare with, so the absolute size is checked: silence is about 35 bytes per frame.
    expect(await bytesPerPacket(parent, 3_000)).toBeGreaterThan(50);
  } finally {
    await dual.dispose();
  }
});
