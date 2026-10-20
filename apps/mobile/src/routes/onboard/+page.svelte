<script lang="ts">
  import { browser } from "$app/environment";
  import { goto } from "$app/navigation";
  import { onMount } from "svelte";
  import { persistAppPreferences } from "$lib/app-preferences.js";
  import IndicatorTrainingDemo from "$lib/components/IndicatorTrainingDemo.svelte";
  import { getI18n } from "$lib/i18n/context.js";
  import { pushBackInterceptor } from "$lib/ui/app-navigation.js";

  type Step = "welcome" | "training" | "done";

  const i18n = getI18n();

  let step = $state<Step>("welcome");
  let trainingPhase = $state<"connected" | "disconnected">("connected");
  let trainingTimer: number | undefined;

  function finish(): void {
    persistAppPreferences({ onboardingCompleted: true });
    void goto("/");
  }

  function startTraining(): void {
    step = "training";
    trainingPhase = "connected";
    if (!browser) return;
    trainingTimer = window.setInterval(() => {
      trainingPhase = trainingPhase === "connected" ? "disconnected" : "connected";
    }, 3500);
  }

  function stopTrainingTimer(): void {
    if (trainingTimer != null) {
      clearInterval(trainingTimer);
      trainingTimer = undefined;
    }
  }

  function nextFromTraining(): void {
    stopTrainingTimer();
    step = "done";
  }

  function goBackStep(): void {
    if (step === "done") {
      step = "training";
      trainingPhase = "connected";
      return;
    }
    if (step === "training") {
      stopTrainingTimer();
      step = "welcome";
    }
  }

  $effect(() => {
    return () => stopTrainingTimer();
  });

  onMount(() => {
    if (!browser) return;
    return pushBackInterceptor(() => {
      if (step === "welcome") return false;
      goBackStep();
      return true;
    });
  });
</script>

<div class="mx-auto flex max-w-md flex-col gap-6 px-4 py-8">
  <header>
    {#if step !== "welcome"}
      <button
        type="button"
        class="mb-3 inline-flex h-11 w-11 shrink-0 items-center justify-center rounded-lg text-zinc-400 ring-emerald-600/40 hover:bg-zinc-800 hover:text-zinc-200 focus:outline-none focus:ring-2"
        aria-label={i18n?.t("onboarding.back") ?? ""}
        data-testid="onboard-back"
        onclick={goBackStep}
      >
        <svg
          class="h-6 w-6"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          aria-hidden="true"
        >
          <path stroke-linecap="round" stroke-linejoin="round" d="M15.75 19.5 8.25 12l7.5-7.5" />
        </svg>
      </button>
    {/if}
    <p class="text-xs font-medium uppercase tracking-wide text-emerald-500/90">
      {i18n?.t("onboarding.kicker") ?? ""}
    </p>
    <h1 class="mt-1 text-xl font-semibold text-zinc-50">{i18n?.t("onboarding.title") ?? ""}</h1>
  </header>

  {#if step === "welcome"}
    <section
      class="space-y-4 rounded-xl border border-zinc-800 bg-zinc-900/40 p-4 text-sm text-zinc-300"
    >
      <p>{i18n?.t("onboarding.welcomeLead") ?? ""}</p>
      <div
        class="rounded-lg border border-amber-800/60 bg-amber-950/30 px-3 py-3 text-amber-100/95"
        role="note"
      >
        <p class="font-medium">{i18n?.t("onboarding.failureModelTitle") ?? ""}</p>
        <p class="mt-1 text-xs text-amber-200/85">{i18n?.t("onboarding.failureModelBody") ?? ""}</p>
      </div>
    </section>
    <button
      type="button"
      class="rounded-lg bg-emerald-600 px-4 py-3 font-medium text-emerald-950 hover:bg-emerald-500"
      data-testid="onboard-next"
      onclick={startTraining}
    >
      {i18n?.t("onboarding.continue") ?? ""}
    </button>
  {:else if step === "training"}
    <section class="space-y-3">
      <p class="text-sm text-zinc-400">{i18n?.t("onboarding.trainingIntro") ?? ""}</p>
      <IndicatorTrainingDemo phase={trainingPhase} />
    </section>
    <button
      type="button"
      class="rounded-lg bg-emerald-600 px-4 py-3 font-medium text-emerald-950 hover:bg-emerald-500"
      data-testid="onboard-training-done"
      onclick={nextFromTraining}
    >
      {i18n?.t("onboarding.trainingDone") ?? ""}
    </button>
  {:else}
    <section
      class="space-y-3 rounded-xl border border-zinc-800 bg-zinc-900/40 p-4 text-sm text-zinc-300"
    >
      <p>{i18n?.t("onboarding.doneLead") ?? ""}</p>
      <p class="text-xs text-zinc-500">{i18n?.t("onboarding.probeHint") ?? ""}</p>
    </section>
    <button
      type="button"
      class="rounded-lg bg-emerald-600 px-4 py-3 font-medium text-emerald-950 hover:bg-emerald-500"
      data-testid="onboard-finish"
      onclick={finish}
    >
      {i18n?.t("onboarding.finish") ?? ""}
    </button>
  {/if}

  {#if step !== "done"}
    <button
      type="button"
      class="text-center text-xs text-zinc-600 underline hover:text-zinc-400"
      onclick={finish}
    >
      {i18n?.t("onboarding.skip") ?? ""}
    </button>
  {/if}
</div>
