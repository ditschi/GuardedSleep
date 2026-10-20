export type ListenGesturePhase = "idle" | "holding" | "locked";

/** Negative deltaY means the pointer moved upward (toward slide-to-lock). */
export function shouldLockOnSlide(deltaY: number, thresholdPx: number): boolean {
  return deltaY <= -thresholdPx;
}

export function nextPhaseAfterRelease(
  phase: ListenGesturePhase,
  lockedBySlide: boolean,
): ListenGesturePhase {
  if (lockedBySlide || phase === "locked") return "locked";
  return "idle";
}
