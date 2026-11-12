import { fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  defaultParentTalkFields,
  defaultWebTalk,
  type TalkDeny,
  type TalkEnd,
  type TalkUi,
  type TalkUnavailable,
  type WebTalkState,
} from "../contract";
import TalkButton from "./TalkButton.svelte";
import { mayAnnounce, noiseLine, stateLine, unavailableLine, type TalkView } from "./talk-view";

const talk = (over: Partial<TalkView> = {}): TalkView => ({
  ...defaultParentTalkFields(),
  ...over,
});
const unavailable = (reason: TalkUnavailable, retryAtElapsedMs: number | null = null): TalkUi => ({
  kind: "unavailable",
  reason,
  retryAtElapsedMs,
});
const denied = (reason: TalkDeny): TalkUi => ({ kind: "denied", reason, retryAtElapsedMs: null });
const web = (over: Partial<WebTalkState> = {}): WebTalkState => ({ ...defaultWebTalk(), ...over });
const ended = (talkEnd: TalkEnd): Partial<TalkView> => ({ talkEnd });

describe("talk view lines", () => {
  it("says why the button is unavailable before a press", () => {
    expect(unavailableLine(talk(), web(), 0)).toBeNull();
    expect(unavailableLine(talk({ talk: unavailable("off") }), web(), 0)?.key).toBe(
      "webScreens.talk.unavailableOff",
    );
    expect(unavailableLine(talk({ talk: unavailable("cooldown", 9_500) }), web(), 1_000)).toEqual({
      key: "webScreens.talk.unavailableCooldown",
      args: { seconds: 9 },
    });
    expect(
      unavailableLine(talk({ talk: unavailable("unsupported") }), web({ micUnsupported: true }), 0)
        ?.key,
    ).toBe("webScreens.talk.unavailableBrowser");
  });

  it("one moment, speak now, then a countdown for the last 5 s", () => {
    expect(stateLine(talk({ talk: { kind: "requesting" } }), 0)?.key).toBe(
      "webScreens.talk.requesting",
    );
    const talking = talk({ talk: { kind: "talking", untilElapsedMs: 30_000 } });
    expect(stateLine(talking, 10_000)?.key).toBe("webScreens.talk.speakNow");
    expect(stateLine(talking, 26_200)).toEqual({
      key: "webScreens.talk.releaseIn",
      args: { seconds: 4 },
    });
  });

  it("names the end reasons and denials, and stays quiet after a plain release", () => {
    expect(stateLine(talk(ended("no_answer")), 0)?.key).toBe("webScreens.talk.endNoAnswer");
    expect(stateLine(talk(ended("link_lost")), 0)?.key).toBe("webScreens.talk.endLinkLost");
    expect(stateLine(talk({ talk: denied("muted") }), 0)?.key).toBe(
      "webScreens.talk.unavailableMuted",
    );
    expect(stateLine(talk(ended("released")), 0)).toBeNull();
  });

  it("says the noise pause for the talking and for the other parent", () => {
    expect(noiseLine(talk({ noisePause: "you" }))?.key).toBe("webScreens.talk.noisePausedSelf");
    expect(noiseLine(talk({ noisePause: "other" }))?.key).toBe("webScreens.talk.noisePausedOther");
    expect(noiseLine(talk())).toBeNull();
  });

  it("announces nothing while the microphone is open", () => {
    expect(mayAnnounce(talk({ talk: { kind: "talking", untilElapsedMs: 1 } }))).toBe(false);
    expect(mayAnnounce(talk({ talk: { kind: "requesting" } }))).toBe(true);
    expect(mayAnnounce(talk(ended("limit")))).toBe(true);
  });
});

