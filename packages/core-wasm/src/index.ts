import type {
  BuildInviteResult,
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
  parseInvite(text: string, nowEpochS: number): string;
  buildInvite(psk: Uint8Array, urls: string[], expiresAtEpochS: number): string;
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
  /** Parses a scanned or pasted v3 invite. `nowEpochS` is the wall clock in epoch seconds. */
  parseInvite(text: string, nowEpochS: number): ParseInviteResult;
  /** Builds the v3 invite text; `{ ok: false }` when the inputs are not a valid invite. */
  buildInvite(psk: Uint8Array, urls: string[], expiresAtEpochS: number): BuildInviteResult;
  /** Evaluates one `vectors/conformance.json` case; the result is JSON, `{error}` for a bad case. */
  conformanceEval(caseJson: string): unknown;
}

export function createCoreApi(mod: RawCoreModule): CoreApi {
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
      const out: unknown = JSON.parse(mod.parseInvite(text, nowEpochS));
      return toParseInviteResult(out);
    },
    buildInvite(psk, urls, expiresAtEpochS) {
      try {
        return { ok: true, text: mod.buildInvite(psk, urls, expiresAtEpochS) };
      } catch (e) {
        // The core throws its snake_case error key; anything else is a genuine failure.
        const key = e instanceof Error ? e.message : e;
        if (isInviteErrorKey(key)) return { ok: false, error: key };
        throw e;
      }
    },
    conformanceEval: (caseJson) => JSON.parse(mod.conformanceEval(caseJson)) as unknown,
  };
}

interface RawInviteResult {
  type?: unknown;
  error?: unknown;
  psk_b64url?: unknown;
  urls?: unknown;
  expires_at_epoch_s?: unknown;
}

function toParseInviteResult(out: unknown): ParseInviteResult {
  const o = out as RawInviteResult | null;
  if (o?.type === "error" && isInviteErrorKey(o.error)) return { ok: false, error: o.error };
  if (
    o?.type === "ok" &&
    typeof o.psk_b64url === "string" &&
    Array.isArray(o.urls) &&
    o.urls.every((u): u is string => typeof u === "string") &&
    typeof o.expires_at_epoch_s === "number"
  ) {
    return {
      ok: true,
      invite: {
        psk: decodeBase64Url(o.psk_b64url),
        urls: o.urls,
        expiresAtEpochS: o.expires_at_epoch_s,
      },
    };
  }
  throw new Error("core returned an unknown invite result shape");
}

function decodeBase64Url(s: string): Uint8Array {
  const bin = atob(s.replaceAll("-", "+").replaceAll("_", "/"));
  return Uint8Array.from(bin, (c) => c.charCodeAt(0));
}
