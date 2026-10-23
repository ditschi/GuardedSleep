<script lang="ts">
  import type { Snippet } from "svelte";

  type Props = {
    title: string;
    subtitle?: string;
    /** Makes the row a button. */
    onclick?: () => void;
    testid?: string;
    /** Trailing content: a value, a chip, a switch. */
    trailing?: Snippet;
  };

  let { title, subtitle, onclick, testid, trailing }: Props = $props();
</script>

{#snippet content()}
  <span class="gs-row__text">
    <span class="gs-row__title">{title}</span>
    {#if subtitle}<span class="gs-row__subtitle">{subtitle}</span>{/if}
  </span>
  {#if trailing}{@render trailing()}{/if}
{/snippet}

<li class="gs-list-item">
  {#if onclick}
    <button type="button" class="gs-row" data-testid={testid} {onclick}>{@render content()}</button>
  {:else}
    <div class="gs-row" data-testid={testid}>{@render content()}</div>
  {/if}
</li>
