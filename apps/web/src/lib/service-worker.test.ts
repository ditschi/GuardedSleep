import { describe, expect, it } from "vitest";
import { createAppPolicy, registerServiceWorker, type TrustedTypesFactory } from "./service-worker";

const ORIGIN = "https://gs.example";

/** A Trusted Types factory that wraps the accepted URL in a marker object, like the browser. */
function factory(names: string[] = []): TrustedTypesFactory {
  return {
    createPolicy(name, rules) {
      names.push(name);
      return { createScriptURL: (input) => ({ trusted: rules.createScriptURL(input) }) };
    },
  };
}

describe("app Trusted Types policy", () => {
  it("is the single policy of the app and trusts only this origin's worker", () => {
    const names: string[] = [];
    const policy = createAppPolicy(factory(names), ORIGIN);
    expect(names).toEqual(["guardedsleep"]);
    expect(policy.createScriptURL("/sw.js")).toEqual({ trusted: "/sw.js" });
    expect(policy.createScriptURL(`${ORIGIN}/sw.js`)).toEqual({ trusted: `${ORIGIN}/sw.js` });
    for (const bad of [
      "https://evil.example/sw.js",
      "//evil.example/sw.js",
      "/other.js",
      "/sw.js?x=1",
      "data:text/javascript,alert(1)",
      "javascript:alert(1)",
    ]) {
      expect(() => policy.createScriptURL(bad), bad).toThrow(TypeError);
    }
  });
});

describe("registerServiceWorker", () => {
  it("registers the trusted worker URL with the root scope", async () => {
    const calls: Array<[unknown, unknown]> = [];
    await registerServiceWorker(
      {
        serviceWorker: {
          register: (url, options) => {
            calls.push([url, options]);
            return Promise.resolve({});
          },
        },
      },
      factory(),
      ORIGIN,
    );
    expect(calls).toEqual([[{ trusted: "/sw.js" }, { scope: "/" }]]);
  });

  it("registers the plain path where the browser has no Trusted Types", async () => {
    const urls: unknown[] = [];
    await registerServiceWorker(
      {
        serviceWorker: {
          register: (url) => {
            urls.push(url);
            return Promise.resolve({});
          },
        },
      },
      undefined,
      ORIGIN,
    );
    expect(urls).toEqual(["/sw.js"]);
  });

  it("does nothing without service workers and survives a failed registration", async () => {
    await expect(registerServiceWorker({}, factory(), ORIGIN)).resolves.toBeUndefined();
    await expect(
      registerServiceWorker(
        { serviceWorker: { register: () => Promise.reject(new Error("blocked")) } },
        factory(),
        ORIGIN,
      ),
    ).resolves.toBeUndefined();
  });
});
