import type en from "./generated/en.json";

type Paths<T> = {
  [K in keyof T & string]: T[K] extends string ? K : `${K}.${Paths<T[K]>}`;
}[keyof T & string];

/**
 * Every dot-path of the English master catalogue (`locales/en.json`, generated into
 * `./generated`), so a misspelt key is a compile error.
 */
export type MessageKey = Paths<typeof en>;
