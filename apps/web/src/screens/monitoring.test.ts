import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import App from "../App.svelte";
import { defaultLinkUi, type LinkUi } from "../lib/contract";
import { createFakeController, type FakeController } from "../lib/contract/fake-controller";
import { locale } from "../lib/i18n/store";
import { elapsedToWall } from "../lib/ui/parent-view";

function open(path: string, controller: FakeController) {
  window.history.replaceState(null, "", path);
  return render(App, { controller });
}

const link = (over: Partial<LinkUi>): LinkUi => ({ ...defaultLinkUi(), ...over });

function babyController(): FakeController {
  const c = createFakeController();
  c.app.update((s) => ({ ...s, role: "baby", paired: true, monitoring: true }));
  c.baby.update((b) => ({ ...b, link: link({ phase: "connected" }) }));
  return c;
}

function parentController(): FakeController {
  const c = createFakeController();
  c.app.update((s) => ({ ...s, role: "parent", paired: true, monitoring: true }));
  c.soundUnlocked.set(true);
  c.parent.update((p) => ({
    ...p,
    link: link({ phase: "connected", lastAliveElapsedMs: Date.now() }),
    quietSinceWallMs: Date.now() - 5 * 60_000,
  }));
  return c;
}

afterEach(() => {
  window.history.replaceState(null, "", "/");
  locale.set("en");
  document.title = "";
});

describe("S07 baby monitor", () => {
  let c: FakeController;
  beforeEach(() => {
    c = babyController();
  });

  it("shows the connection in words, the mic level and the keep-awake advice", async () => {
    open("/baby", c);
    const status = await screen.findByTestId("baby_status");
    expect(status).toHaveTextContent("Connected to parent device");
    expect(status).toHaveAttribute("data-state", "connected");
    expect(screen.getByTestId("baby_monitoring_line")).toHaveTextContent("Monitoring");
    expect(screen.getByTestId("baby_mic_level")).toHaveAttribute("role", "meter");
    const advice = screen.getByTestId("baby_keep_awake");
    expect(advice).toHaveTextContent("plugged in, lid open, this tab visible");
    expect(advice).toHaveTextContent("Do Not Disturb");
  });

  it("reports silence as unknown and shows the battery where the browser has one", async () => {
    open("/baby", c);
    expect(await screen.findByTestId("baby_silence")).toHaveTextContent("Quiet mode: unknown");
    expect(screen.queryByTestId("baby_battery")).toBeNull();
    c.deviceBattery.set({ pct: 80, charging: true });
    expect(await screen.findByTestId("baby_battery")).toHaveTextContent("charging · 80%");
    c.deviceBattery.set({ pct: 64, charging: false });
    await waitFor(() =>
      expect(screen.getByTestId("baby_battery")).toHaveTextContent("battery 64%"),
    );
  });

  const states: Array<[Partial<LinkUi>, string]> = [
    [{ phase: "connecting" }, "Waiting for parent device"],
    [{ phase: "reconnecting", level: 1 }, "Reconnecting to parent device"],
    [{ phase: "reconnecting", level: 2 }, "Parent device not responding"],
    [{ phase: "peer_stopped" }, "Parent device stopped monitoring"],
    [{ phase: "failed_needs_user", failureKey: "pairing_key_mismatch" }, "no longer paired"],
  ];
  it.each(states)("renders %j as '%s'", async (over, text) => {
    open("/baby", c);
    c.baby.update((b) => ({ ...b, link: link(over) }));
    await waitFor(() => expect(screen.getByTestId("baby_status")).toHaveTextContent(text));
  });

  it("says when the parent is listening", async () => {
    open("/baby", c);
    expect(screen.queryByTestId("baby_listening")).toBeNull();
    c.baby.update((b) => ({ ...b, parentListening: true }));
    expect(await screen.findByTestId("baby_listening")).toHaveTextContent("Parent is listening");
  });

  it("names a refused microphone and lets the user try again", async () => {
    c.baby.update((b) => ({
      ...b,
      link: link({ phase: "failed_needs_user", failureKey: "mic_denied" }),
    }));
    open("/baby", c);
    expect(await screen.findByTestId("baby_failure")).toHaveTextContent(
      "The microphone is blocked",
    );
    await fireEvent.click(screen.getByTestId("baby_retry"));
    expect(c.calls).toContainEqual(["startMonitoring"]);
  });

  it("names a missing microphone", async () => {
    c.baby.update((b) => ({
      ...b,
      link: link({ phase: "failed_needs_user", failureKey: "mic_unavailable" }),
    }));
    open("/baby", c);
    expect(await screen.findByTestId("baby_failure")).toHaveTextContent("No microphone");
  });

  it("warns when the microphone delivers only silence", async () => {
    open("/baby", c);
    c.baby.update((b) => ({ ...b, micSilent: true }));
    expect(await screen.findByTestId("baby_mic_silent")).toBeInTheDocument();
  });

  it("asks before it stops, then returns to the start screen", async () => {
    open("/baby", c);
    await fireEvent.click(await screen.findByTestId("baby_stop"));
    const dialog = screen.getByTestId("dialog_stop");
    expect(dialog).toHaveTextContent("The parent device will stop getting alerts.");
    await fireEvent.click(within(dialog).getByTestId("dialog_stop_confirm"));
    expect(c.calls).toContainEqual(["stopMonitoring"]);
    await waitFor(() => expect(window.location.pathname).toBe("/"));
  });

  it("keeps monitoring when the user declines", async () => {
    open("/baby", c);
    await fireEvent.click(await screen.findByTestId("baby_stop"));
    await fireEvent.click(screen.getByTestId("dialog_stop_cancel"));
    expect(c.calls).not.toContainEqual(["stopMonitoring"]);
    expect(window.location.pathname).toBe("/baby");
  });
});

