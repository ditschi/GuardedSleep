import { createHash } from "node:crypto";
import type { AddressInfo } from "node:net";
import { WebSocketServer, type WebSocket } from "ws";

/*
 * A minimal NIP-01 relay for the browser E2E (`alpha-connection-model` task 3.5): no public relay
 * is ever used in CI. It answers EVENT with OK, serves REQ with EOSE, forwards a new event to
 * every matching subscription of other connections, and models the behaviour that matters for
 * ephemeral signaling events:
 *
 * - strfry keeps ephemeral events for about 300 s and serves them to any new subscription, so
 *   `retentionMs` defaults to that (the clients must not trust a stored event);
 * - `neverDeliver` accepts events with `OK true` and delivers none (spike: some relays do).
 *
 * It checks the event id (SHA-256 of the NIP-01 serialisation) and nothing else: the signature is
 * verified by the clients, which is the point of the test. Docs: NIP-01,
 * https://github.com/nostr-protocol/nips/blob/master/01.md
 */

export interface NostrEvent {
  id: string;
  pubkey: string;
  created_at: number;
  kind: number;
  tags: string[][];
  content: string;
  sig: string;
}

interface Filter {
  kinds?: number[];
  authors?: string[];
  "#t"?: string[];
  since?: number;
  limit?: number;
}

interface Stored {
  at: number;
  event: NostrEvent;
}

export interface NostrRelayOptions {
  /** How long an ephemeral event is served to new subscriptions (default 300 s, like strfry). */
  retentionMs?: number;
  /** Accept every event, deliver none. */
  neverDeliver?: boolean;
}

const DEFAULT_RETENTION_MS = 300_000;

function eventId(e: NostrEvent): string {
  const text = JSON.stringify([0, e.pubkey, e.created_at, e.kind, e.tags, e.content]);
  return createHash("sha256").update(text).digest("hex");
}

function matches(f: Filter, e: NostrEvent): boolean {
  const tag = e.tags.find((t) => t[0] === "t")?.[1];
  return (
    (f.kinds === undefined || f.kinds.includes(e.kind)) &&
    (f.authors === undefined || f.authors.includes(e.pubkey)) &&
    (f["#t"] === undefined || (tag !== undefined && f["#t"].includes(tag))) &&
    (f.since === undefined || e.created_at >= f.since)
  );
}

export class NostrTestRelay {
  /** Every accepted event, in arrival order (for assertions about what a relay operator sees). */
  readonly received: NostrEvent[] = [];
  /** How many REQ frames arrived. */
  reqCount = 0;
  private readonly server: WebSocketServer;
  private readonly subs = new Map<WebSocket, Map<string, Filter[]>>();
  private stored: Stored[] = [];
  private readonly retentionMs: number;
  neverDeliver: boolean;

  constructor(options: NostrRelayOptions = {}) {
    this.retentionMs = options.retentionMs ?? DEFAULT_RETENTION_MS;
    this.neverDeliver = options.neverDeliver ?? false;
    this.server = new WebSocketServer({ host: "127.0.0.1", port: 0 });
    this.server.on("connection", (ws) => this.onConnection(ws));
  }

  /** `ws://127.0.0.1:port`, once listening. */
  async start(): Promise<string> {
    if (!this.server.address()) {
      await new Promise<void>((resolve) => this.server.once("listening", () => resolve()));
    }
    return this.url;
  }

  get url(): string {
    const address = this.server.address() as AddressInfo;
    return `ws://127.0.0.1:${address.port}`;
  }

  async stop(): Promise<void> {
    for (const ws of this.server.clients) ws.terminate();
    await new Promise<void>((resolve) => this.server.close(() => resolve()));
  }

  /** The distinct public keys that published, i.e. the daily keys of the two roles. */
  authors(): string[] {
    return [...new Set(this.received.map((e) => e.pubkey))];
  }

  private onConnection(ws: WebSocket): void {
    this.subs.set(ws, new Map());
    ws.on("message", (data) => this.onFrame(ws, data.toString()));
    ws.on("close", () => this.subs.delete(ws));
    ws.on("error", () => this.subs.delete(ws));
  }

  private send(ws: WebSocket, frame: unknown[]): void {
    if (ws.readyState === ws.OPEN) ws.send(JSON.stringify(frame));
  }

  private onFrame(ws: WebSocket, text: string): void {
    let frame: unknown;
    try {
      frame = JSON.parse(text);
    } catch {
      return;
    }
    if (!Array.isArray(frame)) return;
    const [verb, a, ...rest] = frame as unknown[];
    if (verb === "EVENT") this.onEvent(ws, a as NostrEvent);
    else if (verb === "REQ" && typeof a === "string") this.onReq(ws, a, rest as Filter[]);
    else if (verb === "CLOSE" && typeof a === "string") this.subs.get(ws)?.delete(a);
  }

  private onEvent(ws: WebSocket, event: NostrEvent): void {
    if (typeof event?.id !== "string" || eventId(event) !== event.id) {
      this.send(ws, ["OK", String(event?.id), false, "invalid: bad id"]);
      return;
    }
    this.send(ws, ["OK", event.id, true, ""]);
    this.received.push(event);
    if (this.neverDeliver) return;
    this.stored.push({ at: Date.now(), event });
    for (const [other, subs] of this.subs) {
      if (other === ws) continue;
      for (const [id, filters] of subs) {
        if (filters.some((f) => matches(f, event))) this.send(other, ["EVENT", id, event]);
      }
    }
  }

  private onReq(ws: WebSocket, id: string, filters: Filter[]): void {
    this.reqCount += 1;
    this.subs.get(ws)?.set(id, filters);
    const now = Date.now();
    this.stored = this.stored.filter((s) => now - s.at <= this.retentionMs);
    if (!this.neverDeliver) {
      let served = this.stored.filter((s) => filters.some((f) => matches(f, s.event)));
      const limit = Math.min(...filters.map((f) => f.limit ?? Infinity));
      if (Number.isFinite(limit)) served = served.slice(0, limit);
      for (const s of served) this.send(ws, ["EVENT", id, s.event]);
    }
    this.send(ws, ["EOSE", id]);
  }
}
