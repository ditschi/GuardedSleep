import { Capacitor } from "@capacitor/core";
import type { ConnectionStatus } from "@capacitor/network";
import { Network } from "@capacitor/network";

const HANDOFF_DEBOUNCE_MS = 3000;

/**
 * After a stable network fingerprint change (debounced), invokes `onHandoff`.
 * Native only (`@capacitor/network`). Web / PWA: no-op.
 */
export function subscribeNetworkHandoff(onHandoff: () => void): () => void {
  if (!Capacitor.isNativePlatform()) {
    return () => {};
  }

  const st = {
    disposed: false,
    debounceTimer: undefined as ReturnType<typeof setTimeout> | undefined,
    lastKey: "",
    listenerHandle: null as null | { remove: () => Promise<void> },
  };

  void Network.getStatus()
    .then((s: ConnectionStatus) => {
      if (st.disposed) return;
      st.lastKey = `${s.connected}-${s.connectionType}`;
    })
    .catch(() => {
      /* ignore — first comparison still works once events arrive */
    });

  void Network.addListener("networkStatusChange", (s: ConnectionStatus) => {
    if (st.disposed) return;
    const key = `${s.connected}-${s.connectionType}`;
    if (key === st.lastKey) return;
    st.lastKey = key;
    if (st.debounceTimer !== undefined) clearTimeout(st.debounceTimer);
    st.debounceTimer = setTimeout(() => {
      st.debounceTimer = undefined;
      if (!st.disposed) onHandoff();
    }, HANDOFF_DEBOUNCE_MS);
  }).then((h) => {
    if (st.disposed) {
      void h.remove();
      return;
    }
    st.listenerHandle = h;
  });

  return () => {
    st.disposed = true;
    if (st.debounceTimer !== undefined) clearTimeout(st.debounceTimer);
    void st.listenerHandle?.remove();
  };
}
