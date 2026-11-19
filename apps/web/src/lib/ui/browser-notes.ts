import { writable } from "svelte/store";

/**
 * The browser parent put the plain "keep this page open" note away. Page memory only (it is not a
 * setting and survives screen changes, not a reload), so a full-screen alert that replaces the
 * monitor for a while does not bring the note back.
 */
export const keepOpenDismissed = writable(false);