describe("TalkButton", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  const press = (button: HTMLElement, pointerId = 1) =>
    fireEvent.pointerDown(button, { pointerId, button: 0, clientY: 100 });

  function mount(over: Partial<TalkView> = {}, webState: WebTalkState = web()) {
    const onpress = vi.fn();
    const onrelease = vi.fn();
    render(TalkButton, {
      talk: talk(over),
      web: webState,
      nowMs: 0,
      onpress,
      onrelease,
      onkeeplisten: vi.fn(),
    });
    return { onpress, onrelease, button: screen.getByTestId("parent_talk") };
  }

  it("starts a talk only after the hold time, and a release ends it", async () => {
    const { onpress, onrelease, button } = mount();
    await press(button);
    vi.advanceTimersByTime(299);
    expect(onpress).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(onpress).toHaveBeenCalledOnce();
    await fireEvent.pointerUp(button, { pointerId: 1 });
    expect(onrelease).toHaveBeenCalledOnce();
  });

  it("a tap shows how it works and sends nothing", async () => {
    const { onpress, onrelease, button } = mount();
    await press(button);
    vi.advanceTimersByTime(100);
    await fireEvent.pointerUp(button, { pointerId: 1 });
    vi.advanceTimersByTime(1_000);
    expect(onpress).not.toHaveBeenCalled();
    expect(onrelease).not.toHaveBeenCalled();
    expect(screen.getByTestId("parent_talk_state")).toHaveTextContent("Hold the button to talk");
  });

  it("a cancelled touch ends the talk", async () => {
    const { onrelease, button } = mount();
    await press(button);
    vi.advanceTimersByTime(400);
    await fireEvent.pointerCancel(button, { pointerId: 1 });
    expect(onrelease).toHaveBeenCalledOnce();
  });

  it("a hidden page or a lost focus ends the talk", async () => {
    const { onrelease, button } = mount();
    await press(button);
    vi.advanceTimersByTime(400);
    await fireEvent.blur(window);
    expect(onrelease).toHaveBeenCalledOnce();
  });

  it("sliding up says it cannot be locked, and the release still ends the talk", async () => {
    const { onrelease, button } = mount();
    await press(button);
    vi.advanceTimersByTime(400);
    await fireEvent.pointerMove(button, { pointerId: 1, clientY: 20 });
    expect(screen.getByTestId("parent_talk_state")).toHaveTextContent("Talk-back can't be locked");
    await fireEvent.pointerUp(button, { pointerId: 1 });
    expect(onrelease).toHaveBeenCalledOnce();
  });

  it("is disabled with a reason line when unavailable, and a press does nothing", async () => {
    const { onpress, button } = mount({ talk: unavailable("not_connected") });
    expect(button).toBeDisabled();
    expect(screen.getByTestId("parent_talk_unavailable_reason")).toHaveTextContent(
      "Not connected to the baby device.",
    );
    await press(button);
    vi.advanceTimersByTime(500);
    expect(onpress).not.toHaveBeenCalled();
  });

  it("keeps the state line out of any live region while the microphone is open", () => {
    mount({ talk: { kind: "talking", untilElapsedMs: 30_000 } });
    expect(screen.getByTestId("parent_talk_state")).toHaveAttribute("aria-live", "off");
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("announces the result after the release, with the unconfirmed line", () => {
    mount({ talkEnd: "limit", talkUnconfirmed: true });
    expect(screen.getByTestId("parent_talk_state")).toHaveAttribute("aria-live", "polite");
    expect(screen.getByTestId("parent_talk_unconfirmed")).toBeInTheDocument();
  });

  it("holds the unconfirmed line back while talking", () => {
    mount({ talk: { kind: "talking", untilElapsedMs: 30_000 }, talkUnconfirmed: true });
    expect(screen.queryByTestId("parent_talk_unconfirmed")).toBeNull();
  });

  it("shows the silent-microphone hint only while talking, outside any live region", () => {
    mount({ talk: { kind: "talking", untilElapsedMs: 30_000 }, talkMicSilent: true });
    const hint = screen.getByTestId("parent_talk_mic_silent");
    expect(hint).toHaveTextContent("Your microphone isn't picking up sound.");
    expect(hint.closest("[aria-live]")?.getAttribute("aria-live") ?? "off").toBe("off");
    expect(hint).not.toHaveAttribute("role", "status");
  });

  it("hides the silent-microphone hint once the talk is over", () => {
    mount({ talkMicSilent: true });
    expect(screen.queryByTestId("parent_talk_mic_silent")).toBeNull();
  });

  it("shows the noise pause line", () => {
    mount({ noisePause: "you" });
    expect(screen.getByTestId("parent_noise_paused")).toBeInTheDocument();
  });
});
