<script lang="ts">
  import { onDestroy } from "svelte";
  import {
    provideController,
    provideScanner,
    type AppController,
    type CodeScanner,
  } from "./lib/contract";
  import { resolveLocale } from "./lib/i18n/bundles";
  import { locale, t } from "./lib/i18n/store";
  import PageAttention from "./lib/ui/PageAttention.svelte";
  import { createRouter, focusRouteHeading, formatDocumentTitle, type RouteId } from "./lib/router";
  import { provideRouter } from "./lib/router-context";
  import { applyTheme, resolveTheme } from "./lib/theme";
  import BabyMonitorScreen from "./screens/BabyMonitorScreen.svelte";
  import BabyPairScreen from "./screens/BabyPairScreen.svelte";
  import ParentMonitorScreen from "./screens/ParentMonitorScreen.svelte";
  import ParentPairScreen from "./screens/ParentPairScreen.svelte";
  import RoleScreen from "./screens/RoleScreen.svelte";
  import SettingsScreen from "./screens/SettingsScreen.svelte";

  let { controller, scanner }: { controller: AppController; scanner?: CodeScanner } = $props();

  // The controller never changes for the lifetime of the page, so it is read once.
  // svelte-ignore state_referenced_locally -- the controller prop is fixed for the page's life
  provideController(controller);
  // svelte-ignore state_referenced_locally -- the controller prop is fixed for the page's life
  const { app, settings } = controller;

  // svelte-ignore state_referenced_locally -- the scanner prop is fixed for the page's life
  provideScanner(scanner);

  const router = createRouter(window);
  const route = router.route;
  provideRouter(router);
  onDestroy(() => router.destroy());

  let main: HTMLElement | undefined = $state();

  // The tab title follows the route and the locale.
  $effect(() => {
    document.title = formatDocumentTitle($t($route.titleKey), $t("meta.appTitle"));
  });

  // Focus moves to the screen heading when the route changes, not on the first load (the browser's
  // own focus handling applies there) and not when only the locale changes.
  let lastRoute: RouteId | undefined;
  $effect(() => {
    const id = $route.id;
    if (lastRoute !== undefined && id !== lastRoute && main) focusRouteHeading(main);
    lastRoute = id;
  });

  // The monitoring screens only exist while this device monitors in that role. Pairing material
  // lives in memory, so a reload (or a stopped session) lands on the start screen.
  $effect(() => {
    const id = $route.id;
    const role = id === "baby" ? "baby" : id === "parent" ? "parent" : null;
    if (role !== null && !($app.role === role && $app.monitoring)) {
      router.navigate("/", { replace: true });
    }
  });

  // Settings drive the language (null = follow the browser) and the theme; the baby device
  // switches to the dim theme while it monitors.
  $effect(() => {
    locale.set(resolveLocale($settings.language, navigator.languages));
  });
  $effect(() => {
    const onBabyScreen = $route.id === "babyPair" || $route.id === "baby";
    const dim = $app.role === "baby" && ($app.monitoring || onBabyScreen);
    const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    applyTheme(document, resolveTheme($settings.theme, prefersDark, dim));
  });
  $effect(() => {
    document.documentElement.lang = $locale;
  });
</script>

<PageAttention />
<a class="gs-skip" href="#main">{$t("webShell.skipToContent")}</a>
<main id="main" bind:this={main}>
  {#if $route.id === "role"}
    <RoleScreen />
  {:else if $route.id === "babyPair"}
    <BabyPairScreen />
  {:else if $route.id === "baby"}
    <BabyMonitorScreen />
  {:else if $route.id === "parentPair"}
    <ParentPairScreen />
  {:else if $route.id === "parent"}
    <ParentMonitorScreen />
  {:else}
    <SettingsScreen />
  {/if}
</main>
