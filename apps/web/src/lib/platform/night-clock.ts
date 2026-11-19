import { readable, type Readable } from "svelte/store";
import { localOffsetMs, nextBoundaryWallMs, nightActive, type NightSettings } from "../night-mode";

/** The longest wait before the clock is read again, so a changed clock or zone is picked up. */
export const NIGHT_RECHECK_MS = 60_000;

/**
 * Resolves the night settings against the device clock (`ui-appearance` "Automatic night mode by
 * time"). It emits again at each window boundary and, under Automatic, re-reads the clock at least
 * once a minute. Presentation only: no core input and no sensor. `now` and `offset` are seams for
 * tests.
 */
export function createNightClock(
  settings: Readable<NightSettings>,
  now: () => number = Date.now,
  offset: (wallMs: number) => number = localOffsetMs,
): Readable<boolean> {
  return readable(false, (set) => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const unsubscribe = settings.subscribe((s) => {
      if (timer !== undefined) clearTimeout(timer);
      const tick = (): void => {
        const wall = now();
        const off = offset(wall);
        set(nightActive(s, wall, off));
        if (s.nightMode !== "auto") return;
        const boundary = nextBoundaryWallMs(s.nightFromMinute, s.nightToMinute, wall, off);
        const until = s.nightOverride?.untilWallMs ?? null;
        const held = until !== null && until > wall ? until : null;
        const wake = Math.min(boundary ?? Infinity, held ?? Infinity);
        timer = setTimeout(tick, Math.min(Math.max(wake - wall, 1), NIGHT_RECHECK_MS));
      };
      tick();
    });
    return () => {
      unsubscribe();
      if (timer !== undefined) clearTimeout(timer);
    };
  });
}
