import { mount } from "svelte";
import { registerSW } from "virtual:pwa-register";
import App from "./App.svelte";
import { createWebApp, createWebScanner, elapsedNow } from "./lib/platform";
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
  mount(App, { target, props: { controller, scanner: createWebScanner() } });
});

// autoUpdate: the new worker takes over on the next load; nothing is shown to the user.
registerSW({ immediate: true });
