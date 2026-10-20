<script lang="ts">
  import QRCode from "qrcode";

  type Props = {
    payload: string;
    alt: string;
    expired?: boolean;
    bright?: boolean;
  };
  let { payload, alt, expired = false, bright = false }: Props = $props();

  let dataUrl = $state("");

  $effect(() => {
    const text = payload;
    let cancelled = false;
    // Inverted (light modules on near-black) so the baby unit's screen stays dark; scanners that
    // support inversion read it, and the copyable text is the fallback.
    void QRCode.toDataURL(text, {
      margin: 2,
      width: 280,
      errorCorrectionLevel: "M",
      color: { dark: bright ? "#f5f5f5" : "#a3a3a3", light: "#050505" },
    }).then((url) => {
      if (!cancelled) dataUrl = url;
    });
    return () => {
      cancelled = true;
    };
  });
</script>

{#if dataUrl}
  <img
    src={dataUrl}
    {alt}
    width="280"
    height="280"
    data-testid="baby_pair_qr"
    class={`mx-auto rounded-lg transition-opacity ${expired ? "opacity-20" : ""}`}
  />
{/if}
