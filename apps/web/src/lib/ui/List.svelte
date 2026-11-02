<script lang="ts">
  import type { Snippet } from "svelte";

  // `role="list"` keeps the semantics in Safari, which drops them for style-reset lists.
  let { children, testid }: { children: Snippet; testid?: string } = $props();
</script>

<ul class="gs-list" role="list" data-testid={testid}>
  {@render children()}
</ul>
