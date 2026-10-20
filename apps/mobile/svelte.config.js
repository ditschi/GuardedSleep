import adapter from "@sveltejs/adapter-static";

// Only `vite dev` may talk plain ws:// (local signaling); every other invocation gets the
// production policy, so a misdetection fails closed.
const isDev = process.argv.includes("dev");

/** @type {import('@sveltejs/kit').Config} */
const config = {
  compilerOptions: {
    runes: ({ filename }) => (filename.split(/[/\\]/).includes("node_modules") ? undefined : true),
  },
  kit: {
    adapter: adapter({
      fallback: "index.html",
      strict: false,
    }),
    // Static build: Kit emits this as a <meta> tag with hashes for its inline bootstrap script, so
    // frame-ancestors (ignored in meta) is sent by nginx instead. `ws:` is allowed in `vite dev`
    // only (local signaling is plain ws://); production builds allow `wss:` alone. `wasm-unsafe-eval` is needed by the WASM core; `blob:` workers are
    // used by the tick worker (lib/link/clock.ts).
    csp: {
      mode: "hash",
      directives: {
        "default-src": ["self"],
        "script-src": ["self", "wasm-unsafe-eval"],
        "worker-src": ["self", "blob:"],
        "connect-src": isDev ? ["self", "wss:", "ws:"] : ["self", "wss:"],
        "img-src": ["self", "data:", "blob:"],
        "media-src": ["self", "blob:"],
        "style-src": ["self", "unsafe-inline"],
        "base-uri": ["none"],
        "object-src": ["none"],
        "form-action": ["self"],
      },
    },
  },
};

export default config;
