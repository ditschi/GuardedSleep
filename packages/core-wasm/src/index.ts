import type { LinkAction, LinkConfig, LinkInput } from "./types.js";
import { isLinkAction } from "./types.js";

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
  };
}
