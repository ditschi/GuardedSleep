<script lang="ts">
  import { getI18n } from "$lib/i18n/context.js";
  import {
    clearStoredSignalingUrl,
    getEffectiveSignalingUrl,
    setStoredSignalingUrl,
    signalingUrlDraftInitial,
    validateSignalingUrl,
  } from "$lib/signaling-url.js";

  let { onChanged }: { onChanged?: () => void } = $props();

  const i18n = getI18n();

  const SERVER_MSG_KEYS: Record<string, string> = {
    "URL must not be empty": "signaling.validationEmpty",
    "Only ws:// or wss:// is allowed": "signaling.validationProtocol",
    "Invalid URL": "signaling.validationInvalid",
  };

  function localizeValidation(msg: string | null): string | null {
    if (msg == null) return null;
    const key = SERVER_MSG_KEYS[msg];
    if (key && i18n) return i18n.t(key);
    return msg;
  }

  let draft = $state(signalingUrlDraftInitial());
  let error = $state<string | null>(null);
  let activeDisplay = $state(getEffectiveSignalingUrl());

  function refreshActive(): void {
    activeDisplay = getEffectiveSignalingUrl();
  }

  function save(): void {
    const msg = validateSignalingUrl(draft);
    error = localizeValidation(msg);
    if (msg) return;
    setStoredSignalingUrl(draft);
    refreshActive();
    onChanged?.();
  }

  function useBuildDefault(): void {
    clearStoredSignalingUrl();
    draft = signalingUrlDraftInitial();
    error = null;
    refreshActive();
    onChanged?.();
  }
</script>

<div class="flex flex-col gap-2 rounded-lg border border-zinc-700/80 bg-zinc-900/40 p-3">
  <label class="text-xs font-medium text-zinc-400" for="signaling-url-input"
    >{i18n?.t("signaling.editorLabel") ?? ""}</label
  >
  <div class="flex flex-col gap-2 sm:flex-row sm:items-center">
    <input
      id="signaling-url-input"
      type="url"
      autocomplete="off"
      spellcheck="false"
      class="min-w-0 flex-1 rounded border border-zinc-600 bg-zinc-950 px-2 py-1.5 font-mono text-xs text-zinc-100 placeholder:text-zinc-600 focus:border-emerald-600 focus:outline-none"
      bind:value={draft}
    />
    <div class="flex shrink-0 gap-2">
      <button
        type="button"
        class="rounded bg-emerald-700 px-3 py-1.5 text-xs font-medium text-white hover:bg-emerald-600"
        onclick={save}
      >
        {i18n?.t("signaling.save") ?? ""}
      </button>
      <button
        type="button"
        class="rounded border border-zinc-600 px-3 py-1.5 text-xs text-zinc-300 hover:bg-zinc-800"
        onclick={useBuildDefault}
        title={i18n?.t("signaling.useDefaultTitle") ?? ""}
      >
        {i18n?.t("signaling.useDefault") ?? ""}
      </button>
    </div>
  </div>
  {#if error}
    <p class="text-xs text-amber-400">{error}</p>
  {/if}
  <p class="text-[11px] text-zinc-500">
    {i18n?.t("signaling.activePrefix") ?? ""}
    <span class="font-mono text-zinc-400">{activeDisplay}</span>
  </p>
</div>
