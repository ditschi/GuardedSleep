// Measurement of one Nostr relay as a signaling mailbox (the spike of alpha-connection-model).
//   node scripts/spike/relay-probe.mjs wss://host [--events 20] [--spacing-ms 600] [--quick]
// Phases, in this order (the one most likely to trip a limit comes last):
//   connect        time to an open WebSocket
//   delivery       N ephemeral events at 1.7 per second from one socket, received on a second socket
//   sizes          2, 4, 8 and 16 KiB events: accepted? delivered?
//   retention      does a NEW subscription get an event published a moment ago (strfry keeps
//                  ephemeral events for about 5 minutes)
//   created_at     events 1 h, 15 min, 5 min and 1 min in the past and the future: accepted?
//                  (spike unknown 9; the tolerance differs per relay software)
//   burst          12 events of 2 KiB without pause (rate-limit behaviour), unless --quick
// Throwaway keys, random content, a random tag: nothing derived from a pairing. About 45 events
// per relay in about a minute; every wait has a deadline.
import { pathToFileURL } from "node:url";
import { Conn, parseArgs, percentile, signEvents, sleep, subscribe } from "./lib.mjs";

const DELIVERY_WAIT_MS = 10_000;
const SIZES = [2048, 4096, 8192, 16384];
const OFFSETS_S = [-3600, -900, -300, -60, 60, 300, 600, 3600];
const BURST_EVENTS = 12;

/** Publishes `events` (parsed `gs-soak sign` lines) in order and collects the OK answers. */
async function publish(pub, events, spacingMs) {
  const sentAt = new Map();
  const answers = new Map();
  const off = pub.on((frame) => {
    if (frame && frame[0] === "OK")
      answers.set(frame[1], {
        ok: frame[2] === true,
        message: String(frame[3] ?? "").slice(0, 100),
      });
  });
  for (const e of events) {
    sentAt.set(e.id, performance.now());
    if (!pub.send(e.frame)) break;
    if (spacingMs > 0) await sleep(spacingMs);
  }
  return { sentAt, answers, stop: off };
}

async function waitForAnswers(answers, count, timeoutMs) {
  const end = performance.now() + timeoutMs;
  while (answers.size < count && performance.now() < end) await sleep(50);
}

