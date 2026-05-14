export function getByPath(obj: unknown, path: string): unknown {
  const parts = path.split(".").filter(Boolean);
  let cur: unknown = obj;
  for (const p of parts) {
    if (!isPlainObject(cur)) return undefined;
    cur = cur[p];
  }
  return cur;
}

export function hasLeafPath(obj: unknown, path: string): boolean {
  const parts = path.split(".").filter(Boolean);
  let cur: unknown = obj;
  for (let i = 0; i < parts.length; i++) {
    const p = parts[i];
    if (!isPlainObject(cur) || !(p in cur)) return false;
    cur = (cur as Record<string, unknown>)[p];
  }
  return typeof cur === "string";
}

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

/** Dot paths to string leaves (skips non-plain nested values). */
export function flattenMessagePaths(obj: unknown, prefix = ""): string[] {
  if (!isPlainObject(obj)) return [];
  const keys: string[] = [];
  for (const [k, v] of Object.entries(obj)) {
    const path = prefix ? `${prefix}.${k}` : k;
    if (typeof v === "string") {
      keys.push(path);
    } else if (isPlainObject(v)) {
      keys.push(...flattenMessagePaths(v, path));
    }
  }
  return keys;
}

const PLACEHOLDER = /\{\{(\w+)\}\}/g;

export function interpolate(template: string, params?: Record<string, string | number>): string {
  if (!params) return template;
  return template.replace(PLACEHOLDER, (_, name: string) =>
    params[name] !== undefined ? String(params[name]) : `{{${name}}}`,
  );
}
