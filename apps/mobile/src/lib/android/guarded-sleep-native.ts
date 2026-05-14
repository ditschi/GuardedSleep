import { Capacitor, registerPlugin } from "@capacitor/core";

export type LanPeerResolved = {
  room: string;
  host: string;
  port: number;
  token: string;
};

export type GuardedSleepNativePlugin = {
  /** Move app task to front (e.g. parent alert while screen was off). */
  bringToForeground(): Promise<void>;
  /**
   * Android: CATEGORY_ALARM + full-screen intent (OS-dependent). Use for parent alerts.
   */
  showAlarmStyleNotification(options: {
    title: string;
    body: string;
    notificationId: number;
  }): Promise<void>;
  openBatterySettings(): Promise<void>;
  setKeepAwake(options: { enabled: boolean }): Promise<void>;
  setImmersiveMode(options: { enabled: boolean }): Promise<void>;
  /** Embedded LAN signaling (baby unit). Resolves with bound TCP port. */
  startLanSignalingServer(options: { roomId: string }): Promise<{ port: number }>;
  stopLanSignalingServer(): Promise<void>;
  /**
   * NSD advertise for `_guardedsleep._tcp`. Prefer `port` + `discoveryToken` (opaque `gst-*` name).
   * Legacy: `roomId` only generates a token and uses placeholder port 9.
   */
  startNsdAdvertise(options: {
    port?: number;
    discoveryToken?: string;
    roomId?: string;
  }): Promise<void>;
  stopNsdAdvertise(): Promise<void>;
  startNsdDiscovery(): Promise<void>;
  stopNsdDiscovery(): Promise<void>;
  addListener(
    eventName: "lanPeerResolved",
    listenerFunc: (event: LanPeerResolved) => void,
  ): Promise<{ remove: () => Promise<void> }>;
};

export const GuardedSleepNative = registerPlugin<GuardedSleepNativePlugin>("GuardedSleepNative", {
  web: () => ({
    async bringToForeground() {
      void 0;
    },
    async showAlarmStyleNotification() {
      void 0;
    },
    async openBatterySettings() {
      void 0;
    },
    async setKeepAwake() {
      void 0;
    },
    async setImmersiveMode() {
      void 0;
    },
    async startLanSignalingServer() {
      return { port: 0 };
    },
    async stopLanSignalingServer() {
      void 0;
    },
    async startNsdAdvertise() {
      void 0;
    },
    async stopNsdAdvertise() {
      void 0;
    },
    async startNsdDiscovery() {
      void 0;
    },
    async stopNsdDiscovery() {
      void 0;
    },
    async addListener() {
      return { remove: async () => {} };
    },
  }),
});

export function isNativeAndroid(): boolean {
  return Capacitor.getPlatform() === "android";
}
