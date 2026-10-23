import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../App.svelte";
import { makePairingState, type PairingError } from "../lib/contract";
import {
  createFakeController,
  createFakeScanner,
  type FakeController,
  type FakeScanner,
} from "../lib/contract/fake-controller";
import { locale } from "../lib/i18n/store";

// The key is in the code on purpose: the tests prove it never reaches the page text.
const PSK = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const INVITE = JSON.stringify({ v: 3, k: PSK, s: ["wss://relay.example.test"], x: 1_900_000_000 });

function open(path: string, controller: FakeController, scanner?: FakeScanner) {
  window.history.replaceState(null, "", path);
  return render(App, { controller, scanner });
}

const heading = (name: string | RegExp) => screen.findByRole("heading", { level: 1, name });

describe("S01 role screen", () => {
  afterEach(() => window.history.replaceState(null, "", "/"));

  it("offers two cards and an order hint", () => {
    open("/", createFakeController());
    expect(screen.getByTestId("role_title")).toHaveTextContent("Use this device as");
    expect(screen.getByTestId("role_order_hint")).toHaveTextContent("Set up the baby device first");
    expect(screen.getByTestId("role_baby")).toHaveTextContent("Baby device");
    expect(screen.getByTestId("role_parent")).toHaveTextContent("Parent device");
    expect(screen.getByTestId("home_settings")).toHaveAttribute("href", "/settings");
  });

  it("sets the role and opens the matching pairing screen", async () => {
    const controller = createFakeController();
    open("/", controller);
    await fireEvent.click(screen.getByTestId("role_baby"));
    expect(controller.calls).toContainEqual(["chooseRole", "baby"]);
    expect(await heading("Connect the parent device")).toBeInTheDocument();
    expect(window.location.pathname).toBe("/baby/pair");
  });

  it("leaves modified clicks to the browser", async () => {
    const controller = createFakeController();
    open("/", controller);
    // jsdom cannot navigate; the window listener runs after the app's own and only silences it.
    const silence = (event: Event): void => event.preventDefault();
    window.addEventListener("click", silence);
    await fireEvent.click(screen.getByTestId("role_parent"), { ctrlKey: true });
    window.removeEventListener("click", silence);
    expect(controller.calls).toEqual([]);
    expect(window.location.pathname).toBe("/");
  });
});

