import { getI18n } from "./context";
import en from "./messages/en.json";
import { getByPath, interpolate } from "./paths";

/** Uses context `t` when inside the app shell; otherwise English JSON (e.g. unit tests without layout). */
export function tWithEnFallback(key: string, params?: Record<string, string | number>): string {
  const ctx = getI18n();
  if (ctx) return ctx.t(key, params);
  const raw = getByPath(en, key);
  return typeof raw === "string" ? interpolate(raw, params) : key;
}
