<script lang="ts">
  import { goto } from "$app/navigation";
  import { onMount } from "svelte";
  import InviteScanner from "$lib/components/InviteScanner.svelte";
  import { getI18n } from "$lib/i18n/context.js";
  import { setActivePairing } from "$lib/pairing/pairing-memory.js";
  import { parsePairingCode } from "$lib/pairing/pairing-v3.js";
  import type { PairingError } from "$lib/link/ui-contract.js";

  const i18n = getI18n();
  const t = (key: string): string => i18n?.t(key) ?? key;

  let pasted = $state("");
  let error: PairingError | null = $state(null);
  let connecting = $state(false);
  let pasteEl: HTMLTextAreaElement | undefined = $state();

  onMount(() => {
    // The baby unit has no pairing screen of its own: it shows its code on its monitoring page.
    const role = new URLSearchParams(location.search).get("role");
    if (role === "baby") void goto("/baby", { replaceState: true });
  });

  const ERROR_KEYS: Record<PairingError, string> = {
    camera_denied: "web.pair.error.cameraDenied",
    not_our_code: "web.pair.error.notOurs",
    expired: "web.pair.error.expired",
    key_mismatch: "web.pair.error.mismatch",
    already_paired: "web.pair.error.alreadyPaired",
    unreachable: "web.pair.error.unreachable",
    incomplete_paste: "web.pair.error.incomplete",
  };

  /** Returns true when the code is good and pairing material was stored (in memory only). */
  function accept(text: string): boolean {
    const r = parsePairingCode(text, Date.now());
    if (!r.ok) {
      error = r.error;
      return false;
    }
    error = null;
    connecting = true;
    setActivePairing({
      ...r.material,
      pairedBefore: false,
      codePayload: null,
      codeExpiresAtWallMs: null,
    });
    void goto("/parent");
    return true;
  }
</script>

<div class="mx-auto flex max-w-md flex-col gap-4 px-4 py-6">
  <h1 class="text-xl font-semibold text-zinc-50" data-testid="parent_pair_title">
    {t("web.pair.parent.title")}
  </h1>
  <p class="text-xs text-zinc-500" data-testid="parent_pair_camera_why">
    {t("web.pair.parent.cameraWhy")}
  </p>

  <div data-testid="parent_pair_scanner">
    <InviteScanner onText={accept} onCameraDenied={() => (error = "camera_denied")} />
  </div>

  {#if error === "camera_denied"}
    <div
      class="rounded-lg bg-amber-950/40 p-3 text-sm text-amber-100"
      data-testid="parent_pair_camera_denied"
    >
      <p>{t(ERROR_KEYS.camera_denied)}</p>
      <button
        type="button"
        class="mt-2 min-h-12 rounded-lg bg-white/10 px-4"
        data-testid="parent_pair_paste_instead"
        onclick={() => pasteEl?.focus()}>{t("web.pair.error.pasteInstead")}</button
      >
    </div>
  {/if}

  <label class="flex flex-col gap-2 text-sm text-zinc-300">
    {t("web.pair.parent.pasteLabel")}
    <textarea
      bind:this={pasteEl}
      bind:value={pasted}
      rows="4"
      class="rounded-lg border border-zinc-700 bg-zinc-900 p-2 font-mono text-xs"
      data-testid="parent_pair_paste_field"
      onkeydown={(e) => {
        if (e.key === "Enter" && !e.shiftKey) {
          e.preventDefault();
          accept(pasted);
        }
      }}
    ></textarea>
  </label>
  <button
    type="button"
    class="min-h-14 rounded-xl bg-emerald-600 px-4 font-semibold text-emerald-950 hover:bg-emerald-500"
    data-testid="parent_pair_paste_submit"
    onclick={() => accept(pasted)}
  >
    {t("web.pair.parent.pasteSubmit")}
  </button>

  {#if error && error !== "camera_denied"}
    <p
      class="rounded-lg bg-red-950 p-3 text-sm text-red-100"
      role="alert"
      data-testid="parent_pair_error"
      data-error={error}
    >
      {t(ERROR_KEYS[error])}
    </p>
    <button
      type="button"
      class="min-h-12 rounded-lg border border-zinc-600 px-4 text-sm"
      data-testid="parent_pair_retry"
      onclick={() => {
        error = null;
        pasted = "";
      }}>{t("web.common.tryAgain")}</button
    >
  {/if}
  {#if connecting}
    <p class="text-sm text-zinc-400" data-testid="parent_pair_connecting">
      {t("web.pair.parent.connecting")}
    </p>
  {/if}
  <p class="text-xs text-zinc-500" data-testid="parent_pair_memory_note">
    {t("web.pair.memoryNote")}
  </p>
</div>
