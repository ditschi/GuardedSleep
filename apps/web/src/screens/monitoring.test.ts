import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import App from "../App.svelte";
import { defaultLinkUi, type LinkUi } from "../lib/contract";
import { createFakeController, type FakeController } from "../lib/contract/fake-controller";
import { locale } from "../lib/i18n/store";

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

  it("shows the ambient bar, the headline and the quiet meter", async () => {
    open("/parent", c);
    const bar = await screen.findByTestId("parent_ambient_bar");
    expect(bar).toHaveTextContent("Connected to Baby device");
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

  it("says 'Connected again after' for a few seconds", async () => {
    c.parent.update((p) => ({ ...p, restoredAfterMs: 125_000 }));
    open("/parent", c);
    expect(await screen.findByTestId("parent_restored_toast")).toHaveTextContent(
      "Connected again after 2 min",
    );
  });

  it("has no more than two warning chips and counts the rest", async () => {
    c.parent.update((p) => ({
      ...p,
      peer: { ...p.peer, unitWarnings: ["page_hidden", "not_charging", "wake_lock_off"] },
    }));
    open("/parent", c);
    await screen.findByTestId("parent_ambient_bar");
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

describe("S11 link alarm", () => {
  let c: FakeController;
  beforeEach(() => {
    c = parentController();
  });

  function lose(level = 2): void {
    c.parent.update((p) => ({
      ...p,
      link: link({
        phase: "reconnecting",
        level,
        lastAliveElapsedMs: Date.now() - 120_000,
        causeHint: "peer_absent",
      }),
      quietSinceWallMs: null,
    }));
  }

  it("takes over the page when the connection is lost for good", async () => {
    open("/parent", c);
    await screen.findByTestId("parent_headline");
    lose();
    const alarm = await screen.findByRole("alertdialog");
    expect(alarm).toHaveAccessibleName("No connection to Baby device");
    expect(screen.getByTestId("link_alarm_duration")).toHaveTextContent("No connection for 2 min");
    expect(screen.getByTestId("link_alarm_body")).toHaveTextContent(
      "You won't hear your child until the devices reconnect.",
    );
    expect(screen.getByTestId("link_alarm_cause")).toHaveTextContent("isn't answering");
    expect(screen.getByTestId("link_alarm_ack_note")).toHaveTextContent("Sound off for 5 min");
    expect(screen.getByTestId("link_alarm_bar")).toHaveAttribute("role", "alert");
  });

  it("uses the urgent title at level 3", async () => {
    open("/parent", c);
    lose(3);
    expect(await screen.findByRole("alertdialog")).toHaveAccessibleName(
      "No connection to Baby device!",
    );
  });

  it("acknowledges with 'I'm checking'", async () => {
    open("/parent", c);
    lose();
    await fireEvent.click(await screen.findByTestId("link_alarm_ack"));
    expect(c.calls).toContainEqual(["acknowledgeAlarm"]);
  });

  it("leaves the full screen while the acknowledgement mutes it", async () => {
    open("/parent", c);
    lose();
    await screen.findByRole("alertdialog");
    c.parent.update((p) => ({
      ...p,
      link: { ...p.link, acknowledgedUntilElapsedMs: Date.now() + 300_000 },
    }));
    expect(await screen.findByTestId("parent_headline")).toBeInTheDocument();
    expect(screen.getByTestId("parent_ambient_bar")).toHaveAttribute("data-kind", "cant_reach");
  });

  it("stops monitoring from the alarm", async () => {
    open("/parent", c);
    lose();
    await fireEvent.click(await screen.findByTestId("link_alarm_stop"));
    expect(c.calls).toContainEqual(["stopMonitoring"]);
    await waitFor(() => expect(window.location.pathname).toBe("/"));
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
    expect(screen.getByTestId("parent_ambient_bar")).toHaveTextContent("stopped monitoring");
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

  it("opens the reconnect help from the bar of a monitor that is in trouble", async () => {
    open("/parent", c);
    lose();
    await fireEvent.click(await screen.findByTestId("link_alarm_ack"));
    c.parent.update((p) => ({
      ...p,
      link: { ...p.link, acknowledgedUntilElapsedMs: Date.now() + 300_000 },
    }));
    await fireEvent.click(await screen.findByTestId("parent_help_reconnect"));
    expect(screen.getByTestId("sheet_help_reconnect")).toHaveTextContent(
      "Is the baby device on and showing GuardedSleep?",
    );
  });
});
