/** Icon paths on a 24x24 grid, drawn with a 2 px round stroke (see `.gs-glyph`). */
export const GLYPHS = {
  check: "M5 12l5 5L20 7",
  link: "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1",
  linkOff:
    "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1M4 4l16 16",
  refresh: "M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7",
  alert:
    "M12 9v4M12 17h.01M10.3 4.3L2.5 18a2 2 0 0 0 1.7 3h15.6a2 2 0 0 0 1.7-3L13.7 4.3a2 2 0 0 0-3.4 0z",
  pause: "M8 5v14M16 5v14",
  snooze: "M4 6h6L4 14h6M13 10h7l-7 8h7",
  info: "M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20zM12 16v-4M12 8h.01",
  close: "M6 6l12 12M18 6L6 18",
  battery: "M3 8h15v8H3zM18 11h3v2h-3",
  mic: "M12 15a3 3 0 0 0 3-3V6a3 3 0 0 0-6 0v6a3 3 0 0 0 3 3zM6 11a6 6 0 0 0 12 0M12 17v4",
} as const;

export type GlyphName = keyof typeof GLYPHS;

/** The tones shared by the status bar, chip and banner. */
export type Tone = "neutral" | "ok" | "warn" | "orange" | "bad" | "snooze";

/** Default icon per tone, so colour is never the only signal. */
export const TONE_GLYPH: Record<Tone, GlyphName> = {
  neutral: "pause",
  ok: "link",
  warn: "refresh",
  orange: "linkOff",
  bad: "alert",
  snooze: "snooze",
};
