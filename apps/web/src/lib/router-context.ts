import { getContext, setContext } from "svelte";
import type { Router } from "./router";

const KEY = Symbol("guardedsleep.router");

/** App root: lets the screens navigate without owning the router. */
export function provideRouter(router: Router): void {
  setContext(KEY, router);
}

export function useRouter(): Router {
  const router = getContext<Router | undefined>(KEY);
  if (!router) throw new Error("no Router provided: call provideRouter at the root");
  return router;
}
