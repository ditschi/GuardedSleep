import { test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { pairDual } from "./utils/pairing";
import {
  logRtcRoundTripSamples,
  parentSendListen,
  readInboundAudioStats,
  waitForConnected,
} from "./utils/probes";

test("audio latency metrics (log only, no hard SLA)", async () => {
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "pulses-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
  });
  try {
    await pairDual(dual);
    await waitForConnected(dual.babyPage);
    await waitForConnected(dual.parentPage);
    await parentSendListen(dual.parentPage, true);

    for (let i = 0; i < 5; i++) {
      await logRtcRoundTripSamples(dual.parentPage);
      await new Promise((r) => setTimeout(r, 400));
    }

    const a = await readInboundAudioStats(dual.parentPage);
    await new Promise((r) => setTimeout(r, 600));
    const b = await readInboundAudioStats(dual.parentPage);
    console.info("[e2e] inbound-rtp snapshot A", a);
    console.info("[e2e] inbound-rtp snapshot B", b);
    console.info(
      "[e2e] pulse-marker worklet latency: deferred (see ADR-007); RTT + packet deltas logged only.",
    );
  } finally {
    await dual.dispose();
  }
});
