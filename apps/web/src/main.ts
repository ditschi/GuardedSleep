import { mount } from "svelte";
import { registerSW } from "virtual:pwa-register";
import App from "./App.svelte";
import { createWebApp, createWebScanner } from "./lib/platform";
import "./lib/ui/theme.css";
import "./lib/ui/components.css";
import "./lib/ui/screens.css";

const target = document.getElementById("app");
if (!target) throw new Error("missing #app mount point");

// The platform layer owns the WASM core, the link driver, the audio and the camera; the UI gets
// the controller and scanner interfaces only. The in-memory fakes stay in tests.
void createWebApp().then((controller) => {
  mount(App, { target, props: { controller, scanner: createWebScanner() } });
});

// autoUpdate: the new worker takes over on the next load; nothing is shown to the user.
registerSW({ immediate: true });
