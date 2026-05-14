import type { SupportedLocale } from "./config";

export type I18nContext = {
  readonly locale: SupportedLocale;
  setLocale: (next: SupportedLocale) => void;
  t: (key: string, params?: Record<string, string | number>) => string;
};
