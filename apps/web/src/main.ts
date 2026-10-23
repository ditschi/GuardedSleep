import { mount } from "svelte";
import { registerSW } from "virtual:pwa-register";
import App from "./App.svelte";
import { createWebApp } from "./lib/platform";
import "./lib/ui/theme.css";
import "./lib/ui/components.css";

const target = document.getElementById("app");
if (!target) throw new Error("missing #app mount point");

// The platform layer owns the WASM core, the link driver and the audio; the UI gets the
// controller interface only. The in-memory fake stays in tests.
void createWebApp().then((controller) => {
  mount(App, { target, props: { controller } });
});

// autoUpdate: the new worker takes over on the next load; nothing is shown to the user.
registerSW({ immediate: true });
