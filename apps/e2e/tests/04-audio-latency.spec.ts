import { test } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { babyDeepLink, parentDeepLink, startSession } from "./utils/pairing";
import {
  logRtcRoundTripSamples,
  parentSendListen,
  readInboundAudioStats,
  waitForConnected,
} from "./utils/probes";

test("audio latency metrics (log only, no hard SLA)", async () => {
  const room = `e2e-${randomUUID().slice(0, 8)}`;
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "pulses-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
  });
  try {
    await dual.babyPage.goto(babyDeepLink(room));
    await dual.parentPage.goto(parentDeepLink(room));
    await startSession(dual.babyPage);
    await startSession(dual.parentPage);
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
