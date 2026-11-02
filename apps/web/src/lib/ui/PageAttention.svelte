<script lang="ts">
  import { useController } from "../contract";
  import { t } from "../i18n/store";
  import { browserNotifications, notifyAway } from "./attention";

  const controller = useController();
  const { app, pageGuard } = controller;

  // A page that is not in front can be slowed down or frozen by the browser and cannot sound an
  // alert, so while this device monitors the user gets a system notification the moment the page
  // goes to the background (when they allowed notifications). It closes when the page returns.
  $effect(() => {
    if (!$app.monitoring || !$pageGuard.hidden) return;
    return notifyAway(browserNotifications(window), document, {
      title: $t("webScreens.guard.hiddenTitle"),
      body: $t("webScreens.guard.hiddenBody"),
      tag: "guardedsleep-hidden",
    });
  });
</script>
