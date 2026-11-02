<script lang="ts">
  import type { Snippet } from "svelte";

  type Props = {
    tone?: "surface" | "variant" | "alert";
    testid?: string;
    children: Snippet;
  };

  let { tone = "surface", testid, children }: Props = $props();
</script>

<section class="gs-card" data-tone={tone} data-testid={testid}>
  {@render children()}
</section>
