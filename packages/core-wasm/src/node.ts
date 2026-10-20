import { createRequire } from "node:module";
import { createCoreApi, type CoreApi, type RawCoreModule } from "./index.js";

/** Synchronous Node build (`pkg-node/`), used by Vitest and Node tooling. */
export function loadCoreNode(): CoreApi {
  const require = createRequire(import.meta.url);
  const mod = require("../pkg-node/guardedsleep_core.js") as RawCoreModule;
  return createCoreApi(mod);
}
