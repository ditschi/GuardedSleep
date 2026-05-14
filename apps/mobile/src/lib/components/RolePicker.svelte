<script lang="ts">
  import { tWithEnFallback } from "$lib/i18n/fallback-t";

  type Role = "baby" | "parent";

  let { onPick }: { onPick: (role: Role) => void } = $props();

  const title = $derived(tWithEnFallback("meta.appTitle"));
  const subtitle = $derived(tWithEnFallback("rolePicker.subtitle"));
  const emphasis = $derived(tWithEnFallback("rolePicker.instructionEmphasis"));
  const instructionRest = $derived(tWithEnFallback("rolePicker.instructionRest"));
  const babyTitle = $derived(tWithEnFallback("rolePicker.babyTitle"));
  const babyHint = $derived(tWithEnFallback("rolePicker.babyHint"));
  const parentTitle = $derived(tWithEnFallback("rolePicker.parentTitle"));
  const parentHint = $derived(tWithEnFallback("rolePicker.parentHint"));
</script>

<div class="mx-auto flex max-w-md flex-col gap-6 px-4 py-10">
  <header class="space-y-2 text-center">
    <h1 class="text-2xl font-semibold tracking-tight text-zinc-50">{title}</h1>
    <p class="text-sm text-zinc-400">{subtitle}</p>
    <p class="text-sm text-zinc-500">
      <strong class="font-medium text-zinc-300">{emphasis}</strong>
      {" "}{instructionRest}
    </p>
  </header>

  <div class="grid gap-3">
    <button
      type="button"
      class="rounded-xl border border-zinc-800 bg-zinc-900/60 px-4 py-4 text-left text-base font-medium text-zinc-100 shadow-sm transition hover:border-emerald-700/60 hover:bg-zinc-900"
      data-testid="role-baby"
      onclick={() => onPick("baby")}
    >
      {babyTitle}
      <span class="mt-1 block text-sm font-normal text-zinc-500">{babyHint}</span>
    </button>
    <button
      type="button"
      class="rounded-xl border border-zinc-800 bg-zinc-900/60 px-4 py-4 text-left text-base font-medium text-zinc-100 shadow-sm transition hover:border-emerald-700/60 hover:bg-zinc-900"
      data-testid="role-parent"
      onclick={() => onPick("parent")}
    >
      {parentTitle}
      <span class="mt-1 block text-sm font-normal text-zinc-500">{parentHint}</span>
    </button>
  </div>
</div>
