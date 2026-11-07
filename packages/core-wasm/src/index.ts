import type {
  BuildInviteResult,
  InviteMailbox,
  LinkAction,
  LinkConfig,
  LinkInput,
  ParseInviteResult,
} from "./types.js";
import { isInviteErrorKey, isLinkAction } from "./types.js";

export * from "./types.js";

/** The subset of the wasm-bindgen module this wrapper needs (web and nodejs builds both fit). */
export interface RawCoreModule {
  WasmLink: new (configJson: string) => {
    handle(nowMs: number, inputJson: string): string;
    roomId(): string;
    free(): void;
  };
  messageKeys(): string;
  deriveRoomId(psk: Uint8Array): string;
  parseInvite(text: string, nowEpochS: number, allowDevLoopback: boolean): string;
  buildInvite(
    psk: Uint8Array,
    mailboxesJson: string,
    expiresAtEpochS: number,
    allowDevLoopback: boolean,
  ): string;
  filterOnlyLocal(mailboxesJson: string, projectHost: string): string;
  conformanceEval(caseJson: string): string;
}

export interface CoreLink {
  /** `nowMs` must be monotonic (see `LinkDriver`). Throws a typed error on malformed core output. */
  handle(nowMs: number, input: LinkInput): LinkAction[];
  roomId(): string;
  free(): void;
}

export interface CoreApi {
  /** Throws if the core rejects the config (invalid PSK or timing). */
  createLink(config: LinkConfig): CoreLink;
  messageKeys(): string[];
  deriveRoomId(psk: Uint8Array): string;
  /** Parses a scanned or pasted v4 invite. `nowEpochS` is the wall clock in epoch seconds. */
  parseInvite(text: string, nowEpochS: number): ParseInviteResult;
  /**
   * Builds the v4 invite text; `{ ok: false }` when the inputs are not a valid invite or the
   * protected entries alone exceed the size budget. Over-budget lists are trimmed by the core.
   */
  buildInvite(
    psk: Uint8Array,
    mailboxes: InviteMailbox[],
    expiresAtEpochS: number,
  ): BuildInviteResult;
  /**
   * "Only same Wi-Fi or my own server" for a new invite: the mailboxes without the relay entries
   * whose host is `projectHost` (lower-case host). The core applies the same rule to its own set
   * and to received updates when `LinkConfig.only_local` is on.
   */
  filterOnlyLocal(mailboxes: InviteMailbox[], projectHost: string): InviteMailbox[];
  /** Evaluates one `vectors/conformance.json` case; the result is JSON, `{error}` for a bad case. */
  conformanceEval(caseJson: string): unknown;
}

export interface CoreOptions {
  /**
   * Development builds only (`import.meta.env.DEV`): also accept loopback `l:` mailboxes, because
   * `just dev` and the browser E2E run the signaling server on 127.0.0.1. Release builds leave it off.
   */
  allowDevLoopback?: boolean;
}

export function createCoreApi(mod: RawCoreModule, options: CoreOptions = {}): CoreApi {
  const allowDevLoopback = options.allowDevLoopback ?? false;
  return {
    createLink(config) {
      const raw = new mod.WasmLink(JSON.stringify(config));
      return {
        handle(nowMs, input) {
          const out: unknown = JSON.parse(raw.handle(nowMs, JSON.stringify(input)));
          if (!Array.isArray(out) || !out.every(isLinkAction)) {
            throw new Error("core returned an unknown action shape");
          }
          return out;
        },
        roomId: () => raw.roomId(),
        free: () => raw.free(),
      };
    },
    messageKeys: () => JSON.parse(mod.messageKeys()) as string[],
    deriveRoomId: (psk) => mod.deriveRoomId(psk),
    parseInvite(text, nowEpochS) {
      const out: unknown = JSON.parse(mod.parseInvite(text, nowEpochS, allowDevLoopback));
      return toParseInviteResult(out);
    },
    buildInvite(psk, mailboxes, expiresAtEpochS) {
      try {
        const json = JSON.stringify(mailboxes);
        return { ok: true, text: mod.buildInvite(psk, json, expiresAtEpochS, allowDevLoopback) };
      } catch (e) {
        // The core throws its snake_case error key; anything else is a genuine failure.
        const key = e instanceof Error ? e.message : e;
        if (isInviteErrorKey(key)) return { ok: false, error: key };
        throw e;
      }
    },
    filterOnlyLocal(mailboxes, projectHost) {
      const out: unknown = JSON.parse(mod.filterOnlyLocal(JSON.stringify(mailboxes), projectHost));
      return out as InviteMailbox[];
    },
    conformanceEval: (caseJson) => JSON.parse(mod.conformanceEval(caseJson)) as unknown,
  };
}

interface RawInviteResult {
  type?: unknown;
  error?: unknown;
  psk_b64url?: unknown;
  mailboxes?: unknown;
  expires_at_epoch_s?: unknown;
}

function toParseInviteResult(out: unknown): ParseInviteResult {
  const o = out as RawInviteResult | null;
  if (o?.type === "error" && isInviteErrorKey(o.error)) return { ok: false, error: o.error };
  if (
    o?.type === "ok" &&
    typeof o.psk_b64url === "string" &&
    Array.isArray(o.mailboxes) &&
    o.mailboxes.every(isInviteMailbox) &&
    typeof o.expires_at_epoch_s === "number"
  ) {
    return {
      ok: true,
      invite: {
        psk: decodeBase64Url(o.psk_b64url),
        mailboxes: o.mailboxes,
        expiresAtEpochS: o.expires_at_epoch_s,
      },
    };
  }
  throw new Error("core returned an unknown invite result shape");
}

function isInviteMailbox(v: unknown): v is InviteMailbox {
  const m = v as Partial<Record<keyof InviteMailbox, unknown>> | null;
  return (m?.kind === "relay" || m?.kind === "lan") && typeof m.url === "string";
}

function decodeBase64Url(s: string): Uint8Array {
  const bin = atob(s.replaceAll("-", "+").replaceAll("_", "/"));
  return Uint8Array.from(bin, (c) => c.charCodeAt(0));
}
