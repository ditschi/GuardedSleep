<script lang="ts">
  import { navigateAppBack } from "$lib/ui/app-navigation.js";

  let {
    ariaLabel = "Back",
    fallbackHref = "/",
    testId = "app-back",
  }: {
    ariaLabel?: string;
    fallbackHref?: string;
    testId?: string;
  } = $props();

  function onBack(): void {
    void navigateAppBack(fallbackHref);
  }
</script>

<button
  type="button"
  class="inline-flex h-11 w-11 shrink-0 items-center justify-center rounded-lg text-zinc-400 ring-emerald-600/40 hover:bg-zinc-800 hover:text-zinc-200 focus:outline-none focus:ring-2"
  aria-label={ariaLabel}
  data-testid={testId}
  onclick={onBack}
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
