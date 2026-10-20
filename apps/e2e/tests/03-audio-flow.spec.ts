import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { pairDual } from "./utils/pairing";
import { parentSendListen, readInboundAudioStats, waitForConnected } from "./utils/probes";

test("audio: parent listen enables inbound RTP", async () => {
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "pulses-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
  });
  try {
    await pairDual(dual);
    await waitForConnected(dual.babyPage);
    await waitForConnected(dual.parentPage);

    await parentSendListen(dual.parentPage, true);

    let packets = 0;
    const deadline = Date.now() + 25_000;
    while (Date.now() < deadline) {
      const s = await readInboundAudioStats(dual.parentPage);
      packets = s?.packetsReceived ?? 0;
      if (packets > 0) break;
      await new Promise((r) => setTimeout(r, 250));
    }
    expect(packets).toBeGreaterThan(0);
  } finally {
    await dual.dispose();
  }
});
