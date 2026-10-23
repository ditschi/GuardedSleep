/*
 * Ways a page can catch the parent's attention when it is not in front: the tab title flashes,
 * and a system notification appears when the user allowed it. Neither replaces the alert sound
 * (the platform layer plays it); both end when the alert screen closes.
 */

const FLASH_MS = 1000;

/** Alternates `document.title` between its current text and `alertText`; returns the stop. */
export function flashTitle(doc: Document, alertText: string, intervalMs = FLASH_MS): () => void {
  const original = doc.title;
  let showAlert = false;
  const id = setInterval(() => {
    showAlert = !showAlert;
    doc.title = showAlert ? alertText : original;
  }, intervalMs);
  // Show the alert at once: the first second matters most.
  doc.title = alertText;
  showAlert = true;
  return () => {
    clearInterval(id);
    doc.title = original;
  };
}

/** The slice of the Notification API the page uses (a fake in tests). */
export interface NotificationApi {
  readonly permission: NotificationPermission;
  new (title: string, options?: NotificationOptions): { close(): void };
  requestPermission(): Promise<NotificationPermission>;
}

/** The page is not in front: another tab, another window or a minimised browser. */
export function pageIsAway(doc: Document): boolean {
  return doc.visibilityState !== "visible" || !doc.hasFocus();
}

/**
 * Shows a system notification when the user allowed them and the page is away. Returns the
 * function that closes it again. The tag makes a repeated alert replace the earlier one.
 */
export function notifyAway(
  api: NotificationApi | undefined,
  doc: Document,
  message: { title: string; body: string; tag: string },
): () => void {
  if (!api || api.permission !== "granted" || !pageIsAway(doc)) return () => undefined;
  try {
    const shown = new api(message.title, {
      body: message.body,
      tag: message.tag,
      requireInteraction: true,
    });
    return () => shown.close();
  } catch {
    // Some browsers only allow notifications through a service worker (Android Chrome).
    return () => undefined;
  }
}

/**
 * Asks once, from a user gesture, so a later alert can show a notification. Nothing happens when
 * the user already answered or the browser has no Notification API.
 */
export async function askForNotifications(api: NotificationApi | undefined): Promise<void> {
  if (!api || api.permission !== "default") return;
  try {
    await api.requestPermission();
  } catch {
    // An older callback-style API or a blocked prompt: the page works without notifications.
  }
}

/** The browser's Notification API where it exists. */
export function browserNotifications(win: Window): NotificationApi | undefined {
  return (win as unknown as { Notification?: NotificationApi }).Notification;
}
