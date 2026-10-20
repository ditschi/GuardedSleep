import type { CoreApi } from "@guardedsleep/core-wasm";
import { loadCore } from "@guardedsleep/core-wasm/web";
import wasmUrl from "@guardedsleep/core-wasm/pkg/guardedsleep_core_bg.wasm?url";

let core: Promise<CoreApi> | null = null;

/** Loads the Rust core (WASM) once per page. */
export function getCore(): Promise<CoreApi> {
  core ??= loadCore(wasmUrl).catch((e: unknown) => {
    core = null;
    throw e;
  });
  return core;
}
