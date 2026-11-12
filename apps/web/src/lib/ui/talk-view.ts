import type { ParentMonitorState, TalkUnavailable, WebTalkState } from "../contract";
import type { MessageKey } from "../i18n/keys";
import { TALK_COUNTDOWN_FROM_MS } from "./talk-constants";

/** What the talk button reads: the talk fields of the parent state and the browser's own. */
export type TalkView = Pick<
  ParentMonitorState,
  | "talk"
  | "talkEnd"
  | "talkPermission"
  | "noisePause"
  | "talkUnconfirmed"
  | "talkAudibleUncertain"
  | "talkMicSilent"
  | "afterTalkListenUntilElapsedMs"
>;

/** One line of copy: a catalogue key and its arguments. */
export interface TalkLine {
  key: MessageKey;
  args?: Record<string, string | number>;
}

const seconds = (ms: number): number => Math.max(1, Math.ceil(ms / 1000));

const UNAVAILABLE_KEYS: Record<Exclude<TalkUnavailable, "cooldown">, MessageKey> = {
  not_connected: "webScreens.talk.unavailableNotConnected",
  off: "webScreens.talk.unavailableOff",
  other_talking: "webScreens.talk.unavailableOther",
  unsupported: "webScreens.talk.unavailableUnsupported",
  muted: "webScreens.talk.unavailableMuted",
};

/** Why the button cannot be pressed, before any press (design section 9). Null when it can. */
export function unavailableLine(view: TalkView, web: WebTalkState, nowMs: number): TalkLine | null {
  const talk = view.talk;
  if (talk.kind !== "unavailable") return null;
  // Only a browser that cannot capture gets its own words; Android has no such state.
  if (web.micUnsupported) return { key: "webScreens.talk.unavailableBrowser" };
  if (talk.reason === "cooldown") {
    const wait = talk.retryAtElapsedMs === null ? 1000 : talk.retryAtElapsedMs - nowMs;
    return { key: "webScreens.talk.unavailableCooldown", args: { seconds: seconds(wait) } };
  }
  return { key: UNAVAILABLE_KEYS[talk.reason] };
}

/** The line for a denied request, null when the last request was not denied. */
function deniedLine(view: TalkView, nowMs: number): TalkLine | null {
  const talk = view.talk;
  if (talk.kind !== "denied") return null;
  switch (talk.reason) {
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
}

function endLine(view: TalkView): TalkLine | null {
  switch (view.talkEnd) {
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
}

/**
 * What the state line says. While the microphone is open this changes only as a visible
 * countdown; the screen keeps it out of any live region (no speech into the open microphone).
 */
export function stateLine(view: TalkView, nowMs: number): TalkLine | null {
  const talk = view.talk;
  switch (talk.kind) {
    case "requesting":
      return { key: "webScreens.talk.requesting" };
    case "talking": {
      const left = talk.untilElapsedMs - nowMs;
      if (left <= TALK_COUNTDOWN_FROM_MS) {
        return { key: "webScreens.talk.releaseIn", args: { seconds: seconds(left) } };
      }
      return { key: "webScreens.talk.speakNow" };
    }
    case "denied":
      return deniedLine(view, nowMs);
    case "idle":
    case "unavailable":
      return endLine(view);
  }
}

/** The noise-pause line for the talking parent and for any other parent. */
export function noiseLine(view: TalkView): TalkLine | null {
  if (view.noisePause === "you") return { key: "webScreens.talk.noisePausedSelf" };
  if (view.noisePause === "other") return { key: "webScreens.talk.noisePausedOther" };
  return null;
}

/** Whether the screen may announce changes: never while the microphone is open. */
export function mayAnnounce(view: TalkView): boolean {
  return view.talk.kind !== "talking";
}