describe("S04 baby pair screen", () => {
  let controller: FakeController;
  let writeText: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    controller = createFakeController();
    writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
  });
  afterEach(() => window.history.replaceState(null, "", "/"));

  function showCode(remainingMs = 272_000): void {
    controller.pairing.set(
      makePairingState("showing_code", {
        codePayload: INVITE,
        expiresAtElapsedMs: performance.now() + remainingMs,
      }),
    );
  }

  it("takes the baby role and starts a fresh code when it opens", async () => {
    open("/baby/pair", controller);
    expect(await heading("Connect the parent device")).toBeInTheDocument();
    expect(controller.calls).toEqual([["chooseRole", "baby"], ["startPairing"]]);
  });

  it("shows the code as an image with a countdown, the hint and the waiting state", async () => {
    open("/baby/pair", controller);
    showCode();
    const qr = await screen.findByTestId("baby_pair_qr");
    expect(qr).toHaveAttribute("role", "img");
    expect(qr).toHaveAccessibleName("Pairing code");
    expect(qr.querySelector("path")).not.toBeNull();
    expect(screen.getByTestId("baby_pair_countdown")).toHaveTextContent(/Code valid for 4:3\d/);
    expect(screen.getByTestId("baby_pair_copy_hint")).toHaveTextContent(
      "Paste it on the parent device within 5 minutes. Don't send it in a chat.",
    );
    expect(screen.getByTestId("baby_pair_waiting")).toHaveTextContent("Waiting for parent device");
    expect(screen.getByTestId("baby_pair_privacy")).toBeInTheDocument();
  });

  it("never writes the key into the page text and offers no share action", async () => {
    open("/baby/pair", controller);
    showCode();
    await screen.findByTestId("baby_pair_qr");
    expect(document.body.textContent).not.toContain(PSK);
    expect(document.body.innerHTML).not.toContain(PSK);
    expect(screen.queryByRole("button", { name: /share/i })).toBeNull();
  });

  it("copies the code and says so", async () => {
    open("/baby/pair", controller);
    showCode();
    await fireEvent.click(await screen.findByTestId("baby_pair_copy"));
    expect(writeText).toHaveBeenCalledWith(INVITE);
    await waitFor(() => expect(screen.getByTestId("baby_pair_copy")).toHaveTextContent("Copied"));
  });

  it("makes the code brighter on request", async () => {
    open("/baby/pair", controller);
    showCode();
    const button = await screen.findByTestId("baby_pair_brighter");
    expect(button).toHaveAttribute("aria-pressed", "false");
    await fireEvent.click(button);
    expect(button).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByTestId("baby_pair_qr")).toHaveAttribute("data-bright");
  });

  it("counts the validity down while it is open", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "performance", "Date"] });
    try {
      open("/baby/pair", controller);
      showCode(65_000);
      expect(await screen.findByTestId("baby_pair_countdown")).toHaveTextContent("1:05");
      await vi.advanceTimersByTimeAsync(5_000);
      await waitFor(() =>
        expect(screen.getByTestId("baby_pair_countdown")).toHaveTextContent("1:00"),
      );
    } finally {
      vi.useRealTimers();
    }
  });

  it("offers a new code after it expired", async () => {
    open("/baby/pair", controller);
    controller.pairing.set(makePairingState("expired"));
    expect(await screen.findByTestId("baby_pair_expired")).toHaveTextContent("has expired");
    expect(screen.queryByTestId("baby_pair_qr")).toBeNull();
    await fireEvent.click(screen.getByTestId("baby_pair_new_code"));
    expect(controller.calls).toContainEqual(["refreshPairingCode"]);
  });

  it("names the microphone, not 'unreachable', when the run could not start", async () => {
    open("/baby/pair", controller);
    controller.baby.update((b) => ({
      ...b,
      link: { ...b.link, phase: "failed_needs_user", failureKey: "mic_denied" },
    }));
    controller.pairing.set(makePairingState("failed", { error: "unreachable" }));
    const error = await screen.findByTestId("baby_pair_error");
    expect(error).toHaveTextContent("The microphone is blocked");
    expect(error).not.toHaveTextContent(/reach/i);
  });

  it("explains a missing microphone", async () => {
    open("/baby/pair", controller);
    controller.baby.update((b) => ({
      ...b,
      link: { ...b.link, phase: "failed_needs_user", failureKey: "mic_unavailable" },
    }));
    controller.pairing.set(makePairingState("failed", { error: "unreachable" }));
    expect(await screen.findByTestId("baby_pair_error")).toHaveTextContent("No microphone");
  });

  it("falls back to the generic text for any other failure", async () => {
    open("/baby/pair", controller);
    controller.pairing.set(makePairingState("failed", { error: "key_mismatch" }));
    expect(await screen.findByTestId("baby_pair_error")).toHaveTextContent(
      "Pairing didn't work. Show a new code and try again.",
    );
  });

  it("moves on to monitoring once the other device paired", async () => {
    open("/baby/pair", controller);
    showCode();
    controller.app.update((s) => ({ ...s, role: "baby", paired: true, monitoring: true }));
    controller.pairing.set(makePairingState("paired"));
    await waitFor(() => expect(window.location.pathname).toBe("/baby"));
  });

  it("ends the pairing run when the user goes back", async () => {
    open("/baby/pair", controller);
    await fireEvent.click(await screen.findByTestId("baby_pair_back"));
    await waitFor(() => expect(window.location.pathname).toBe("/"));
    expect(controller.calls).toContainEqual(["cancelPairing"]);
  });
});

