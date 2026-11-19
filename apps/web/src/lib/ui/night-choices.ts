import type { MessageKey } from "../i18n/keys";
import type { NightMode } from "../night-mode";

/** The three night mode choices with their copy, shared by Settings and the monitor's More sheet. */
export const NIGHT_MODE_CHOICES: ReadonlyArray<{ value: NightMode; label: MessageKey }> = [
  { value: "off", label: "native.ui.settings.night.off" },
  { value: "on", label: "native.ui.settings.night.on" },
  { value: "auto", label: "native.ui.settings.night.auto" },
];
