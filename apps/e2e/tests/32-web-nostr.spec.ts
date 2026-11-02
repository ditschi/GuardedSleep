import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { NostrTestRelay, type NostrEvent } from "./utils/nostr-relay";
import { CLIPBOARD_PERMISSIONS, pasteCode, showCode, waitForBarKind } from "./utils/web";

/**
 * The Nostr mailbox from the user's side (`alpha-connection-model` task 3.5): two browsers pair and
 * monitor over a local test relay while their own signaling server address leads nowhere. No
 * public relay is used. What the relay operator could read is checked too: every event has the one
 * kind and one daily tag, the content is sealed base64url, and a verified link publishes nothing.
 */

const silence = join(fixturesDir, "silence-30s.wav");
/** The discard port: nothing listens, so the own relay can never carry the pairing. */
const NO_SERVER = "ws://127.0.0.1:9";
const NOSTR_KIND = 24861;

/** What the relay received, decoded as far as an operator could: nothing readable. */
function expectSealed(events: NostrEvent[]): void {
  expect(events.length).toBeGreaterThan(0);
  for (const e of events) {
    expect(e.kind).toBe(NOSTR_KIND);
    expect(e.tags).toHaveLength(1);
    expect(e.tags[0]?.[0]).toBe("t");
    expect(e.tags[0]?.[1]).toMatch(/^[A-Za-z0-9_-]{22}$/);
    expect(e.content).toMatch(/^[A-Za-z0-9_-]+$/);
    const plain = Buffer.from(e.content, "base64url").toString("latin1");
    for (const secret of ["v=0", "candidate", "fingerprint", "a=sendonly", "sdp"]) {
      expect(plain).not.toContain(secret);
    }
  }
}

test("two browsers pair and monitor over a Nostr relay alone", async () => {
  test.setTimeout(150_000);
  const relay = new NostrTestRelay();
  const url = await relay.start();
  const dual = await launchDualContexts({
    babyFakeWav: silence,
    parentFakeWav: silence,
    signalUrl: NO_SERVER,
    nostrRelays: [url],
    permissions: CLIPBOARD_PERMISSIONS,
  });
  try {
    const code = await showCode(dual.babyPage, NO_SERVER);
    const invite = JSON.parse(code) as { m: string[] };
    expect(invite.m).toContain(`n:${url.replace("ws://", "")}`);
    await pasteCode(dual.parentPage, code);
    await waitForBarKind(dual.parentPage, "connected", 90_000);
    await expect(dual.babyPage.getByTestId("baby_status")).toHaveAttribute(
      "data-state",
      "connected",
      { timeout: 60_000 },
    );

    // Both roles published, under their own daily keys, and only sealed events.
    expect(relay.authors().length).toBe(2);
    expectSealed(relay.received);

    // A verified link holds no mailbox open: nothing more is published for a while.
    await dual.parentPage.waitForTimeout(1_500);
    const settled = relay.received.length;
    await dual.parentPage.waitForTimeout(8_000);
    expect(relay.received.length).toBe(settled);
  } finally {
    await dual.dispose();
    await relay.stop();
  }
});

test("a relay that accepts events and delivers none does not stop the pairing", async () => {
  test.setTimeout(150_000);
  const black = new NostrTestRelay({ neverDeliver: true });
  const good = new NostrTestRelay();
  const urls = [await black.start(), await good.start()];
  const dual = await launchDualContexts({
    babyFakeWav: silence,
    parentFakeWav: silence,
    signalUrl: NO_SERVER,
    nostrRelays: urls,
    permissions: CLIPBOARD_PERMISSIONS,
  });
  try {
    const code = await showCode(dual.babyPage, NO_SERVER);
    await pasteCode(dual.parentPage, code);
    await waitForBarKind(dual.parentPage, "connected", 90_000);
    await expect(dual.babyPage.getByTestId("baby_status")).toHaveAttribute(
      "data-state",
      "connected",
      { timeout: 60_000 },
    );
    // Both relays got the same envelopes; only one passed them on.
    expect(black.received.length).toBeGreaterThan(0);
    expect(good.received.length).toBeGreaterThan(0);
    expectSealed(black.received);
    expectSealed(good.received);
  } finally {
    await dual.dispose();
    await black.stop();
    await good.stop();
  }
});
