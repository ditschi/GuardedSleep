/**
 * UI-side timing of the talk button. The platform layer has its own copy of the numbers it needs
 * (`platform/policy.ts`); the UI may not import that layer (AGENTS.md section 5.1).
 */

/** The button must be held this long before a talk starts; a shorter tap shows a hint. */
export const TALK_HOLD_MS = 300;
/** Upward drag in pixels that would try to lock the button: it cannot be locked. */
export const TALK_LOCK_DISTANCE = 56;
/** A talk shows a countdown for its last seconds. */
export const TALK_COUNTDOWN_FROM_MS = 5_000;
/** How long the "hold the button" and "can't be locked" hints stay. */
export const TALK_HINT_MS = 3_000;
