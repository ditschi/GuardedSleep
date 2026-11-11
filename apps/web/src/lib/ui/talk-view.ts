import type { ParentTalkUi, TalkUnavailable } from "../contract";
import type { MessageKey } from "../i18n/keys";
import { TALK_COUNTDOWN_FROM_MS } from "./talk-constants";

/** One line of copy: a catalogue key and its arguments. */
export interface TalkLine {
  key: MessageKey;
  args?: Record<string, string | number>;
}

const seconds = (ms: number): number => Math.max(1, Math.ceil(ms / 1000));

const UNAVAILABLE_KEYS: Record<Exclude<TalkUnavailable, "cooldown">, MessageKey> = {
  not_connected: "webScreens.talk.unavailableNotConnected",
  browser_unsupported: "webScreens.talk.unavailableBrowser",
  off: "webScreens.talk.unavailableOff",
  other_talker: "webScreens.talk.unavailableOther",
  unsupported: "webScreens.talk.unavailableUnsupported",
  muted: "webScreens.talk.unavailableMuted",
};

/** Why the button cannot be pressed, before any press (design section 9). Null when it can. */
export function unavailableLine(talk: ParentTalkUi, nowMs: number): TalkLine | null {
  const reason = talk.unavailable;
  if (reason === null) return null;
  if (reason === "cooldown") {
    const wait = talk.retryAtElapsedMs === null ? 1000 : talk.retryAtElapsedMs - nowMs;
    return { key: "webScreens.talk.unavailableCooldown", args: { seconds: seconds(wait) } };
  }
  return { key: UNAVAILABLE_KEYS[reason] };
}

/**
 * What the state line says. While the microphone is open this changes only as a visible
 * countdown; the screen keeps it out of any live region (no speech into the open microphone).
 */
export function stateLine(talk: ParentTalkUi, nowMs: number): TalkLine | null {
  switch (talk.phase) {
    case "requesting":
      return { key: "webScreens.talk.requesting" };
    case "talking": {
      const left = talk.untilElapsedMs === null ? null : talk.untilElapsedMs - nowMs;
      if (left !== null && left <= TALK_COUNTDOWN_FROM_MS) {
        return { key: "webScreens.talk.releaseIn", args: { seconds: seconds(left) } };
      }
      return { key: "webScreens.talk.speakNow" };
    }
    case "ended":
      switch (talk.endedReason) {
        case "limit":
          return { key: "webScreens.talk.endLimit" };
        case "no_answer":
          return { key: "webScreens.talk.endNoAnswer" };
        case "link_lost":
          return { key: "webScreens.talk.endLinkLost" };
        case "mic_failed":
          return { key: "webScreens.talk.endMicFailed" };
        case "alert":
          return { key: "webScreens.talk.endAlert" };
        case "switched_off":
          return { key: "webScreens.talk.unavailableOff" };
        default:
          return null;
      }
    case "denied":
      switch (talk.deniedReason) {
        case "off":
          return { key: "webScreens.talk.unavailableOff" };
        case "unsupported":
          return { key: "webScreens.talk.unavailableUnsupported" };
        case "muted":
          return { key: "webScreens.talk.unavailableMuted" };
        case "busy":
          return { key: "webScreens.talk.unavailableOther" };
        case "cooldown":
        case "budget": {
          const wait = talk.retryAtElapsedMs === null ? 1000 : talk.retryAtElapsedMs - nowMs;
          return { key: "webScreens.talk.unavailableCooldown", args: { seconds: seconds(wait) } };
        }
        default:
          return { key: "webScreens.talk.deniedOther" };
      }
    case "idle":
      return null;
  }
}

/** The noise-pause line for the talking parent and for any other parent. */
export function noiseLine(talk: ParentTalkUi): TalkLine | null {
  if (talk.noisePause === "self") return { key: "webScreens.talk.noisePausedSelf" };
  if (talk.noisePause === "other") return { key: "webScreens.talk.noisePausedOther" };
  return null;
}

/** Whether the screen may announce changes: never while the microphone is open. */
export function mayAnnounce(talk: ParentTalkUi): boolean {
  return talk.phase !== "talking";
}
