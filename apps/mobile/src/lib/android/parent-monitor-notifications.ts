import { Capacitor } from "@capacitor/core";
import { LocalNotifications } from "@capacitor/local-notifications";
import { GuardedSleepNative } from "$lib/android/guarded-sleep-native.js";

const CH_NOISE = "guardedsleep_noise_alert";
const CH_LINK = "guardedsleep_link_watch";

let channelsCreated = false;

function nextNoiseNotificationId(): number {
  return 21_000 + Math.floor(Math.random() * 99_000);
}

export async function ensureParentMonitorNotificationsReady(): Promise<void> {
  if (!Capacitor.isNativePlatform()) return;
  const perm = await LocalNotifications.checkPermissions();
  if (perm.display !== "granted") {
    await LocalNotifications.requestPermissions();
  }
  if (Capacitor.getPlatform() !== "android" || channelsCreated) return;
  await LocalNotifications.createChannel({
    id: CH_NOISE,
    name: "Baby sounds",
    description: "When the baby unit detects sustained noise",
    importance: 5,
    vibration: true,
  });
  await LocalNotifications.createChannel({
    id: CH_LINK,
    name: "Connection to baby unit",
    description: "When the link may be interrupted",
    importance: 4,
    vibration: true,
  });
  channelsCreated = true;
}

/** Heads-up when parent is not looking at the app; baby audio in-app is the primary alert. */
export async function notifyNoiseAlertIfBackground(): Promise<void> {
  if (!Capacitor.isNativePlatform()) return;
  if (typeof document === "undefined" || document.visibilityState === "visible") return;
  const perm = await LocalNotifications.checkPermissions();
  if (perm.display !== "granted") return;
  if (Capacitor.getPlatform() === "android") {
    await GuardedSleepNative.showAlarmStyleNotification({
      title: "Noise on baby unit",
      body: "Open the app — you should hear the room",
      notificationId: nextNoiseNotificationId(),
    });
    return;
  }
  await LocalNotifications.schedule({
    notifications: [
      {
        title: "Noise on baby unit",
        body: "Open the app — you should hear the room",
        id: nextNoiseNotificationId(),
        channelId: CH_NOISE,
        schedule: { at: new Date(Date.now() + 80) },
      },
    ],
  });
}

const LINK_NOTIFY_ID = 19_001;

export async function notifyBabyLinkUncertain(): Promise<void> {
  if (!Capacitor.isNativePlatform()) return;
  const perm = await LocalNotifications.checkPermissions();
  if (perm.display !== "granted") return;
  if (Capacitor.getPlatform() === "android") {
    await GuardedSleepNative.showAlarmStyleNotification({
      title: "Baby link weak or lost",
      body: "Check Wi‑Fi and that the baby device is still monitoring",
      notificationId: LINK_NOTIFY_ID,
    });
    return;
  }
  await LocalNotifications.schedule({
    notifications: [
      {
        title: "Baby link weak or lost",
        body: "Check Wi‑Fi and that the baby device is still monitoring",
        id: LINK_NOTIFY_ID,
        channelId: CH_LINK,
        schedule: { at: new Date(Date.now() + 80) },
      },
    ],
  });
}