describe("S09 parent monitor", () => {
  let c: FakeController;
  beforeEach(() => {
    c = parentController();
  });

  it("shows the status bar, the headline and the quiet meter", async () => {
    c.app.update((s) => ({ ...s, peerName: "Nursery" }));
    open("/parent", c);
    const bar = await screen.findByTestId("status_bar");
    // Neutral, with the connection name after the state; colour only ever means a change.
    expect(bar).toHaveTextContent("Connected · Nursery");
    expect(bar).toHaveAttribute("data-tone", "ok");
    expect(bar.querySelector("svg")).not.toBeNull();
    expect(screen.getByTestId("parent_headline")).toHaveTextContent(/All quiet since \d/);
    expect(screen.getByTestId("parent_meter")).toHaveAccessibleName("Sound level: quiet");
    expect(screen.getByTestId("parent_keep_open_banner")).toHaveTextContent(
      "Keep this page open and the screen on",
    );
  });

  it("grey-chips the silence of a browser baby device and offers no silence request", async () => {
    c.parent.update((p) => ({
      ...p,
      peer: { ...p.peer, isWebClient: true, batteryPct: 80, silence: "unknown" },
    }));
    open("/parent", c);
    expect(await screen.findByTestId("parent_silence_chip")).toHaveTextContent(
      "Quiet mode unknown · turn it on at the baby device",
    );
    expect(screen.getByTestId("parent_silence_chip")).toHaveAttribute("data-tone", "neutral");
    expect(screen.getByTestId("parent_baby_battery_chip")).toHaveTextContent("Baby device 80%");
    expect(screen.queryByTestId("sheet_silence_apply")).toBeNull();
  });

  it("offers to silence a native baby device that may ring", async () => {
    c.parent.update((p) => ({ ...p, peer: { ...p.peer, silence: "may_ring" } }));
    open("/parent", c);
    await fireEvent.click(await screen.findByTestId("sheet_silence_apply"));
    expect(c.calls).toContainEqual(["requestSilenceBaby"]);
  });

  it("turns the headline into 'last contact' and disables Listen when the link is down", async () => {
    open("/parent", c);
    c.parent.update((p) => ({
      ...p,
      link: link({ phase: "reconnecting", level: 1, lastAliveElapsedMs: Date.now() }),
      quietSinceWallMs: null,
    }));
    await waitFor(() =>
      expect(screen.getByTestId("parent_headline")).toHaveTextContent(/Last contact \d/),
    );
    expect(screen.getByTestId("parent_headline")).not.toHaveTextContent("All quiet");
    expect(screen.getByTestId("parent_listen")).toBeDisabled();
    expect(screen.getByTestId("parent_listen")).toHaveTextContent("Listen when connected");
  });

  it("makes Listen a 72 px target that keyboard users switch on and off", async () => {
    open("/parent", c);
    const listen = await screen.findByTestId("parent_listen");
    expect(listen).toHaveAttribute("data-listen", "off");
    expect(listen).toHaveTextContent("Hold to listen");
    // A keyboard or assistive-technology activation is a click without a pointer (detail 0).
    await fireEvent.click(listen, { detail: 0 });
    expect(c.calls).toContainEqual(["setListen", "locked"]);
    await waitFor(() => expect(listen).toHaveTextContent("Listening · tap to stop"));
    expect(listen).toHaveAttribute("aria-pressed", "true");
    await fireEvent.click(listen, { detail: 0 });
    expect(c.calls).toContainEqual(["setListen", "off"]);
  });

  it("listens while the button is held and stops on release", async () => {
    open("/parent", c);
    const listen = await screen.findByTestId("parent_listen");
    await fireEvent.pointerDown(listen, { pointerId: 1, button: 0, clientY: 300 });
    expect(c.calls).toContainEqual(["setListen", "hold"]);
    await fireEvent.pointerUp(listen, { pointerId: 1, button: 0, clientY: 300 });
    expect(c.calls.filter((call) => call[0] === "setListen").map((call) => call[1])).toEqual([
      "hold",
      "off",
    ]);
  });

  it("keeps listening when the finger slides up", async () => {
    open("/parent", c);
    const listen = await screen.findByTestId("parent_listen");
    await fireEvent.pointerDown(listen, { pointerId: 1, button: 0, clientY: 300 });
    await fireEvent.pointerMove(listen, { pointerId: 1, clientY: 200 });
    await fireEvent.pointerUp(listen, { pointerId: 1, button: 0, clientY: 200 });
    expect(c.calls.filter((call) => call[0] === "setListen").map((call) => call[1])).toEqual([
      "hold",
      "locked",
    ]);
  });

  it("shows 'Tap to turn on alert sound' until the sound is unlocked", async () => {
    c.soundUnlocked.set(false);
    open("/parent", c);
    const gate = await screen.findByTestId("parent_sound_gate");
    expect(gate).toHaveTextContent("Tap to turn on alert sound");
    await fireEvent.click(gate);
    expect(c.calls).toContainEqual(["unlockSound"]);
    await waitFor(() => expect(screen.queryByTestId("parent_sound_gate")).toBeNull());
  });

  it("snoozes through the sheet and ends the snooze again", async () => {
    open("/parent", c);
    await fireEvent.click(await screen.findByTestId("parent_snooze"));
    const sheet = screen.getByTestId("sheet_snooze");
    expect(sheet).toHaveTextContent("You still get alerts if the connection drops");
    await fireEvent.click(within(sheet).getByTestId("sheet_snooze_10"));
    expect(c.calls).toContainEqual(["snooze", 10]);
    expect(await screen.findByTestId("parent_snooze_chip")).toHaveTextContent("Snoozed ·");
    await fireEvent.click(screen.getByTestId("parent_snooze"));
    await fireEvent.click(screen.getByTestId("sheet_snooze_end"));
    expect(c.calls).toContainEqual(["cancelSnooze"]);
  });

  it("runs a test alert and asks whether it was heard", async () => {
    open("/parent", c);
    await fireEvent.click(await screen.findByTestId("parent_probe"));
    expect(screen.getByTestId("sheet_probe")).toHaveTextContent("The baby device stays silent.");
    await fireEvent.click(screen.getByTestId("sheet_probe_run"));
    expect(c.calls).toContainEqual(["requestTestAlert"]);
    // The baby device answers with a test alert; its screen carries the TEST badge.
    c.parent.update((p) => ({
      ...p,
      noiseAlertActive: true,
      noiseIsTest: true,
      noiseSinceWallMs: Date.now(),
    }));
    expect(await screen.findByTestId("alert_test_badge")).toHaveTextContent("TEST");
    await fireEvent.click(screen.getByTestId("alert_stop_sound"));
    const question = await screen.findByTestId("sheet_probe_question");
    await fireEvent.click(within(question).getByTestId("sheet_probe_yes"));
    expect(c.calls).toContainEqual(["reportTestResult", true]);
  });

  it("asks at once when the test only sounded on this device", async () => {
    c.parent.update((p) => ({ ...p, link: link({ phase: "reconnecting", level: 1 }) }));
    open("/parent", c);
    await fireEvent.click(await screen.findByTestId("parent_probe"));
    await fireEvent.click(screen.getByTestId("sheet_probe_run"));
    const question = await screen.findByTestId("sheet_probe_question");
    expect(question).toHaveTextContent("Tested this device only");
    await fireEvent.click(within(question).getByTestId("sheet_probe_no"));
    expect(c.calls).toContainEqual(["reportTestResult", false]);
  });

  it("keeps Stop monitoring inside More and asks before it stops", async () => {
    open("/parent", c);
    await screen.findByTestId("parent_overflow");
    expect(screen.getByTestId("parent_stop")).not.toBeVisible();
    await fireEvent.click(screen.getByTestId("parent_overflow"));
    await fireEvent.click(screen.getByTestId("parent_stop"));
    const dialog = screen.getByTestId("dialog_stop");
    await fireEvent.click(within(dialog).getByTestId("dialog_stop_confirm"));
    expect(c.calls).toContainEqual(["stopMonitoring"]);
    await waitFor(() => expect(window.location.pathname).toBe("/"));
  });

  it("shows the missed-events summary after an outage and lets the user dismiss it", async () => {
    c.parent.update((p) => ({
      ...p,
      missed: { downMs: 120_000, count: 2, complete: true, endWallMs: null, loudest: 0.7 },
    }));
    open("/parent", c);
    const card = await screen.findByTestId("card_missed_events");
    expect(card).toHaveTextContent("While disconnected: 2 sounds");
    await fireEvent.click(within(card).getByRole("button"));
    expect(c.calls).toContainEqual(["dismissMissedEvents"]);
  });

  it("keeps the 'Connected again' card until OK and shows the sound result", async () => {
    c.parent.update((p) => ({
      ...p,
      restoredAfterMs: 108_000,
      missed: { downMs: 108_000, count: 0, complete: true, endWallMs: null, loudest: null },
    }));
    open("/parent", c);
    const card = await screen.findByTestId("restored_card");
    expect(card).toHaveTextContent("Connected again");
    expect(screen.getByTestId("restored_body")).toHaveTextContent(
      "Monitoring was interrupted for 1 min 48 s.",
    );
    expect(screen.getByTestId("restored_result")).toHaveTextContent(
      "The baby device heard no sound during that time.",
    );
    await fireEvent.click(screen.getByTestId("restored_ok"));
    expect(c.calls).toContainEqual(["dismissRestored"]);
    await waitFor(() => expect(screen.queryByTestId("restored_card")).toBeNull());
  });

  it("says sounds were heard, or that it does not know, without a count", async () => {
    c.parent.update((p) => ({
      ...p,
      restoredAfterMs: 90_000,
      missed: { downMs: 90_000, count: 2, complete: true, endWallMs: null, loudest: 0.7 },
    }));
    open("/parent", c);
    expect(await screen.findByTestId("restored_result")).toHaveTextContent("It heard 2 sounds.");
    c.parent.update((p) => ({
      ...p,
      missed: { downMs: 90_000, count: 2, complete: false, endWallMs: null, loudest: null },
    }));
    await waitFor(() =>
      expect(screen.getByTestId("restored_result")).toHaveTextContent(
        "We don't know whether there were sounds.",
      ),
    );
    expect(screen.getByTestId("restored_result")).not.toHaveTextContent("2");
  });

  it("replaces the card when the link drops again", async () => {
    c.parent.update((p) => ({ ...p, restoredAfterMs: 90_000 }));
    open("/parent", c);
    await screen.findByTestId("restored_card");
    c.parent.update((p) => ({
      ...p,
      restoredAfterMs: null,
      link: link({ phase: "reconnecting", level: 0, downSinceElapsedMs: Date.now() }),
    }));
    await waitFor(() => expect(screen.queryByTestId("restored_card")).toBeNull());
    expect(screen.getByTestId("status_bar")).toHaveAttribute("data-kind", "reconnecting");
  });

  it("has no more than two warning chips and counts the rest", async () => {
    c.parent.update((p) => ({
      ...p,
      peer: { ...p.peer, unitWarnings: ["page_hidden", "not_charging", "wake_lock_off"] },
    }));
    open("/parent", c);
    await screen.findByTestId("status_bar");
    expect(screen.getByTestId("parent_more_chip")).toHaveTextContent("+1");
  });
});

