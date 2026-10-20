import type { PairingMaterial } from "./pairing-v3.js";

/**
 * The pairing key lives in this module's memory only (ADR-004 amendment: web clients never
 * persist the PSK). A page reload forgets it; the UI says so and asks to pair again.
 */
export type ActivePairing = PairingMaterial & {
  /** The PSK verified at least once in this tab, so a later mismatch is never terminal. */
  pairedBefore: boolean;
  /** Baby only: the code shown to the parent and when it stops being valid. */
  codePayload: string | null;
  codeExpiresAtWallMs: number | null;
};

let active: ActivePairing | null = null;

export function setActivePairing(p: ActivePairing): void {
  active = p;
}

export function getActivePairing(): ActivePairing | null {
  return active;
}

export function markPairedBefore(): void {
  if (active) active = { ...active, pairedBefore: true };
}

export function clearActivePairing(): void {
  active = null;
}
