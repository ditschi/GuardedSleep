import { goto } from "$app/navigation";
import { browser } from "$app/environment";

export type BackInterceptor = () => boolean;

const interceptors: BackInterceptor[] = [];

/** Routes that never show the in-app back control (OS back may still exit on home). */
const ROOT_ROUTES = new Set(["/", "/onboard"]);

export function shouldShowAppBack(pathname: string): boolean {
  return !ROOT_ROUTES.has(pathname);
}

export function resolveBackFallback(pathname: string, searchParams: URLSearchParams): string {
  if (pathname === "/settings") {
    const returnTo = searchParams.get("returnTo");
    const room = searchParams.get("room") ?? "";
    const signal = searchParams.get("signal") ?? "";
    if (returnTo === "baby" && room) {
      const q = new URLSearchParams({ room });
      if (signal) q.set("signal", signal);
      return `/baby?${q}`;
    }
    if (returnTo === "parent" && room) {
      const q = new URLSearchParams({ room });
      if (signal) q.set("signal", signal);
      return `/parent?${q}`;
    }
  }
  return "/";
}

export function canNavigateBack(): boolean {
  if (!browser) return false;
  const idx = history.state?.["sveltekit:index"];
  if (typeof idx === "number") return idx > 0;
  return history.length > 1;
}

export function pushBackInterceptor(fn: BackInterceptor): () => void {
  interceptors.push(fn);
  return () => {
    const at = interceptors.indexOf(fn);
    if (at >= 0) interceptors.splice(at, 1);
  };
}

export async function navigateAppBack(fallbackHref = "/"): Promise<void> {
  if (!browser) return;

  for (let i = interceptors.length - 1; i >= 0; i--) {
    if (interceptors[i]()) return;
  }

  if (canNavigateBack()) {
    history.back();
    return;
  }

  if (fallbackHref === "/" && window.location.pathname === "/") return;

  await goto(fallbackHref);
}
