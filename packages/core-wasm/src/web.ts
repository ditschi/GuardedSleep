import init, * as wasm from "../pkg/guardedsleep_core.js";
import { createCoreApi, type CoreApi } from "./index.js";

/**
 * Browser build. Pass the URL of `pkg/guardedsleep_core_bg.wasm` (the app imports it with Vite's
 * `?url`); without it the glue code resolves the file relative to its own module URL.
 */
export async function loadCore(wasmUrl?: string | URL): Promise<CoreApi> {
  await init(wasmUrl === undefined ? undefined : { module_or_path: wasmUrl });
  return createCoreApi(wasm);
}
