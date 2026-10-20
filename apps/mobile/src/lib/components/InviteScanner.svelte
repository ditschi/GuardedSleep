<script lang="ts">
  import { onDestroy, tick } from "svelte";
  import { decodeQrFromImageFile } from "$lib/pairing/decode-qr-image.js";
  import { isRuntimeSecureContext } from "$lib/runtime/capabilities.js";
  import { tWithEnFallback } from "$lib/i18n/fallback-t.js";

  type BarcodeDetectorCtor = new (opts: { formats: string[] }) => {
    detect: (source: HTMLVideoElement | ImageBitmap) => Promise<{ rawValue?: string }[]>;
  };

  /** `onText` gets every decoded QR text; returning true means "accepted, stop scanning". */
  let {
    onText,
    onCameraDenied,
  }: { onText: (text: string) => boolean; onCameraDenied?: () => void } = $props();

  let mode = $state<"idle" | "camera" | "error">("idle");
  let message = $state<string | null>(null);
  let videoEl = $state<HTMLVideoElement | null>(null);
  let stream = $state<MediaStream | null>(null);
  let raf = 0;
  /** ZXing continuous scan; BarcodeDetector path uses `raf` instead. */
  let zxingControls = $state<{ stop: () => void } | null>(null);

  function stopCamera(): void {
    if (raf) cancelAnimationFrame(raf);
    raf = 0;
    zxingControls?.stop();
    zxingControls = null;
    stream?.getTracks().forEach((t) => t.stop());
    stream = null;
    if (mode === "camera") mode = "idle";
  }

  async function getLiveCameraStream(): Promise<MediaStream> {
    const attempts: MediaStreamConstraints[] = [
      { video: { facingMode: { ideal: "environment" } }, audio: false },
      { video: { facingMode: "environment" }, audio: false },
      { video: { width: { ideal: 1280 }, height: { ideal: 720 } }, audio: false },
      { video: true, audio: false },
    ];
    let lastErr: unknown;
    for (const c of attempts) {
      try {
        return await navigator.mediaDevices.getUserMedia(c);
      } catch (e) {
        lastErr = e;
      }
    }
    throw lastErr;
  }

  function liveCameraErrorMessage(err: unknown): string {
    const name = err instanceof DOMException ? err.name : "";
    if (name === "NotAllowedError" || name === "PermissionDeniedError") {
      onCameraDenied?.();
      return tWithEnFallback("pair.scanner.cameraDenied");
    }
    if (name === "NotFoundError" || name === "OverconstrainedError") {
      return tWithEnFallback("pair.scanner.cameraNotFound");
    }
    if (name === "NotSupportedError" || name === "SecurityError") {
      return tWithEnFallback("pair.scanner.cameraNotSupported");
    }
    return tWithEnFallback("pair.scanner.cameraUnavailable");
  }

  async function tickBarcode(): Promise<void> {
    const BD = (globalThis as unknown as { BarcodeDetector?: BarcodeDetectorCtor }).BarcodeDetector;
    const v = videoEl;
    if (!BD || !v || v.readyState < 2) {
      raf = requestAnimationFrame(() => void tickBarcode());
      return;
    }
    try {
      const detector = new BD({ formats: ["qr_code"] });
      const codes = await detector.detect(v);
      for (const c of codes) {
        const raw = c.rawValue?.trim();
        if (!raw) continue;
        if (onText(raw)) {
          stopCamera();
          return;
        }
      }
    } catch {
      /* ignore frame */
    }
    raf = requestAnimationFrame(() => void tickBarcode());
  }

  async function startCamera(): Promise<void> {
    message = null;
    stopCamera();
    try {
      if (!navigator.mediaDevices?.getUserMedia) {
        mode = "error";
        message = tWithEnFallback("pair.scanner.liveCameraUnsupported");
        return;
      }
      stream = await getLiveCameraStream();
      mode = "camera";
      await tick();
      if (!videoEl) {
        mode = "error";
        message = tWithEnFallback("pair.scanner.previewNotReady");
        stopCamera();
        return;
      }

      const BD = (globalThis as unknown as { BarcodeDetector?: BarcodeDetectorCtor })
        .BarcodeDetector;
      if (BD) {
        videoEl.srcObject = stream;
        await videoEl.play();
        void tickBarcode();
        return;
      }

      const { BrowserMultiFormatReader, BarcodeFormat } = await import("@zxing/browser");
      const reader = new BrowserMultiFormatReader();
      reader.possibleFormats = [BarcodeFormat.QR_CODE];
      zxingControls = await reader.decodeFromStream(stream, videoEl, (result, _err, ctrls) => {
        if (!result) return;
        const raw = result.getText()?.trim();
        if (!raw) return;
        if (!onText(raw)) return;
        ctrls.stop();
        zxingControls = null;
        stream = null;
        mode = "idle";
      });
    } catch (e) {
      mode = "error";
      let msg = liveCameraErrorMessage(e);
      if (!isRuntimeSecureContext()) {
        msg += tWithEnFallback("pair.scanner.secureContextTip");
      }
      message = msg;
    }
  }

  async function onFilePick(ev: Event): Promise<void> {
    const input = ev.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = "";
    if (!file) return;
    message = null;
    try {
      const raw = (await decodeQrFromImageFile(file)).trim();
      if (!raw) {
        message = tWithEnFallback("pair.scanner.noQrInImage");
        return;
      }
      onText(raw);
    } catch {
      message = tWithEnFallback("pair.scanner.qrUnreadable");
    }
  }

  onDestroy(() => stopCamera());
</script>

<div class="flex flex-col gap-3">
  {#if mode === "camera" && stream}
    <video
      bind:this={videoEl}
      class="aspect-video w-full rounded-lg bg-black object-cover"
      playsinline
      muted
    ></video>
    <button
      type="button"
      class="rounded-lg border border-zinc-600 px-3 py-2 text-sm text-zinc-200 hover:bg-zinc-800"
      onclick={stopCamera}
    >
      {tWithEnFallback("pair.scanner.scanCancel")}
    </button>
  {:else}
    <div class="flex flex-wrap gap-2">
      <button
        type="button"
        class="rounded-lg bg-emerald-700 px-3 py-2 text-sm font-medium text-white hover:bg-emerald-600"
        onclick={() => void startCamera()}
      >
        {tWithEnFallback("pair.scanner.scanWithCamera")}
      </button>
      <label
        class="cursor-pointer rounded-lg border border-zinc-600 px-3 py-2 text-sm text-zinc-200 hover:bg-zinc-800"
      >
        {tWithEnFallback("pair.scanner.galleryOrFiles")}
        <input type="file" accept="image/*" class="sr-only" onchange={(e) => void onFilePick(e)} />
      </label>
    </div>
    <p class="text-[11px] text-zinc-600">
      {tWithEnFallback("pair.scanner.galleryHelp")}
    </p>
  {/if}
  {#if message}
    <p class="text-sm text-amber-400">{message}</p>
  {/if}
</div>