export async function probeRelay(url, options = {}) {
  const { events: count = 20, spacingMs = 600, quick = false } = options;
  const report = { url, started: new Date().toISOString() };
  const pub = new Conn(url);
  const sub = new Conn(url);
  try {
    // connect
    try {
      const [pubMs, subMs] = await Promise.all([pub.open(), sub.open()]);
      report.connect_ms = Math.round(Math.min(pubMs, subMs));
    } catch (e) {
      return { ...report, connected: false, error: e.message };
    }
    report.connected = true;

    // delivery
    const tag = signEvents(null, ["1"])[0].tag;
    const delivery = signEvents(
      tag,
      Array.from({ length: count }, () => "1024"),
    );
    const kind = delivery[0].kind;
    const received = new Map();
    sub.on((frame, at) => {
      if (frame && frame[0] === "EVENT" && frame[2]?.id) received.set(frame[2].id, at);
    });
    report.subscribed = await subscribe(sub, "probe", kind, tag);
    const sent = await publish(pub, delivery, spacingMs);
    await waitForAnswers(sent.answers, delivery.length, DELIVERY_WAIT_MS);
    const settleUntil = performance.now() + DELIVERY_WAIT_MS;
    while (received.size < delivery.length && performance.now() < settleUntil) await sleep(50);
    sent.stop();
    const latencies = [];
    for (const e of delivery) {
      const at = received.get(e.id);
      if (at !== undefined) latencies.push(Math.round(at - sent.sentAt.get(e.id)));
    }
    const answered = [...sent.answers.values()];
    report.delivery = {
      sent: delivery.length,
      accepted: answered.filter((a) => a.ok).length,
      refused: answered.filter((a) => !a.ok).length,
      no_answer: delivery.length - answered.length,
      delivered: latencies.length,
      p50_ms: percentile(latencies, 50),
      p95_ms: percentile(latencies, 95),
      max_ms: latencies.length ? Math.max(...latencies) : null,
      refusal_messages: [...new Set(answered.filter((a) => !a.ok).map((a) => a.message))],
    };

    // sizes
    const sizeEvents = signEvents(tag, SIZES.map(String));
    const sizeSent = await publish(pub, sizeEvents, 1000);
    await waitForAnswers(sizeSent.answers, sizeEvents.length, 5_000);
    await sleep(1_000);
    sizeSent.stop();
    report.sizes = Object.fromEntries(
      sizeEvents.map((e) => {
        const answer = sizeSent.answers.get(e.id);
        return [
          e.bytes,
          {
            accepted: answer?.ok ?? null,
            delivered: received.has(e.id),
            message: answer?.ok === false ? answer.message : undefined,
          },
        ];
      }),
    );

    // retention: a new subscription asks for an event published a moment ago
    const [keep] = signEvents(tag, ["512"]);
    const keepSent = await publish(pub, [keep], 0);
    await waitForAnswers(keepSent.answers, 1, 5_000);
    keepSent.stop();
    await sleep(1_500);
    const late = new Conn(url);
    try {
      await late.open();
      let served = false;
      late.on((frame) => {
        if (frame && frame[0] === "EVENT" && frame[2]?.id === keep.id) served = true;
      });
      await subscribe(late, "late", kind, tag, { sinceOffsetS: -120 });
      report.retention = { served_to_new_subscription: served };
    } catch (e) {
      report.retention = { error: e.message };
    } finally {
      late.close();
    }

    // created_at
    const offsetEvents = signEvents(
      tag,
      OFFSETS_S.map((o) => `256:${o}`),
    );
    const offsetSent = await publish(pub, offsetEvents, 700);
    await waitForAnswers(offsetSent.answers, offsetEvents.length, 5_000);
    offsetSent.stop();
    report.created_at = Object.fromEntries(
      offsetEvents.map((e) => {
        const answer = offsetSent.answers.get(e.id);
        return [
          e.offset_s,
          {
            accepted: answer?.ok ?? null,
            message: answer?.ok === false ? answer.message : undefined,
          },
        ];
      }),
    );

    // burst
    if (!quick) {
      await sleep(3_000);
      const burst = signEvents(
        tag,
        Array.from({ length: BURST_EVENTS }, () => "2048"),
      );
      const burstSent = await publish(pub, burst, 0);
      await waitForAnswers(burstSent.answers, burst.length, 8_000);
      burstSent.stop();
      const answers = [...burstSent.answers.values()];
      const firstRefused = burst.findIndex((e) => burstSent.answers.get(e.id)?.ok === false);
      report.burst = {
        sent: burst.length,
        accepted: answers.filter((a) => a.ok).length,
        refused: answers.filter((a) => !a.ok).length,
        no_answer: burst.length - answers.length,
        first_refused_index: firstRefused < 0 ? null : firstRefused,
        refusal_messages: [...new Set(answers.filter((a) => !a.ok).map((a) => a.message))],
      };
    }
    report.socket_dropped = pub.closed || sub.closed;
  } catch (e) {
    report.error = `probe failed: ${e.message}`;
  } finally {
    pub.close();
    sub.close();
  }
  return report;
}

/**
 * ok = a usable mailbox for our use: connected, subscription worked, at least 90 % of the paced
 * events arrived with p95 under 3 s, and 2 and 4 KiB events are accepted. Everything else
 * (burst limits, retention, created_at windows) is information, not a failure.
 */
export function judge(report) {
  if (!report.connected) return { ok: false, why: report.error ?? "no connection" };
  if (report.error) return { ok: false, why: report.error };
  const d = report.delivery;
  if (!report.subscribed) return { ok: false, why: "subscription not answered" };
  if (d.delivered < Math.ceil(d.sent * 0.9))
    return { ok: false, why: `delivered ${d.delivered}/${d.sent}` };
  if (d.p95_ms === null || d.p95_ms >= 3000) return { ok: false, why: `p95 ${d.p95_ms} ms` };
  for (const size of [2048, 4096]) {
    if (report.sizes?.[size]?.accepted !== true)
      return { ok: false, why: `${size} bytes not accepted` };
  }
  return { ok: true, why: "" };
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const args = parseArgs(process.argv.slice(3), {
    events: { default: 20, number: true },
    "spacing-ms": { default: 600, number: true },
    quick: { flag: true, default: false },
  });
  const url = process.argv[2];
  if (!url?.startsWith("wss://")) {
    console.error("usage: relay-probe.mjs wss://host [--events N] [--spacing-ms N] [--quick]");
    process.exit(2);
  }
  const report = await probeRelay(url, {
    events: args.events,
    spacingMs: args["spacing-ms"],
    quick: args.quick,
  });
  console.log(JSON.stringify({ ...report, judged: judge(report) }, null, 2));
  process.exit(0);
}
