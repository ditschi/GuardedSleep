import { getContext, setContext } from "svelte";
import type { I18nContext } from "./types";

const I18N_KEY = Symbol("guardedsleep.i18n");

export function setI18nContext(ctx: I18nContext): void {
  setContext(I18N_KEY, ctx);
}

export function getI18n(): I18nContext | undefined {
  return getContext(I18N_KEY);
}
