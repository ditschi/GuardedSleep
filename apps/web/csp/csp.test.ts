import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import {
  buildCsp,
  configuredConnectOrigins,
  nginxCspHeader,
  NGINX_CSP_FILE,
  validateConnectOrigins,
} from "./csp";
import { PROJECT_SERVER } from "../src/lib/public-lists/generated/public-lists";

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, "../../..");
const read = (path: string): string => readFileSync(join(repo, path), "utf8");

describe("buildCsp", () => {
  it("is the policy of design.md: self only, no inline script, trusted types", () => {
    const csp = buildCsp();
    const directives = new Map(csp.split("; ").map((d) => [d.split(" ")[0], d] as const));
    expect(directives.get("default-src")).toBe("default-src 'self'");
    expect(directives.get("script-src")).toBe("script-src 'self' 'wasm-unsafe-eval'");
    expect(directives.get("connect-src")).toBe("connect-src 'self'");
    expect(directives.get("img-src")).toBe("img-src 'self' data: blob:");
    expect(directives.get("object-src")).toBe("object-src 'none'");
    expect(directives.get("base-uri")).toBe("base-uri 'none'");
    expect(directives.get("frame-ancestors")).toBe("frame-ancestors 'none'");
    expect(directives.get("require-trusted-types-for")).toBe("require-trusted-types-for 'script'");
    expect(directives.get("trusted-types")).toBe("trusted-types guardedsleep svelte-trusted-html");
    expect(csp).not.toMatch(/'unsafe-inline'|'unsafe-eval'|\*/);
  });

  it("adds configured WebSocket origins to connect-src only", () => {
    const csp = buildCsp({ connectOrigins: ["wss://relay.example.org", "ws://10.0.0.2:8787"] });
    expect(csp).toContain("connect-src 'self' wss://relay.example.org ws://10.0.0.2:8787; img-src");
    expect(csp.replace(/connect-src [^;]*; /, "")).toBe(
      buildCsp().replace(/connect-src [^;]*; /, ""),
    );
  });

  it("refuses an origin that could break out of the directive", () => {
    for (const bad of [
      "https://a.example",
      "wss://a.example; script-src *",
      "wss://",
      "a.example",
    ]) {
      expect(() => validateConnectOrigins([bad]), bad).toThrow();
    }
  });

  it("drops frame-ancestors in the meta variant and loosens only styles and sockets in dev", () => {
    expect(buildCsp({ meta: true })).not.toContain("frame-ancestors");
    const dev = buildCsp({ dev: true, meta: true });
    expect(dev).toContain("style-src 'self' 'unsafe-inline'");
    expect(dev).toContain("connect-src 'self' ws://localhost:* ws://127.0.0.1:*");
    expect(dev).toContain("require-trusted-types-for 'script'");
    expect(buildCsp()).not.toContain("unsafe-inline");
  });

  it("reads the data file and the build variable, and allows no blanket wss:", () => {
    // Without a deployment setting the policy allows nothing but the serving host.
    expect(configuredConnectOrigins({})).toEqual([]);
    expect(
      configuredConnectOrigins({ GS_CSP_CONNECT_ORIGINS: "wss://a.example, wss://b.example" }),
    ).toEqual(["wss://a.example", "wss://b.example"]);
    expect(() => configuredConnectOrigins({ GS_CSP_CONNECT_ORIGINS: "http://x" })).toThrow();
    const connect = buildCsp({
      connectOrigins: configuredConnectOrigins({ GS_CSP_CONNECT_ORIGINS: "wss://a.example" }),
    });
    expect(connect).toContain(" wss://a.example");
    expect(connect).not.toMatch(/ wss:[ ;]/);
  });
});

describe("the three places that carry the policy agree", () => {
  it("nginx includes the generated policy on every server and index.html location", () => {
    const conf = read("docker/web/nginx.default.conf");
    // No policy is spelled out in the nginx file: the build writes it (see below).
    expect(conf).not.toMatch(/add_header Content-Security-Policy/);
    expect(conf.match(/^\s*include \/etc\/nginx\/csp-header\.conf;$/gm)).toHaveLength(4);
    const dockerfile = read("docker/web/Dockerfile");
    expect(dockerfile).toContain(
      `/usr/share/nginx/html/${NGINX_CSP_FILE} /etc/nginx/${NGINX_CSP_FILE}`,
    );
  });

  it("the build writes the nginx header line with the same policy as the meta element", async () => {
    const { cspPlugin } = await import("./csp");
    const emitted: { fileName: string; source: string }[] = [];
    const plugin = cspPlugin();
    const generate = plugin.generateBundle as (this: {
      emitFile(file: { type: "asset"; fileName: string; source: string }): void;
    }) => void;
    generate.call({ emitFile: (file) => emitted.push(file) });
    const nginx = emitted.find((f) => f.fileName === NGINX_CSP_FILE);
    expect(nginx?.source).toBe(nginxCspHeader(configuredConnectOrigins(process.env)));
    expect(nginx?.source).toBe(
      `add_header Content-Security-Policy "${buildCsp({ connectOrigins: configuredConnectOrigins(process.env) })}" always;\n`,
    );
    const origins = emitted.find((f) => f.fileName === "csp-connect-origins.json");
    expect(JSON.parse(origins?.source ?? "{}")).toEqual({
      origins: configuredConnectOrigins(process.env),
    });
  });

  it("guardedsleep-server --serve-web sends the same policy", () => {
    const source = read("crates/guardedsleep-server/src/csp.rs");
    const constant = (name: string): string => {
      const match = source.match(
        new RegExp(`const ${name}: &str =\\s*"((?:[^"\\\\]|\\\\.)*)";`, "s"),
      );
      if (!match?.[1]) throw new Error(`const ${name} not found in csp.rs`);
      // A backslash at the end of a line continues the string without the indentation.
      return match[1].replace(/\\\n\s*/g, "");
    };
    // The server fills `connect-src` from the origins file the web build writes; with none it is
    // the bare policy.
    expect(`${constant("BEFORE_CONNECT")}; ${constant("AFTER_CONNECT")}`).toBe(buildCsp());
  });

  it("index.html gets the policy as a meta element first in head (build and dev)", async () => {
    const { cspPlugin } = await import("./csp");
    const plugin = cspPlugin();
    const transform = plugin.transformIndexHtml;
    if (typeof transform !== "object" || !("handler" in transform)) throw new Error("no hook");
    const html = readFileSync(join(here, "../index.html"), "utf8");
    const built = (transform.handler as (h: string, c: object) => string)(html, {});
    expect(built).toMatch(
      /<head>\s*<meta http-equiv="Content-Security-Policy" content="[^"]+" \/>/,
    );
    expect(built).toContain(
      `content="${buildCsp({ connectOrigins: configuredConnectOrigins(process.env), meta: true })}"`,
    );
    expect(built).not.toMatch(/<script(?![^>]*\bsrc=)[^>]*>/);
  });
});

describe("the project server origin", () => {
  it("is in connect-src only for a build with the official-deployment flag", () => {
    expect(configuredConnectOrigins({})).not.toContain(`wss://${PROJECT_SERVER.host}`);
    expect(configuredConnectOrigins({ VITE_GS_OFFICIAL_DEPLOYMENT: "false" })).toEqual(
      configuredConnectOrigins({}),
    );
    expect(configuredConnectOrigins({ VITE_GS_OFFICIAL_DEPLOYMENT: "true" })).toContain(
      `wss://${PROJECT_SERVER.host}`,
    );
  });
});
