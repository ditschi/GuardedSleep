import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { logConnectionDiagnostics } from "./utils/connection-diagnostics";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { resolveLanIpv4 } from "./utils/lan-ip";
import { pairDual } from "./utils/pairing";
import { waitDataChannelOpen, waitForConnected } from "./utils/probes";

const e2eSignalPort = process.env.GUARDEDSLEEP_E2E_SIGNAL_PORT ?? "18787";

/**
 * Cross-device LAN smoke: both Chromium contexts load the PWA via the host LAN IP
 * (not 127.0.0.1) and use ws://<LAN-IP>:<port> for signaling — the same URLs a phone
 * on Wi-Fi and a laptop browser need when not using adb reverse or Docker TLS stack.
 *
 * Fails fast when the dev stack is loopback-only or the host has no routable LAN IPv4.
 */
test("LAN IP: baby and parent connect via host LAN address (not loopback)", async () => {
  test.skip(
    process.env.GUARDEDSLEEP_E2E_IN_DOCKER === "1" && !process.env.GUARDEDSLEEP_LAN_IP?.trim(),
    "In docker/e2e-local set GUARDEDSLEEP_LAN_IP to a reachable LAN IPv4 (auto-detect may pick VPN bridges).",
  );

  const lanIp = resolveLanIpv4();
  test.skip(!lanIp, "No LAN IPv4 on host — connect Wi-Fi and retry or set GUARDEDSLEEP_LAN_IP");

  const baseUrl = `http://${lanIp}:5173`;
  const signalWs = `ws://${lanIp}:${e2eSignalPort}`;
  const prevSignal = process.env.E2E_SIGNALING_URL;
  process.env.E2E_SIGNALING_URL = signalWs;

  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
    baseUrl,
  });

  try {
    await dual.babyPage.goto("/");
    const secure = await dual.babyPage.evaluate(() => window.isSecureContext);
    test.skip(
      !secure,
      "This Chromium build does not treat the plain-http LAN origin as secure, so the browser baby cannot open the microphone. Run the LAN check against the HTTPS stack (pnpm run stack:lan).",
    );
    await pairDual(dual);

    try {
      await waitForConnected(dual.babyPage, 60_000);
      await waitForConnected(dual.parentPage, 60_000);
      await waitDataChannelOpen(dual.babyPage, 30_000);
      await waitDataChannelOpen(dual.parentPage, 30_000);
    } catch (err) {
      await logConnectionDiagnostics(`LAN ${lanIp}`, dual.babyPage, dual.parentPage);
      throw err;
    }

    const states = await Promise.all(
      [dual.babyPage, dual.parentPage].map((p) =>
        p.evaluate(
          () =>
            window.__guardedsleep_test?.getSession?.()?.getE2ePeerConnection?.()?.connectionState ??
            null,
        ),
      ),
    );
    expect(states).toEqual(["connected", "connected"]);
    const invite = JSON.parse(
      await dual.babyPage.evaluate(() => window.__guardedsleep_test!.getInviteJson!()),
    ) as { s: string[] };
    expect(invite.s).toEqual([signalWs]);
  } finally {
    if (prevSignal === undefined) delete process.env.E2E_SIGNALING_URL;
    else process.env.E2E_SIGNALING_URL = prevSignal;
    await dual.dispose();
  }
});
