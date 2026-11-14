import type { MessageKey } from "../i18n/keys";
import type { Translate } from "../i18n/translate";

/** "4:32": the countdown of a pairing code. Never negative. */
export function formatCountdown(remainingMs: number): string {
  const total = Math.max(0, Math.ceil(remainingMs / 1000));
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

/**
 * "1:12": how long something has been going on, whole seconds truncated like the Android counter
 * (a count-up never runs ahead of the clock; `formatCountdown` rounds up for a count-down).
 */
export function formatElapsed(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(total / 60)}:${(total % 60).toString().padStart(2, "0")}`;
}

/** Milliseconds left until a monotonic deadline; null when there is none. */
export function remainingMs(deadlineElapsedMs: number | null, nowElapsedMs: number): number | null {
  return deadlineElapsedMs === null ? null : Math.max(0, deadlineElapsedMs - nowElapsedMs);
}

/** A clock time for the user ("22:41"), in the language of the page. */
export function formatClock(wallMs: number, locale: string): string {
  return new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }).format(wallMs);
}

/** "45 s", "2 min", "1 h 5 min": the catalogue's duration messages, never below one second. */
export function formatDuration(t: Translate, ms: number): string {
  const seconds = Math.max(1, Math.round(ms / 1000));
  if (seconds < 60) return t("native.ui.common.durationSec", { seconds });
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return t("native.ui.common.durationMin", { minutes });
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest === 0
    ? t("native.ui.common.durationHour", { hours })
    : t("native.ui.common.durationHourMin", { hours, minutes: rest });
}

/** The loudness word for the meter's accessible name. */
export function levelWordKey(level: number): MessageKey {
  if (level >= 0.5) return "native.ui.parent.level.loud";
  if (level >= 0.12) return "native.ui.parent.level.some";
  return "native.ui.parent.level.quiet";
}

/**
 * `{duration}` in running text: "48 s", "1 min 48 s", "2 min" (durations in words, never `m:ss`;
 * `redesign-calm-ui` design section 8).
 */
export function formatDurationWords(t: Translate, ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  if (total < 60) return t("native.ui.common.durationSec", { seconds: total });
  const min = t("native.ui.common.durationMin", { minutes: Math.floor(total / 60) });
  return total % 60 === 0
    ? min
    : `${min} ${t("native.ui.common.durationSec", { seconds: total % 60 })}`;
}

/** "1 minute 12 seconds" for the screen reader; the visible counter stays digits. */
export function spokenDuration(t: Translate, ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  const sec = t("native.ui.connAlert.spokenSec", { count: seconds });
  if (minutes === 0) return sec;
  const min = t("native.ui.connAlert.spokenMin", { count: minutes });
  return seconds === 0 ? min : `${min} ${sec}`;
}
