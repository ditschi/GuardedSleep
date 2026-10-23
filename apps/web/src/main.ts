import { mount } from "svelte";
import { registerSW } from "virtual:pwa-register";
import App from "./App.svelte";
import { createFakeController } from "./lib/contract/fake-controller";
import "./lib/ui/theme.css";
import "./lib/ui/components.css";

const target = document.getElementById("app");
if (!target) throw new Error("missing #app mount point");

// The in-memory controller stands in until the link driver (group 4) provides the real one.
mount(App, { target, props: { controller: createFakeController() } });

// autoUpdate: the new worker takes over on the next load; nothing is shown to the user.
registerSW({ immediate: true });
