import type { ReadinessIssue, SilenceStatus } from "../contract";
import type { MessageKey } from "../i18n/keys";

/*
 * The words of the monitor notices, as catalogue keys (`redesign-calm-ui` copy table). Kept apart
 * from `banners.ts`, which ranks and decides without any wording.
 */

export interface BannerWords {
  title: MessageKey;
  body?: MessageKey;
}

/** The readiness item in words; the id is the worst failing issue. */
export const READINESS_WORDS: Readonly<Record<ReadinessIssue, BannerWords>> = {
  notifications_blocked: {
    title: "native.ui.banner.ready.notifications.title",
    body: "native.ui.banner.ready.notifications.body",
  },
  full_screen_blocked: {
    title: "native.ui.banner.ready.fullScreen.title",
    body: "native.ui.banner.ready.fullScreen.body",
  },
  alarm_volume_zero: {
    title: "native.ui.banner.ready.alarmVolumeZero.title",
    body: "native.ui.banner.ready.alarmVolumeZero.body",
  },
  alarm_volume_low: {
    title: "native.ui.banner.ready.alarmVolumeLow.title",
    body: "native.ui.banner.ready.alarmVolumeLow.body",
  },
  dnd_bypass_missing: {
    title: "native.ui.banner.ready.dnd.title",
    body: "native.ui.banner.ready.dnd.body",
  },
  battery_optimized: {
    title: "native.ui.banner.ready.battery.title",
    body: "native.ui.banner.ready.battery.body",
  },
  parent_battery_low: {
    title: "native.ui.banner.ready.parentBattery.title",
    body: "native.ui.banner.ready.parentBattery.body",
  },
  headphones_connected: {
    title: "native.ui.banner.ready.headphones.title",
    body: "native.ui.banner.ready.headphones.body",
  },
  never_tested: {
    title: "native.ui.banner.ready.neverTested.title",
    body: "native.ui.banner.ready.neverTested.body",
  },
};

/** The silence banner's title; a status without a warning has none. */
export const SILENCE_TITLE: Readonly<Partial<Record<SilenceStatus, MessageKey>>> = {
  screen_may_light: "native.ui.silence.screenMayLight",
  may_buzz: "native.ui.silence.mayVibrate",
  may_ring: "native.ui.silence.mayRing",
  no_access: "native.ui.silence.mayRing",
};

/** The baby device's own warnings, by wire name. */
export const UNIT_WARNING_KEYS: Readonly<Record<string, MessageKey>> = {
  page_hidden: "native.ui.parent.unitWarning.pageHidden",
  wake_lock_off: "native.ui.parent.unitWarning.wakeLockOff",
  resumed_from_sleep: "native.ui.parent.unitWarning.resumedFromSleep",
  not_charging: "native.ui.parent.unitWarning.notCharging",
  mic_permission_lost: "native.ui.parent.unitWarning.micPermissionLost",
};
