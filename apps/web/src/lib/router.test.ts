import { get } from "svelte/store";
import { afterEach, describe, expect, it } from "vitest";
import {
  createRouter,
  focusRouteHeading,
  formatDocumentTitle,
  link,
  matchRoute,
  ROUTES,
  type Router,
} from "./router";

describe("matchRoute", () => {
  it("matches every route path", () => {
    expect(ROUTES.map((r) => matchRoute(r.path).id)).toEqual(ROUTES.map((r) => r.id));
  });

  it("ignores a trailing slash", () => {
    expect(matchRoute("/baby/pair/").id).toBe("babyPair");
    expect(matchRoute("/parent/").id).toBe("parent");
  });

  it("falls back to the role screen for unknown paths", () => {
    for (const path of ["", "/nope", "/baby/pair/extra", "/Baby", "/settings.html"]) {
      expect(matchRoute(path).id, path).toBe("role");
    }
  });

  it("gives every route a distinct title key", () => {
    expect(new Set(ROUTES.map((r) => r.titleKey)).size).toBe(ROUTES.length);
  });
});

describe("router on the History API", () => {
  let router: Router | undefined;
  afterEach(() => {
    router?.destroy();
    router = undefined;
    window.history.replaceState(null, "", "/");
  });

  it("starts on the current path", () => {
    window.history.replaceState(null, "", "/parent");
    router = createRouter(window);
    expect(get(router.route).id).toBe("parent");
  });

  it("rewrites an unknown start path to /", () => {
    window.history.replaceState(null, "", "/nope");
    router = createRouter(window);
    expect(get(router.route).id).toBe("role");
    expect(window.location.pathname).toBe("/");
  });

  it("navigates with pushState and notifies subscribers", () => {
    router = createRouter(window);
    const seen: string[] = [];
    const stop = router.route.subscribe((r) => seen.push(r.id));
    const before = window.history.length;
    router.navigate("/baby/pair");
    expect(window.location.pathname).toBe("/baby/pair");
    expect(window.history.length).toBe(before + 1);
    router.navigate("/baby/pair");
    expect(window.history.length).toBe(before + 1);
    stop();
    expect(seen).toEqual(["role", "babyPair"]);
  });

  it("replaces instead of pushing when asked", () => {
    router = createRouter(window);
    const before = window.history.length;
    router.navigate("/settings", { replace: true });
    expect(window.location.pathname).toBe("/settings");
    expect(window.history.length).toBe(before);
  });

  it("sends unknown targets to /", () => {
    router = createRouter(window);
    router.navigate("/parent");
    router.navigate("/elsewhere");
    expect(get(router.route).id).toBe("role");
    expect(window.location.pathname).toBe("/");
  });

  it("follows popstate", () => {
    router = createRouter(window);
    router.navigate("/baby");
    window.history.replaceState(null, "", "/parent/pair");
    window.dispatchEvent(new PopStateEvent("popstate"));
    expect(get(router.route).id).toBe("parentPair");
  });

  it("stops listening after destroy", () => {
    router = createRouter(window);
    router.destroy();
    window.history.replaceState(null, "", "/settings");
    window.dispatchEvent(new PopStateEvent("popstate"));
    expect(get(router.route).id).toBe("role");
  });
});

describe("link action", () => {
  function setup(href: string): { a: HTMLAnchorElement; router: Router } {
    const router = createRouter(window);
    const a = document.createElement("a");
    a.href = href;
    document.body.append(a);
    link(a, router);
    return { a, router };
  }
  afterEach(() => {
    document.body.replaceChildren();
    window.history.replaceState(null, "", "/");
  });

  it("navigates in page on a plain click", () => {
    const { a, router } = setup("/settings");
    const event = new MouseEvent("click", { bubbles: true, cancelable: true, button: 0 });
    a.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    expect(get(router.route).id).toBe("settings");
    router.destroy();
  });

  it("leaves modified clicks and other origins to the browser", () => {
    const { a, router } = setup("/settings");
    // jsdom cannot follow links; record what the action did, then cancel the default navigation.
    const preventedByAction: boolean[] = [];
    const record = (e: Event): void => {
      preventedByAction.push(e.defaultPrevented);
      e.preventDefault();
    };
    document.addEventListener("click", record);
    a.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true, ctrlKey: true }));
    const external = setup("https://example.org/baby");
    external.a.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    document.removeEventListener("click", record);
    expect(preventedByAction).toEqual([false, false]);
    expect(get(router.route).id).toBe("role");
    router.destroy();
    external.router.destroy();
  });
});

describe("route chrome", () => {
  it("focuses the screen heading", () => {
    document.body.innerHTML = '<main><h1 tabindex="-1" data-route-heading>Screen</h1></main>';
    expect(focusRouteHeading(document.body)).toBe(true);
    expect(document.activeElement?.textContent).toBe("Screen");
  });

  it("reports a missing heading", () => {
    document.body.innerHTML = "<main></main>";
    expect(focusRouteHeading(document.body)).toBe(false);
  });

  it("formats the document title", () => {
    expect(formatDocumentTitle("Settings", "GuardedSleep")).toBe("Settings · GuardedSleep");
  });
});
