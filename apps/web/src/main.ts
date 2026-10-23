import { mount } from "svelte";
import { registerSW } from "virtual:pwa-register";
import App from "./App.svelte";
import { resolveLocale } from "./lib/i18n/bundles";
import { locale } from "./lib/i18n/store";
import "./lib/ui/theme.css";
import "./lib/ui/components.css";

// Language follows the browser until the settings store (task 3.4) carries an explicit choice.
locale.set(resolveLocale(null, navigator.languages));

const target = document.getElementById("app");
if (!target) throw new Error("missing #app mount point");
mount(App, { target });

// autoUpdate: the new worker takes over on the next load; nothing is shown to the user.
registerSW({ immediate: true });
