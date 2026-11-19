import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import SoundGate from "./SoundGate.svelte";

describe("SoundGate", () => {
  it("says the sound is off and unlocks on a tap", async () => {
    const onunlock = vi.fn();
    render(SoundGate, { onunlock });
    expect(screen.getByRole("heading", { name: "Alert sound is off" })).toBeInTheDocument();
    expect(screen.getByTestId("sound_gate")).toHaveTextContent("needs one tap");
    await fireEvent.click(screen.getByTestId("sound_gate_unlock"));
    expect(onunlock).toHaveBeenCalledOnce();
    expect(screen.getByTestId("sound_gate_unlock")).toHaveTextContent("Tap to turn on alert sound");
  });
});
