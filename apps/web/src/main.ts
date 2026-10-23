import { mount } from "svelte";
import { registerSW } from "virtual:pwa-register";
import App from "./App.svelte";

const target = document.getElementById("app");
if (!target) throw new Error("missing #app mount point");
mount(App, { target });

// autoUpdate: the new worker takes over on the next load; nothing is shown to the user.
registerSW({ immediate: true });
