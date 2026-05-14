import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import RolePicker from "./RolePicker.svelte";

describe("RolePicker", () => {
  it("calls onPick with baby or parent", () => {
    const onPick = vi.fn();
    render(RolePicker, { props: { onPick } });
    fireEvent.click(screen.getByTestId("role-baby"));
    expect(onPick).toHaveBeenCalledWith("baby");
    fireEvent.click(screen.getByTestId("role-parent"));
    expect(onPick).toHaveBeenCalledWith("parent");
  });
});
