<script lang="ts">
  import { goto } from "$app/navigation";
  import RolePicker from "$lib/components/RolePicker.svelte";
  import { listConnections, type SavedConnection } from "$lib/connection-store.js";
  import { tWithEnFallback } from "$lib/i18n/fallback-t";
  import { onMount } from "svelte";

  let connections = $state<SavedConnection[]>([]);

  onMount(() => {
    connections = listConnections();
  });

  const recentTitle = $derived(tWithEnFallback("home.recentTitle"));
  const recentHelp = $derived(tWithEnFallback("home.recentHelp"));
  const openAsBaby = $derived(tWithEnFallback("home.openAsBaby"));
  const openAsParent = $derived(tWithEnFallback("home.openAsParent"));
</script>

<div class="mx-auto max-w-md px-4 py-6">
  <RolePicker
    onPick={(role) => {
      goto(`/pair?role=${role}`);
    }}
  />

  {#if connections.length > 0}
    <section class="mt-10 space-y-3 border-t border-zinc-800 pt-8">
      <h2 class="text-sm font-medium text-zinc-400">{recentTitle}</h2>
      <p class="text-xs text-zinc-600">
        {recentHelp}
      </p>
      <ul class="space-y-3">
        {#each connections as c (c.id)}
          <li class="rounded-xl border border-zinc-800 bg-zinc-900/30 px-3 py-3">
            <p class="font-mono text-sm text-zinc-200">{c.roomId}</p>
            <p class="truncate text-xs text-zinc-600">{c.signalingUrl}</p>
            <div class="mt-2 flex gap-2">
              <button
                type="button"
                class="flex-1 rounded-lg bg-emerald-900/50 px-2 py-2 text-xs font-medium text-emerald-200 hover:bg-emerald-900"
                onclick={() =>
                  goto(
                    `/baby?room=${encodeURIComponent(c.roomId)}&signal=${encodeURIComponent(c.signalingUrl)}`,
                  )}
              >
                {openAsBaby}
              </button>
              <button
                type="button"
                class="flex-1 rounded-lg border border-zinc-700 px-2 py-2 text-xs text-zinc-300 hover:bg-zinc-800"
                onclick={() =>
                  goto(
                    `/parent?room=${encodeURIComponent(c.roomId)}&signal=${encodeURIComponent(c.signalingUrl)}`,
                  )}
              >
                {openAsParent}
              </button>
            </div>
          </li>
        {/each}
      </ul>
    </section>
  {/if}
</div>
