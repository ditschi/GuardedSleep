import { readable, type Readable } from "svelte/store";
import type { MessageKey } from "./i18n/keys";

/**
 * History API router: clean paths on `pushState`/`popstate`. Both hosts serve `index.html` for
 * unknown paths (nginx `try_files`, the server's `--serve-web` fallback, the PWA navigateFallback),
 * so a reload lands on the same path. Pairing material lives in memory only, so a reload of a
 * paired screen shows the start screen: the path is only a hint for where the user was.
 */

export type RouteId = "role" | "babyPair" | "baby" | "parentPair" | "parent" | "settings";

export interface Route {
  readonly id: RouteId;
  readonly path: string;
  /** Catalogue key of the screen name; used for `document.title` and the heading. */
  readonly titleKey: MessageKey;
}

export const ROUTES: readonly Route[] = [
  { id: "role", path: "/", titleKey: "webShell.route.role" },
  { id: "babyPair", path: "/baby/pair", titleKey: "webShell.route.babyPair" },
  { id: "baby", path: "/baby", titleKey: "webShell.route.baby" },
  { id: "parentPair", path: "/parent/pair", titleKey: "webShell.route.parentPair" },
  { id: "parent", path: "/parent", titleKey: "webShell.route.parent" },
  { id: "settings", path: "/settings", titleKey: "webShell.route.settings" },
];

const FALLBACK: Route = { id: "role", path: "/", titleKey: "webShell.route.role" };

/** `/baby/` and `/baby` are the same screen; the root stays `/`. */
function normalizePath(pathname: string): string {
  const path = pathname.length > 1 ? pathname.replace(/\/+$/, "") : pathname;
  return path === "" ? "/" : path;
}

/** The route for a pathname; anything unknown falls back to the role screen (`/`). */
export function matchRoute(pathname: string): Route {
  const path = normalizePath(pathname);
  return ROUTES.find((r) => r.path === path) ?? FALLBACK;
}

export function routeById(id: RouteId): Route {
  return ROUTES.find((r) => r.id === id) ?? FALLBACK;
}

/** The slice of `Window` the router needs; tests pass a jsdom window. */
export type RouterWindow = Pick<Window, "addEventListener" | "removeEventListener" | "location"> & {
  history: Pick<History, "pushState" | "replaceState">;
};

export interface Router {
  readonly route: Readable<Route>;
  /** Goes to a path (unknown paths become `/`); the same path again does nothing. */
  navigate(path: string, options?: { replace?: boolean }): void;
  destroy(): void;
}

export function createRouter(win: RouterWindow): Router {
  let current = matchRoute(win.location.pathname);
  // A URL that is not a route is rewritten to `/` so the address bar tells the truth.
  if (normalizePath(win.location.pathname) !== current.path) {
    win.history.replaceState(null, "", current.path);
  }

  let publish: (route: Route) => void = () => {};
  const route = readable(current, (set) => {
    publish = set;
    set(current);
    return () => {
      publish = () => {};
    };
  });

  const onPopState = (): void => {
    current = matchRoute(win.location.pathname);
    publish(current);
  };
  win.addEventListener("popstate", onPopState);

  return {
    route,
    navigate(path, options) {
      const next = matchRoute(path);
      if (next.id === current.id && normalizePath(win.location.pathname) === next.path) return;
      if (options?.replace) win.history.replaceState(null, "", next.path);
      else win.history.pushState(null, "", next.path);
      current = next;
      publish(current);
    },
    destroy() {
      win.removeEventListener("popstate", onPopState);
    },
  };
}

/**
 * Svelte action for `<a href>`: plain left clicks navigate in page, everything else (new tab,
 * modifier keys, external links) keeps the browser behaviour.
 */
export function link(node: HTMLAnchorElement, router: Router): { destroy(): void } {
  const onClick = (event: MouseEvent): void => {
    if (event.defaultPrevented || event.button !== 0) return;
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    const url = new URL(node.href, node.ownerDocument.baseURI);
    if (url.origin !== node.ownerDocument.location.origin) return;
    event.preventDefault();
    router.navigate(url.pathname);
  };
  node.addEventListener("click", onClick);
  return {
    destroy() {
      node.removeEventListener("click", onClick);
    },
  };
}

/** Moves focus to the screen heading after a route change (screen-reader and keyboard users). */
export function focusRouteHeading(root: ParentNode): boolean {
  const heading = root.querySelector<HTMLElement>("[data-route-heading]");
  if (!heading) return false;
  heading.focus({ preventScroll: false });
  return true;
}

/** `document.title` for a screen: "<screen> · <app>". */
export function formatDocumentTitle(screenTitle: string, appTitle: string): string {
  return `${screenTitle} · ${appTitle}`;
}
