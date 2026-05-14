import { MASTER_LOCALE } from "./config";
import { flattenMessagePaths, getByPath } from "./paths";

export function collectParityIssues(
  master: Record<string, unknown>,
  localeCode: string,
  translated: Record<string, unknown>,
): string[] {
  const required = flattenMessagePaths(master);
  const problems: string[] = [];
  for (const path of required) {
    const v = getByPath(translated, path);
    if (v === undefined) {
      problems.push(`${localeCode}: missing key "${path}"`);
    } else if (typeof v !== "string" || v.trim() === "") {
      problems.push(`${localeCode}: empty or non-string "${path}"`);
    }
  }
  const extra = flattenMessagePaths(translated).filter((p) => !required.includes(p));
  for (const p of extra) {
    problems.push(`${localeCode}: extra key "${p}" (not in ${MASTER_LOCALE})`);
  }
  return problems;
}
