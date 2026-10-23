<script lang="ts">
  import { useController } from "../contract";
  import { t } from "../i18n/store";
  import { formatDuration } from "./format";
  import Banner from "./Banner.svelte";

  const controller = useController();
  const { pageGuard } = controller;
</script>

<!-- Both monitoring screens: this page was in the background, or the device slept, and just came back. -->
{#if $pageGuard.away}
  <div data-testid="away_notice" data-kind={$pageGuard.away.kind}>
    <Banner
      tone="warn"
      message={$t(
        $pageGuard.away.kind === "sleep"
          ? "webScreens.guard.awaySleep"
          : "webScreens.guard.awayBackground",
        { duration: formatDuration($t, $pageGuard.away.awayMs) },
      )}
      dismissLabel={$t("webShell.dismiss")}
      onDismiss={() => controller.dismissAwayReport()}
    />
  </div>
{/if}