describe("S05 parent pair screen", () => {
  let controller: FakeController;
  let scanner: FakeScanner;

  beforeEach(() => {
    controller = createFakeController();
    scanner = createFakeScanner();
    locale.set("en");
  });
  afterEach(() => window.history.replaceState(null, "", "/"));

  it("takes the parent role and opens no camera until it is allowed", async () => {
    open("/parent/pair", controller, scanner);
    expect(await heading("Scan the code on the baby device")).toBeInTheDocument();
    expect(controller.calls).toEqual([["chooseRole", "parent"]]);
    expect(scanner.started).toHaveLength(0);
    expect(screen.getByTestId("parent_pair_camera_why")).toHaveTextContent(
      "uses the camera only to read the code",
    );
  });

  it("scans a code after 'Allow camera' and hands it to the controller", async () => {
    open("/parent/pair", controller, scanner);
    await fireEvent.click(await screen.findByTestId("parent_pair_allow_camera"));
    await waitFor(() => expect(scanner.started).toHaveLength(1));
    scanner.emit(INVITE);
    expect(controller.calls).toContainEqual(["submitPairingCode", INVITE]);
  });

  it("explains a refused camera and offers the paste field instead", async () => {
    open("/parent/pair", controller, scanner);
    scanner.failNext("denied");
    await fireEvent.click(await screen.findByTestId("parent_pair_allow_camera"));
    expect(await screen.findByTestId("parent_pair_camera_denied")).toHaveTextContent(
      "Camera not allowed.",
    );
    await fireEvent.click(screen.getByTestId("parent_pair_paste_instead"));
    expect(screen.getByTestId("parent_pair_paste_field")).toHaveFocus();
  });

  it("says so when there is no camera to use", async () => {
    open("/parent/pair", controller, scanner);
    scanner.failNext("no_camera");
    await fireEvent.click(await screen.findByTestId("parent_pair_allow_camera"));
    expect(await screen.findByTestId("parent_pair_camera_unavailable")).toBeInTheDocument();
  });

  it("shows only the paste path without a scanner", async () => {
    open("/parent/pair", controller);
    expect(await screen.findByTestId("parent_pair_camera_unavailable")).toBeInTheDocument();
    expect(screen.queryByTestId("parent_pair_allow_camera")).toBeNull();
  });

  it("submits the pasted code and clears the field once it is accepted", async () => {
    open("/parent/pair", controller, scanner);
    const field = await screen.findByTestId("parent_pair_paste_field");
    const submit = screen.getByTestId("parent_pair_paste_submit");
    expect(submit).toBeDisabled();
    await fireEvent.input(field, { target: { value: INVITE } });
    expect(submit).toBeEnabled();
    await fireEvent.click(submit);
    expect(controller.calls).toContainEqual(["submitPairingCode", INVITE]);
    controller.pairing.set(makePairingState("connecting"));
    await waitFor(() => expect(field).toHaveValue(""));
    expect(screen.getByTestId("parent_pair_connecting")).toHaveTextContent(
      "Connecting to the baby device",
    );
    expect(document.body.textContent).not.toContain(PSK);
  });

  const errors: Array<[PairingError, string]> = [
    ["not_our_code", "This isn't a GuardedSleep code"],
    ["expired", "This code has expired"],
    ["key_mismatch", "The code doesn't match"],
    ["already_paired", "already paired with another device"],
    ["unreachable", "Can't find the baby device yet"],
    ["incomplete_paste", "This code is incomplete"],
  ];

  it.each(errors)("shows the %s error", async (error, text) => {
    open("/parent/pair", controller, scanner);
    controller.pairing.set(makePairingState("failed", { error }));
    expect(await screen.findByTestId("parent_pair_error")).toHaveTextContent(text);
  });

  it("clears the error and starts again on 'Try again'", async () => {
    open("/parent/pair", controller, scanner);
    controller.pairing.set(makePairingState("failed", { error: "not_our_code" }));
    await fireEvent.click(await screen.findByTestId("parent_pair_retry"));
    expect(controller.calls).toContainEqual(["cancelPairing"]);
  });

  it("warns that the baby device cannot be found while still connecting", async () => {
    open("/parent/pair", controller, scanner);
    controller.pairing.set(makePairingState("connecting", { error: "unreachable" }));
    expect(await screen.findByTestId("parent_pair_error")).toHaveTextContent("Can't find");
    expect(screen.getByTestId("parent_pair_connecting")).toBeInTheDocument();
  });

  it("moves on to monitoring once the baby device answered", async () => {
    open("/parent/pair", controller, scanner);
    controller.app.update((s) => ({ ...s, role: "parent", paired: true, monitoring: true }));
    controller.pairing.set(makePairingState("paired"));
    await waitFor(() => expect(window.location.pathname).toBe("/parent"));
  });

  it("stops the camera when the screen closes", async () => {
    const view = open("/parent/pair", controller, scanner);
    await screen.findByTestId("parent_pair_title");
    view.unmount();
    expect(scanner.stops).toBeGreaterThan(0);
  });
});

describe("monitoring routes", () => {
  afterEach(() => window.history.replaceState(null, "", "/"));

  it("send a page without a running session back to the start screen", async () => {
    open("/baby", createFakeController());
    await waitFor(() => expect(window.location.pathname).toBe("/"));
  });
});
