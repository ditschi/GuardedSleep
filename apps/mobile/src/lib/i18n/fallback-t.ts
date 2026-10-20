import { getI18n } from "./context";
import { rawBundles } from "./bundles";
import { formatMessage } from "./icu";
import { getByPath } from "./paths";

/** Uses context `t` when inside the app shell; otherwise English JSON (e.g. unit tests without layout). */
export function tWithEnFallback(key: string, params?: Record<string, string | number>): string {
  const ctx = getI18n();
  if (ctx) return ctx.t(key, params);
  const raw = getByPath(rawBundles.en, key);
  return typeof raw === "string" ? formatMessage(raw, params, "en") : key;
}
