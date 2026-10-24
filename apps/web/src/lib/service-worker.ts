/*
 * Service worker registration under Trusted Types. The page's CSP has `require-trusted-types-for
 * 'script'`, so the worker URL must be a TrustedScriptURL, and the only policy that can make one
 * (named "guardedsleep" in csp/csp.ts) accepts nothing but this origin's `/sw.js`. Everything else
 * that would reach a script sink is blocked by the browser.
 */

export const POLICY_NAME = "guardedsleep";
export const WORKER_PATH = "/sw.js";

/** The slice of `window.trustedTypes` used here. */
export interface TrustedTypesFactory {
  createPolicy(
    name: string,
    rules: { createScriptURL(input: string): string },
  ): { createScriptURL(input: string): unknown };
}

/** The one policy of the app: a script URL is trusted only when it is this origin's worker. */
export function createAppPolicy(factory: TrustedTypesFactory, origin: string) {
  return factory.createPolicy(POLICY_NAME, {
    createScriptURL(input) {
      if (new URL(input, origin).href !== `${origin}${WORKER_PATH}`) {
        throw new TypeError(`script URL not allowed: ${input}`);
      }
      return input;
    },
  });
}

export interface WorkerEnv {
  serviceWorker?: { register(url: string, options?: { scope?: string }): Promise<unknown> };
}

/**
 * Registers the app's worker (offline shell, installability). A browser without service workers,
 * or a registration that fails, leaves the page working without them.
 */
export async function registerServiceWorker(
  nav: WorkerEnv,
  factory: TrustedTypesFactory | undefined,
  origin: string,
): Promise<void> {
  if (!nav.serviceWorker) return;
  try {
    const url = factory
      ? createAppPolicy(factory, origin).createScriptURL(WORKER_PATH)
      : WORKER_PATH;
    // A TrustedScriptURL stringifies to the URL and is what `register` expects under Trusted Types.
    await nav.serviceWorker.register(url as string, { scope: "/" });
  } catch {
    // No worker (private window, blocked, offline first load): the page still works online.
  }
}
