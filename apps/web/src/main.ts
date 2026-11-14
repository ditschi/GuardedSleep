import { mount } from "svelte";
import App from "./App.svelte";
import { createWebApp, createWebScanner, elapsedNow } from "./lib/platform";
import { registerServiceWorker, type TrustedTypesFactory } from "./lib/service-worker";
import { readAlarmToneSetting } from "./lib/alarm-tone";
import { readThemeSetting } from "./lib/theme";
import { setElapsedSource } from "./lib/ui/ticker";
import "./lib/ui/theme.css";
import "./lib/ui/components.css";
import "./lib/ui/screens.css";

const target = document.getElementById("app");
if (!target) throw new Error("missing #app mount point");

// The platform layer owns the WASM core, the link driver, the audio and the camera; the UI gets
// the controller and scanner interfaces only. The in-memory fakes stay in tests.
void createWebApp().then((controller) => {
  setElapsedSource(elapsedNow);
  // The theme is no secret and is the one setting that survives a reload (theme-boot.js reads it
  // before first paint); the settings in memory start from it so the page does not flip back.
  const theme = readThemeSetting(localStorage);
  if (theme) controller.updateSettings((s) => ({ ...s, theme }));
  // Likewise the connection alarm tone; the controller resolves it through the core, so an unknown
  // stored value becomes the default tone.
  const tone = readAlarmToneSetting(localStorage);
  if (tone !== null) controller.updateSettings((s) => ({ ...s, connectionAlarmTone: tone }));
  mount(App, { target, props: { controller, scanner: createWebScanner() } });
});

// autoUpdate: the new worker takes over on the next load; nothing is shown to the user. Only the
// production build has a worker (`devOptions.enabled` is off in vite.config.ts).
if (import.meta.env.PROD) {
  void registerServiceWorker(
    navigator,
    (window as unknown as { trustedTypes?: TrustedTypesFactory }).trustedTypes,
    window.location.origin,
  );
}
