/** True in Vite dev server or when explicitly enabled at build time (local debug APK / Docker stack). */
export function devConnectHintsEnabled(): boolean {
  return import.meta.env.DEV || import.meta.env.VITE_DEV_CONNECT_HINTS === "true";
}
