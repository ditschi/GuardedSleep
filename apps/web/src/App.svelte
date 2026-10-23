<script lang="ts">
  import { onDestroy } from "svelte";
  import { locale, t } from "./lib/i18n/store";
  import { createRouter, focusRouteHeading, formatDocumentTitle, type RouteId } from "./lib/router";

  const router = createRouter(window);
  const route = router.route;
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

  $effect(() => {
    document.documentElement.lang = $locale;
  });
</script>

<a class="gs-skip" href="#main">{$t("webShell.skipToContent")}</a>
<main id="main" bind:this={main}>
  <!-- Screens (groups 5 and 6) replace this placeholder heading with their own. -->
  <h1 tabindex="-1" data-route-heading>{$t($route.titleKey)}</h1>
</main>
