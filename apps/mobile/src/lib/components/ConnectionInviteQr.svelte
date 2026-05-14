<script lang="ts">
  import QRCode from "qrcode";

  let { inviteUrl }: { inviteUrl: string } = $props();

  let dataUrl = $state<string | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    if (!inviteUrl.trim()) {
      dataUrl = null;
      error = null;
      return;
    }
    let cancelled = false;
    QRCode.toDataURL(inviteUrl, { margin: 2, width: 280, errorCorrectionLevel: "M" })
      .then((url) => {
        if (!cancelled) {
          dataUrl = url;
          error = null;
        }
      })
      .catch(() => {
        if (!cancelled) {
          dataUrl = null;
          error = "Could not generate QR.";
        }
      });
    return () => {
      cancelled = true;
    };
  });
</script>

<div class="flex flex-col items-center gap-3 rounded-xl border border-zinc-700 bg-zinc-900/50 p-4">
  {#if error}
    <p class="text-sm text-amber-400">{error}</p>
  {:else if dataUrl}
    <img
      src={dataUrl}
      width="280"
      height="280"
      class="rounded-lg bg-white p-2"
      alt="Invitation QR code"
    />
  {:else}
    <div
      class="flex h-[280px] w-[280px] items-center justify-center rounded-lg bg-zinc-800 text-sm text-zinc-500"
    >
      …
    </div>
  {/if}
  <p class="max-w-[280px] break-all text-center text-[11px] text-zinc-500">{inviteUrl}</p>
</div>
