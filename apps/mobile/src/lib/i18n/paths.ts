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
