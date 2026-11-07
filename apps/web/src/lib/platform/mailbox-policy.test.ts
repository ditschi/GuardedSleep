import { describe, expect, it } from "vitest";
import {
  connectSources,
  cspAllows,
  defaultMailboxUrls,
  deploymentConnectSources,
} from "./mailbox-policy";
import { buildCsp } from "../../../csp/csp";

const https = { protocol: "https:", hostname: "web.test", port: "" };

describe("connectSources and cspAllows", () => {
  it("reads connect-src of the policy the build emits", () => {
    const csp = buildCsp({ connectOrigins: ["wss://relay.example.org", "ws://10.0.0.2:8787"] });
    expect(connectSources(csp)).toEqual([
      "'self'",
      "wss://relay.example.org",
      "ws://10.0.0.2:8787",
    ]);
    expect(connectSources("default-src 'self'")).toEqual([]);
  });

  it("allows the serving host, listed origins and nothing else", () => {
    const sources = connectSources(buildCsp({ connectOrigins: ["wss://relay.example.org"] }));
    const ok = (url: string): boolean => cspAllows(sources, https, url);
    expect(ok("wss://web.test/signaling")).toBe(true);
    expect(ok("wss://relay.example.org/sig")).toBe(true);
    expect(ok("wss://relay.example.org:443/sig")).toBe(true);
    expect(ok("wss://relay.example.org:444/sig")).toBe(false);
    expect(ok("wss://other.example/sig")).toBe(false);
    expect(ok("ws://web.test/signaling")).toBe(false);
    expect(ok("https://web.test")).toBe(false);
    expect(ok("not a url")).toBe(false);
  });

  it("'self' on a plain http page is ws with the same port; dev adds loopback with any port", () => {
    const http = { protocol: "http:", hostname: "localhost", port: "8787" };
    expect(cspAllows(["'self'"], http, "ws://localhost:8787/x")).toBe(true);
    expect(cspAllows(["'self'"], http, "ws://localhost:9999")).toBe(false);
    const dev = connectSources(buildCsp({ dev: true, meta: true }));
    expect(cspAllows(dev, http, "ws://127.0.0.1:5555")).toBe(true);
    expect(cspAllows(dev, http, "ws://192.168.1.2:5555")).toBe(false);
  });

  it("reads the policy from the page's meta element", () => {
    const doc = (content: string | null) => ({
      querySelector: () => (content === null ? null : { getAttribute: () => content }),
    });
    expect(deploymentConnectSources(doc(buildCsp({ meta: true })))).toEqual(["'self'"]);
    expect(deploymentConnectSources(doc(null))).toBeNull();
  });
});

describe("defaultMailboxUrls", () => {
  const here = { protocol: "https:", hostname: "gs.local", host: "gs.local:8443" };
  it("is the serving host's /signaling, or the dev companion", () => {
    expect(defaultMailboxUrls(here, false)).toEqual(["wss://gs.local:8443/signaling"]);
    expect(defaultMailboxUrls({ ...here, protocol: "http:" }, false)).toEqual([
      "ws://gs.local:8443/signaling",
    ]);
    expect(defaultMailboxUrls(here, true)).toEqual(["ws://gs.local:8787"]);
  });
});
