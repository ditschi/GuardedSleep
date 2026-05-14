<script lang="ts">
  import { onMount } from "svelte";
  import { browser } from "$app/environment";
  import { getI18n } from "$lib/i18n/context.js";
  import { isRuntimeSecureContext, isWebPlatformNotNative } from "$lib/runtime/capabilities.js";

  const LS_INSECURE = "guardedsleep:banner-dismiss:insecure-context";
  const LS_WEB = "guardedsleep:banner-dismiss:web-vs-native";

  const i18n = getI18n();

  const insecure = $derived(browser && !isRuntimeSecureContext());
  const webOnly = $derived(browser && isWebPlatformNotNative() && isRuntimeSecureContext());

  let dismissedInsecure = $state(false);
  let dismissedWeb = $state(false);

  onMount(() => {
    if (!browser) return;
    dismissedInsecure = localStorage.getItem(LS_INSECURE) === "1";
    dismissedWeb = localStorage.getItem(LS_WEB) === "1";
  });

  function dismissInsecure(): void {
    if (browser) localStorage.setItem(LS_INSECURE, "1");
    dismissedInsecure = true;
  }

  function dismissWeb(): void {
    if (browser) localStorage.setItem(LS_WEB, "1");
    dismissedWeb = true;
  }
</script>

{#if insecure && !dismissedInsecure}
  <div
    class="flex items-start gap-2 border-b border-amber-900/60 bg-amber-950/40 px-2 py-2 text-xs text-amber-100/95"
    role="status"
  >
    <p class="min-w-0 flex-1 pt-0.5">{i18n?.t("capabilities.secureContextBabyMicHint") ?? ""}</p>
    <button
      type="button"
      class="shrink-0 rounded px-2 py-1 text-amber-200/90 hover:bg-amber-900/40 hover:text-amber-50"
      aria-label={i18n?.t("runtime.bannerDismissAria") ?? "Dismiss"}
      onclick={dismissInsecure}
    >
      ×
    </button>
  </div>
{/if}

{#if webOnly && !dismissedWeb}
  <div
    class="flex items-start gap-2 border-b border-zinc-800 bg-zinc-900/50 px-2 py-2 text-xs text-zinc-400"
    role="status"
  >
    <p class="min-w-0 flex-1 pt-0.5">{i18n?.t("capabilities.webVersusNativeHint") ?? ""}</p>
    <button
      type="button"
      class="shrink-0 rounded px-2 py-1 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200"
      aria-label={i18n?.t("runtime.bannerDismissAria") ?? "Dismiss"}
      onclick={dismissWeb}
    >
      ×
    </button>
  </div>
{/if}
