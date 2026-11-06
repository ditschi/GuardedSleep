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

/** Opens a connection to exactly `url`: the core names the address of every mailbox. */
export type SignalingFactory = (url: string, listener: SignalingListener) => SignalingSocket;

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
 * One WebSocket to the address the core named in `open_mailbox`. When it cannot be opened the
 * listener gets `onClose` and the core schedules the next `open_mailbox` itself (backoff, failover
 * to another relay mailbox); the transport never retries or tries another address.
 */
export function createWebSocketSignaling(ctor: WebSocketCtor): SignalingFactory {
  return (url, listener) => {
    let socket: WebSocketLike | null = null;
    let opened = false;
    let done = false;

    const finish = (): void => {
      if (done) return;
      done = true;
      listener.onClose();
    };

    try {
      const ws = new ctor(url);
      socket = ws;
      ws.onopen = () => {
        if (done || socket !== ws) return;
        opened = true;
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
        finish();
      };
    } catch {
      // A malformed or blocked URL (for example ws: from an https page) counts as unreachable.
      finish();
    }

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
