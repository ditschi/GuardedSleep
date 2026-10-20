import { GuardedSleepNative } from "$lib/android/guarded-sleep-native.js";
import type { SignalingProbeSocket } from "$lib/signaling/path-selector.js";
import { shouldUseAndroidNativeWss } from "./android-dev-wss-enabled.js";

const READY_STATE_CONNECTING = 0;
const READY_STATE_OPEN = 1;
const READY_STATE_CLOSED = 3;

type NativeSignalingBridgeHandlers = {
  onMessage: (text: string) => void;
  onClose: () => void;
};

let bridgeHandlers: NativeSignalingBridgeHandlers | null = null;
let listenersAttached = false;

async function ensureNativeSignalingListeners(): Promise<void> {
  if (listenersAttached) return;
  listenersAttached = true;
  await GuardedSleepNative.addListener("nativeSignalingMessage", (ev) => {
    bridgeHandlers?.onMessage(ev.text);
  });
  await GuardedSleepNative.addListener("nativeSignalingClose", () => {
    bridgeHandlers?.onClose();
  });
}

export async function nativeSignalingConnect(
  url: string,
  handlers: NativeSignalingBridgeHandlers,
): Promise<void> {
  await ensureNativeSignalingListeners();
  bridgeHandlers = handlers;
  await GuardedSleepNative.nativeSignalingConnect({ url });
}

export async function nativeSignalingSend(text: string): Promise<void> {
  await GuardedSleepNative.nativeSignalingSend({ text });
}

export async function nativeSignalingClose(): Promise<void> {
  bridgeHandlers = null;
  await GuardedSleepNative.nativeSignalingClose();
}

/** Path-selector probe socket backed by the native OkHttp client. */
export class AndroidNativeSignalingProbeSocket implements SignalingProbeSocket {
  readyState = READY_STATE_CONNECTING;
  private readonly openHandlers: Array<() => void> = [];
  private readonly errorHandlers: Array<() => void> = [];
  private readonly closeHandlers: Array<() => void> = [];

  constructor(url: string) {
    void (async () => {
      try {
        await GuardedSleepNative.nativeSignalingConnect({ url });
        this.readyState = READY_STATE_OPEN;
        for (const h of this.openHandlers) h();
        await GuardedSleepNative.nativeSignalingClose();
        this.readyState = READY_STATE_CLOSED;
        for (const h of this.closeHandlers) h();
      } catch {
        this.readyState = READY_STATE_CLOSED;
        for (const h of this.errorHandlers) h();
        for (const h of this.closeHandlers) h();
      }
    })();
  }

  addEventListener(event: "open" | "close" | "error", cb: () => void): void {
    if (event === "open") this.openHandlers.push(cb);
    else if (event === "error") this.errorHandlers.push(cb);
    else this.closeHandlers.push(cb);
  }

  close(): void {
    void GuardedSleepNative.nativeSignalingClose();
    this.readyState = READY_STATE_CLOSED;
  }
}

export function createSignalingProbeSocket(url: string): SignalingProbeSocket {
  if (shouldUseAndroidNativeWss(url)) {
    return new AndroidNativeSignalingProbeSocket(url);
  }
  return new WebSocket(url) as unknown as SignalingProbeSocket;
}
