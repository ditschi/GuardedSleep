export function isPlainObject(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

/** Deep-merge b over a (b wins). Does not merge arrays — b replaces. Always copies plain objects (no shared refs with inputs). */
export function deepMerge<T extends Record<string, unknown>>(a: T, b: Record<string, unknown>): T {
  const out = { ...a } as Record<string, unknown>;
  for (const [k, bv] of Object.entries(b)) {
    const av = out[k];
    if (isPlainObject(bv)) {
      if (isPlainObject(av)) {
        out[k] = deepMerge(av as Record<string, unknown>, bv);
      } else {
        out[k] = deepMerge({}, bv);
      }
    } else {
      out[k] = bv;
    }
  }
  return out as T;
}
