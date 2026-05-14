import { Capacitor } from "@capacitor/core";

let started = false;

export async function startBabyMonitoringForeground(): Promise<void> {
  if (Capacitor.getPlatform() !== "android" || started) return;
  try {
    const { ForegroundService, ServiceType, Importance } =
      await import("@capawesome-team/capacitor-android-foreground-service");
    await ForegroundService.requestPermissions?.().catch(() => {});
    await ForegroundService.createNotificationChannel?.({
      id: "guardedsleep_monitor",
      name: "Monitoring",
      description: "Baby unit audio monitoring",
      // Default: visible ongoing notification; Low can hide the icon on some OEMs.
      importance: Importance.Default,
    });
    await ForegroundService.startForegroundService({
      id: 42,
      title: "GuardedSleep",
      body: "Monitoring — microphone active",
      smallIcon: "ic_launcher_foreground",
      notificationChannelId: "guardedsleep_monitor",
      serviceType: ServiceType.Microphone,
    });
    started = true;
  } catch {
    /* optional: plugin missing or not on native */
  }
}

export async function stopBabyMonitoringForeground(): Promise<void> {
  if (Capacitor.getPlatform() !== "android" || !started) return;
  try {
    const { ForegroundService } =
      await import("@capawesome-team/capacitor-android-foreground-service");
    await ForegroundService.stopForegroundService();
  } catch {
    void 0;
  }
  started = false;
}
