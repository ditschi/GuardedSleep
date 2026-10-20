/** Shared slide-up threshold logic (listen lock + kiosk unlock). */
export function shouldCompleteSlideUp(deltaY: number, thresholdPx: number): boolean {
  return deltaY <= -thresholdPx;
}

export function slideUpProgress(deltaY: number, thresholdPx: number): number {
  return Math.min(1, Math.max(0, -deltaY / thresholdPx));
}
