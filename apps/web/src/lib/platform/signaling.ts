/** One WebSocket to the relay. Text frames only; the core decides what the messages mean. */
export interface SignalingSocket {
  send(text: string): void;
  close(): void;
}

export interface SignalingListener {
  onOpen(): void;
  onMessage(text: string): void;
  onClose(): void;
}

/** Opens a connection to the first reachable URL of the list. */
export type SignalingFactory = (
  urls: readonly string[],
  listener: SignalingListener,
) => SignalingSocket;

/** The subset of `WebSocket` the transport uses (a fake in tests). */
export interface WebSocketLike {
  readonly readyState: number;
  onopen: ((ev: unknown) => void) | null;
  onmessage: ((ev: { data: unknown }) => void) | null;
  onerror: ((ev: unknown) => void) | null;
  onclose: ((ev: unknown) => void) | null;
  send(data: string): void;
  close(): void;
}

export type WebSocketCtor = new (url: string) => WebSocketLike;

const OPEN = 1;

/**
 * Tries the URLs in order until one opens, starting at the URL that worked last (so a dead first
 * entry does not delay every reconnect the core asks for). When none opens the listener gets
 * `onClose` and the core schedules the next `open_signaling` itself; the transport never retries.
 */
export function createWebSocketSignaling(ctor: WebSocketCtor): SignalingFactory {
  let start = 0;
  return (urls, listener) => {
    const list = urls.filter((u) => u.trim() !== "");
    let index = 0;
    let socket: WebSocketLike | null = null;
    let opened = false;
    let done = false;

    const finish = (): void => {
      if (done) return;
      done = true;
      listener.onClose();
    };

    const tryNext = (): void => {
      if (done) return;
      if (index >= list.length) return finish();
      const slot = (start + index) % list.length;
      const url = list[slot] ?? "";
      index++;
      let ws: WebSocketLike;
      try {
        ws = new ctor(url);
      } catch {
        // A malformed or blocked URL (for example ws: from an https page) counts as unreachable.
        return tryNext();
      }
      socket = ws;
      ws.onopen = () => {
        if (done || socket !== ws) return;
        opened = true;
        start = slot;
        listener.onOpen();
      };
      ws.onmessage = (ev) => {
        if (!done && socket === ws && opened && typeof ev.data === "string") {
          listener.onMessage(ev.data);
        }
      };
      ws.onerror = () => {
        // The close event follows; the failure is handled there.
      };
      ws.onclose = () => {
        if (done || socket !== ws) return;
        if (opened) finish();
        else tryNext();
      };
    };

    tryNext();

    return {
      send(text) {
        if (!done && socket?.readyState === OPEN) socket.send(text);
      },
      close() {
        if (done) return;
        done = true;
        const ws = socket;
        socket = null;
        if (ws) {
          ws.onopen = ws.onmessage = ws.onerror = ws.onclose = null;
          ws.close();
        }
      },
    };
  };
}
