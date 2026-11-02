<script lang="ts">
  import { useController } from "../contract";
  import { t } from "../i18n/store";
  import Banner from "./Banner.svelte";
  import Button from "./Button.svelte";

  const controller = useController();
  const { pageGuard } = controller;
</script>

<!-- Both monitoring screens: the browser let go of the screen wake lock (or has none). -->
{#if $pageGuard.wakeLock === "lost" || $pageGuard.wakeLock === "unsupported"}
  <div
    class="gs-stack gs-stack--tight"
    data-testid="wake_lock_warning"
    data-state={$pageGuard.wakeLock}
  >
    <Banner
      tone="warn"
      message={$t(
        $pageGuard.wakeLock === "lost"
          ? "webScreens.guard.wakeLockLost"
          : "webScreens.guard.wakeLockUnsupported",
      )}
    />
    {#if $pageGuard.wakeLock === "lost"}
      <Button variant="tonal" testid="wake_lock_retry" onclick={() => controller.retryKeepAwake()}>
        {$t("webScreens.guard.wakeLockRetry")}
      </Button>
    {/if}
  </div>
{/if}
