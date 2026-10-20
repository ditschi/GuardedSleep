<!--
  IndicatorTrainingDemo — onboarding step showing real AmbientBar states.
-->
<script lang="ts">
  import AmbientBar from "$lib/components/AmbientBar.svelte";
  import { initialLinkUi, type LinkUi } from "$lib/link/ui-contract.js";
  import { getI18n } from "$lib/i18n/context.js";

  type DemoPhase = "connected" | "disconnected";

  type Props = {
    phase?: DemoPhase;
  };

  let { phase = "connected" }: Props = $props();

  const i18n = getI18n();

  const link = $derived<LinkUi>(
    phase === "connected"
      ? { ...initialLinkUi(), phase: "connected" }
      : { ...initialLinkUi(), phase: "reconnecting", level: 2, lastAliveElapsedMs: 0 },
  );
</script>

<div
  class="space-y-3 rounded-xl border border-zinc-800 bg-zinc-900/50 p-4"
  data-testid="indicator-training"
>
  <AmbientBar
    {link}
    role="parent"
    testid="indicator-training-bar"
    nowElapsedMs={phase === "connected" ? 0 : 75_000}
    startedElapsedMs={0}
  />
  <p class="text-sm text-zinc-300">
    {#if phase === "connected"}
      {i18n?.t("onboarding.trainingConnectedCopy") ?? ""}
    {:else}
      {i18n?.t("onboarding.trainingDisconnectedCopy") ?? ""}
    {/if}
  </p>
</div>