describe("S10 noise alert", () => {
  let c: FakeController;
  beforeEach(() => {
    c = parentController();
  });

  function startNoise(test = false): void {
    c.parent.update((p) => ({
      ...p,
      noiseAlertActive: true,
      noiseIsTest: test,
      noiseSinceWallMs: Date.now() - 1000,
      level: 0.8,
    }));
  }

  it("takes over the page with title, start time, meter and the stop action", async () => {
    open("/parent", c);
    await screen.findByTestId("parent_headline");
    startNoise();
    const alert = await screen.findByRole("alertdialog");
    expect(alert).toHaveAccessibleName("Sound detected");
    expect(screen.getByTestId("alert_since")).toHaveTextContent(/Since \d/);
    expect(screen.getByTestId("alert_meter")).toHaveAccessibleName("Sound level: loud");
    expect(screen.queryByTestId("alert_test_badge")).toBeNull();
    expect(screen.queryByTestId("parent_headline")).toBeNull();
    await waitFor(() => expect(screen.getByTestId("alert_title")).toHaveFocus());
  });

  it("flashes the tab title while it is up and restores it after", async () => {
    open("/parent", c);
    await screen.findByTestId("parent_headline");
    const before = document.title;
    startNoise();
    await screen.findByRole("alertdialog");
    expect(document.title).toBe("Sound detected");
    await fireEvent.click(screen.getByTestId("alert_stop_sound"));
    await waitFor(() => expect(document.title).toBe(before));
  });

  it("stops the sound and returns to the monitor", async () => {
    open("/parent", c);
    await screen.findByTestId("parent_headline");
    startNoise();
    await fireEvent.click(await screen.findByTestId("alert_stop_sound"));
    expect(c.calls).toContainEqual(["stopAlertSound"]);
    expect(await screen.findByTestId("parent_headline")).toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("snoozes for 10 or 30 minutes and stops the sound", async () => {
    open("/parent", c);
    await screen.findByTestId("parent_headline");
    startNoise();
    await fireEvent.click(await screen.findByTestId("alert_snooze_30"));
    expect(c.calls).toContainEqual(["snooze", 30]);
    expect(c.calls).toContainEqual(["stopAlertSound"]);
    await screen.findByTestId("parent_headline");
  });

  it("marks a test alert with a TEST badge", async () => {
    open("/parent", c);
    await screen.findByTestId("parent_headline");
    startNoise(true);
    expect(await screen.findByTestId("alert_test_badge")).toHaveTextContent("TEST");
  });

  it("shows the alert again for the next noise event", async () => {
    open("/parent", c);
    await screen.findByTestId("parent_headline");
    startNoise();
    await fireEvent.click(await screen.findByTestId("alert_stop_sound"));
    await screen.findByTestId("parent_headline");
    c.parent.update((p) => ({ ...p, noiseAlertActive: false }));
    startNoise();
    expect(await screen.findByRole("alertdialog")).toBeInTheDocument();
  });

  it("lets the parent listen from the alert", async () => {
    open("/parent", c);
    await screen.findByTestId("parent_headline");
    startNoise();
    await fireEvent.click(await screen.findByTestId("alert_listen"), { detail: 0 });
    expect(c.calls).toContainEqual(["setListen", "locked"]);
  });
});

describe("S11 connection alert", () => {
  let c: FakeController;
  beforeEach(() => {
    c = parentController();
  });

  /** Down for 2 min with the core's timing fields: the next level (loud alarm) is 60 s ahead. */
  function lose(over: Partial<LinkUi> = {}): void {
    const now = Date.now();
    c.parent.update((p) => ({
      ...p,
      link: link({
        phase: "reconnecting",
        level: 2,
        lastAliveElapsedMs: now - 120_000,
        downSinceElapsedMs: now - 120_000,
        nextLevelAtElapsedMs: now + 60_000,
        causeHint: "peer_absent",
        ...over,
      }),
      quietSinceWallMs: null,
    }));
  }

  it("takes over the page with label, headline, counter and the next stage", async () => {
    open("/parent", c);
    await screen.findByTestId("parent_headline");
    lose();
    const alarm = await screen.findByRole("alertdialog");
    expect(alarm).toHaveAccessibleName("Please check on the child");
    expect(screen.getByTestId("alert_label")).toHaveTextContent("This device is sounding an alert");
    expect(screen.getByTestId("link_alarm_since")).toHaveTextContent("2:00");
    expect(screen.getByTestId("link_alarm_since")).toHaveTextContent("without connection");
    expect(screen.getByTestId("link_alarm_sentence")).toHaveTextContent(
      "You can't hear the child. Loud alarm at",
    );
    expect(screen.getByTestId("link_alarm_cause")).toHaveTextContent(
      "The baby device isn't online.",
    );
    expect(screen.getByTestId("link_alarm_step")).toHaveTextContent("plugged in");
  });

  it("states the counter in words and keeps it out of every live region", async () => {
    open("/parent", c);
    lose();
    const counter = await screen.findByTestId("link_alarm_since");
    expect(counter).toHaveTextContent("2 minutes");
    expect(counter.closest("[aria-live], [role=alert], [role=status]")).toBeNull();
    expect(counter).not.toHaveAttribute("aria-live");
  });

  it("shows the next-stage time as next_level_at_ms on the wall clock", async () => {
    open("/parent", c);
    const at = Date.now() + 90_000;
    lose({ nextLevelAtElapsedMs: at });
    const expected = new Intl.DateTimeFormat("en", { hour: "2-digit", minute: "2-digit" }).format(
      elapsedToWall(at, Date.now(), Date.now()),
    );
    expect(await screen.findByTestId("link_alarm_sentence")).toHaveTextContent(expected);
  });

  it("names the loud alarm at level 3 without a level number", async () => {
    open("/parent", c);
    lose({ level: 3, nextLevelAtElapsedMs: null });
    const alarm = await screen.findByRole("alertdialog");
    expect(alarm).toHaveAccessibleName("Still no connection");
    expect(screen.getByTestId("alert_label")).toHaveTextContent("Loud alarm sounding");
    expect(screen.getByTestId("link_alarm_sentence")).toHaveTextContent("Go to the child now.");
    expect(alarm).not.toHaveTextContent(/level/i);
  });

  it("says 'Couldn't connect' when monitoring never started", async () => {
    open("/parent", c);
    lose({ phase: "connecting", lastAliveElapsedMs: null });
    expect(await screen.findByRole("alertdialog")).toHaveAccessibleName("Couldn't connect");
    expect(screen.getByTestId("link_alarm_sentence")).toHaveTextContent(
      "Monitoring never started.",
    );
  });

  it("acknowledges with 'I'm checking' and then stays red as 'Alarm paused'", async () => {
    open("/parent", c);
    lose();
    await fireEvent.click(await screen.findByTestId("link_alarm_ack"));
    expect(c.calls).toContainEqual(["acknowledgeAlarm"]);
    c.parent.update((p) => ({
      ...p,
      link: {
        ...p.link,
        nextLevelAtElapsedMs: null,
        acknowledgedUntilElapsedMs: Date.now() + 300_000,
      },
    }));
    await waitFor(() =>
      expect(screen.getByTestId("alert_label")).toHaveTextContent("Alarm paused"),
    );
    expect(screen.getByTestId("link_alarm_title")).toHaveTextContent("Comes back at");
    expect(screen.queryByTestId("link_alarm_ack")).toBeNull();
    expect(screen.getByTestId("parent_link_alarm")).toBeInTheDocument();
  });

  it("folds the further steps behind a disclosure button with an expanded state", async () => {
    open("/parent", c);
    lose();
    const more = await screen.findByTestId("link_alarm_more_steps");
    expect(more).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByTestId("link_alarm_more_0")).toBeNull();
    await fireEvent.click(more);
    expect(more).toHaveAttribute("aria-expanded", "true");
    expect(await screen.findByTestId("link_alarm_more_0")).toHaveTextContent("Is it on Wi-Fi?");
    expect(screen.getByTestId("link_alarm_more_0")).toHaveFocus();
  });

  it("'Try now' resets the backoff through the existing input", async () => {
    open("/parent", c);
    lose();
    await fireEvent.click(await screen.findByTestId("link_alarm_try_now"));
    expect(c.calls).toContainEqual(["tryReconnectNow"]);
  });

  it("asks before it stops: the confirmation silences and acknowledges nothing", async () => {
    open("/parent", c);
    lose({ level: 3, nextLevelAtElapsedMs: null });
    await fireEvent.click(await screen.findByTestId("link_alarm_stop"));
    expect(await screen.findByTestId("dialog_stop")).toBeVisible();
    // Opening the confirmation did not touch the alarm; "Keep monitoring" returns to it unchanged.
    expect(c.calls.map((x) => x[0])).not.toContain("acknowledgeAlarm");
    expect(c.calls.map((x) => x[0])).not.toContain("stopAlertSound");
    expect(c.calls.map((x) => x[0])).not.toContain("stopMonitoring");
    await fireEvent.click(screen.getByTestId("dialog_stop_cancel"));
    expect(screen.getByTestId("parent_link_alarm")).toBeInTheDocument();
    expect(c.calls.map((x) => x[0])).not.toContain("stopMonitoring");
  });

  it("stops monitoring from the alarm only after the confirmation", async () => {
    open("/parent", c);
    lose();
    await fireEvent.click(await screen.findByTestId("link_alarm_stop"));
    await fireEvent.click(await screen.findByTestId("dialog_stop_confirm"));
    expect(c.calls).toContainEqual(["stopMonitoring"]);
    await waitFor(() => expect(window.location.pathname).toBe("/"));
  });

  it("'Wrong connection?' on the red screen goes through the stop confirmation", async () => {
    open("/parent", c);
    lose({ phase: "connecting", lastAliveElapsedMs: null });
    await fireEvent.click(await screen.findByTestId("link_alarm_wrong_connection"));
    expect(c.calls.map((x) => x[0])).not.toContain("chooseAnotherConnection");
    await fireEvent.click(await screen.findByTestId("dialog_choose_other_confirm"));
    expect(c.calls).toContainEqual(["chooseAnotherConnection"]);
  });

  it("'Wrong connection?' on the blue monitor stops with one tap", async () => {
    c.parent.update((p) => ({ ...p, link: link({ phase: "connecting", level: 0 }) }));
    open("/parent", c);
    await fireEvent.click(await screen.findByTestId("parent_choose_other_connection"));
    expect(c.calls).toContainEqual(["chooseAnotherConnection"]);
  });

  it("puts the lost connection before a noise alert", async () => {
    open("/parent", c);
    await screen.findByTestId("parent_headline");
    c.parent.update((p) => ({ ...p, noiseAlertActive: true, noiseSinceWallMs: Date.now() }));
    await screen.findByTestId("alert_title");
    lose();
    expect(await screen.findByTestId("link_alarm_title")).toBeInTheDocument();
    expect(screen.queryByTestId("alert_title")).toBeNull();
  });

  it("tells the parent that the baby device stopped, and closes with OK", async () => {
    open("/parent", c);
    c.parent.update((p) => ({ ...p, link: link({ phase: "peer_stopped" }) }));
    const alarm = await screen.findByRole("alertdialog");
    expect(alarm).toHaveAccessibleName("Baby device stopped monitoring");
    expect(screen.queryByTestId("link_alarm_ack")).toBeNull();
    await fireEvent.click(screen.getByTestId("link_alarm_ok"));
    expect(await screen.findByTestId("parent_headline")).toBeInTheDocument();
    expect(screen.getByTestId("status_bar")).toHaveTextContent("Stopped");
  });

  it("warns about an almost empty baby device battery", async () => {
    open("/parent", c);
    c.parent.update((p) => ({ ...p, peer: { ...p.peer, batteryPct: 3, charging: false } }));
    const alarm = await screen.findByRole("alertdialog");
    expect(alarm).toHaveAccessibleName("Baby device battery almost empty");
    await fireEvent.click(screen.getByTestId("link_alarm_ok"));
    expect(await screen.findByTestId("parent_headline")).toBeInTheDocument();
  });

  it("asks to pair again when the key no longer matches", async () => {
    open("/parent", c);
    c.parent.update((p) => ({
      ...p,
      link: link({ phase: "failed_needs_user", failureKey: "pairing_key_mismatch" }),
    }));
    await fireEvent.click(await screen.findByTestId("link_alarm_pair_again"));
    expect(c.calls).toContainEqual(["forgetPairing"]);
    await waitFor(() => expect(window.location.pathname).toBe("/parent/pair"));
  });

  it("opens the reconnect help from the monitor while the connection is in trouble", async () => {
    open("/parent", c);
    c.parent.update((p) => ({
      ...p,
      link: link({ phase: "reconnecting", level: 1, lastAliveElapsedMs: Date.now() }),
    }));
    await fireEvent.click(await screen.findByTestId("parent_help_reconnect"));
    expect(screen.getByTestId("sheet_help_reconnect")).toHaveTextContent(
      "Is the baby device on and showing GuardedSleep?",
    );
  });
});

describe("status bar per stage", () => {
  let c: FakeController;
  beforeEach(() => {
    c = parentController();
    c.app.update((s) => ({ ...s, peerName: "Nursery" }));
  });

  const table: Array<[Partial<LinkUi>, string, string, string]> = [
    [{ phase: "connecting", level: 0 }, "info", "connecting", "Connecting · Nursery"],
    [{ phase: "reconnecting", level: 0 }, "info", "reconnecting", "Reconnecting · Nursery"],
    // First connection not answered: orange at 25 s while connecting, not blue.
    [{ phase: "connecting", level: 1 }, "orange", "not_connected_yet", "Not connected yet"],
    [{ phase: "reconnecting", level: 1 }, "orange", "cant_reach", "Can't reach · Nursery"],
    [{ phase: "reconnecting", level: 2 }, "bad", "no_connection", "No connection · Nursery"],
    [{ phase: "peer_stopped" }, "neutral", "baby_stopped", "Stopped · Nursery"],
  ];
  it.each(table)("%j is %s (%s)", async (over, tone, kind, text) => {
    open("/settings", c);
    c.parent.update((p) => ({
      ...p,
      link: link({ ...over, downSinceElapsedMs: Date.now() - 25_000 }),
    }));
    const bar = await screen.findByTestId("status_bar");
    expect(bar).toHaveAttribute("data-tone", tone);
    expect(bar).toHaveAttribute("data-kind", kind);
    expect(bar).toHaveTextContent(text);
  });

  it("is shown on Settings while monitoring, and only the stage words are a live region", async () => {
    open("/settings", c);
    const bar = await screen.findByTestId("status_bar");
    expect(bar).toHaveTextContent("Connected · Nursery");
    expect(screen.getByTestId("status_bar_text")).toHaveAttribute("role", "status");
    c.parent.update((p) => ({
      ...p,
      link: link({ phase: "reconnecting", level: 2, downSinceElapsedMs: Date.now() - 70_000 }),
    }));
    await waitFor(() =>
      expect(screen.getByTestId("status_bar_text")).toHaveAttribute("role", "alert"),
    );
    // The ticking duration sits outside the live region.
    expect(screen.getByTestId("status_bar_duration").closest("[role=alert]")).toBeNull();
    expect(screen.getByTestId("status_bar_duration")).toHaveTextContent("1 min 10 s");
  });

  it("stays visible and announced at the top of an open sheet", async () => {
    open("/parent", c);
    await fireEvent.click(await screen.findByTestId("parent_overflow"));
    const sheet = await screen.findByTestId("sheet_more");
    const bar = within(sheet).getByTestId("sheet_status_bar");
    expect(bar).toHaveTextContent("Connected · Nursery");
    expect(within(sheet).getByTestId("sheet_status_bar_text")).toHaveAttribute("role", "status");
    // It follows the link while the sheet is open.
    c.parent.update((p) => ({
      ...p,
      link: link({ phase: "reconnecting", level: 1, downSinceElapsedMs: Date.now() - 25_000 }),
    }));
    await waitFor(() => expect(bar).toHaveAttribute("data-kind", "cant_reach"));
  });

  it("has no bar before monitoring", async () => {
    c.app.update((s) => ({ ...s, monitoring: false }));
    open("/settings", c);
    await screen.findByRole("heading", { level: 1 });
    expect(screen.queryByTestId("status_bar")).toBeNull();
  });
});
