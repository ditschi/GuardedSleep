<script lang="ts">
  import { onMount, tick } from "svelte";
  import { ScanError, useController, useScanner } from "../lib/contract";
  import { t } from "../lib/i18n/store";
  import { useRouter } from "../lib/router-context";
  import Banner from "../lib/ui/Banner.svelte";
  import Button from "../lib/ui/Button.svelte";
  import { askForNotifications, browserNotifications } from "../lib/ui/attention";
  import Glyph from "../lib/ui/Glyph.svelte";
  import { pairErrorKey } from "../lib/ui/messages";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";

  const controller = useController();
  const scanner = useScanner();
  const router = useRouter();
  const { pairing } = controller;

  /** The camera opens only after "Allow camera" (the browser prompt follows a clear reason). */
  type Camera = "rationale" | "starting" | "scanning" | "denied" | "unavailable";
  let camera = $state<Camera>(scanner ? "rationale" : "unavailable");
  let video: HTMLVideoElement | undefined = $state();
  let field: HTMLTextAreaElement | undefined = $state();
  let pasted = $state("");

  onMount(() => {
    controller.chooseRole("parent");
    return () => {
      scanner?.stop();
      controller.cancelPairing();
    };
  });

  // The baby device answered and the link verified: the same link carries on as monitoring.
  $effect(() => {
    if ($pairing?.phase === "paired") router.navigate("/parent");
  });

  // A pasted code leaves the field as soon as it is accepted.
  $effect(() => {
    if ($pairing?.phase === "connecting") pasted = "";
  });

  // Setup is the moment to ask for system notifications ("this page went to the background");
  // nothing changes if the user declines.
  function askNotifications(): void {
    void askForNotifications(browserNotifications(window));
  }

  async function allowCamera(): Promise<void> {
    if (!scanner) return;
    askNotifications();
    camera = "starting";
    await tick();
    if (!video) return;
    try {
      await scanner.start(video, (text) => {
        camera = "rationale";
        controller.submitPairingCode(text);
      });
      if (camera === "starting") camera = "scanning";
    } catch (error) {
      camera = error instanceof ScanError && error.kind === "denied" ? "denied" : "unavailable";
    }
  }

  function submitPasted(event: SubmitEvent): void {
    event.preventDefault();
    if (pasted.trim() === "") return;
    scanner?.stop();
    askNotifications();
    controller.submitPairingCode(pasted);
  }

  function retry(): void {
    controller.cancelPairing();
    if (scanner && camera !== "unavailable") void allowCamera();
  }

  const showCamera = $derived(camera === "starting" || camera === "scanning");
  const connecting = $derived($pairing?.phase === "connecting");
  const error = $derived($pairing?.phase === "failed" ? ($pairing?.error ?? null) : null);
</script>

<div class="gs-screen" data-screen="parent_pair">
  <ScreenHeader onback={() => router.navigate("/")} backTestid="parent_pair_back" />

  <h1 tabindex="-1" data-route-heading data-testid="parent_pair_title">
    {$t("native.ui.pair.parent.title")}
  </h1>

  <section class="gs-camera" data-camera={camera} data-testid="parent_pair_scanner">
    {#if showCamera}
      <!-- svelte-ignore a11y_media_has_caption -- a live camera preview has no audio track and no captions -->
      <video
        bind:this={video}
        class="gs-camera__video"
        playsinline
        muted
        aria-label={$t("webScreens.pair.cameraView")}
      ></video>
      <div class="gs-camera__frame" aria-hidden="true"></div>
    {:else if camera === "rationale"}
      <Button testid="parent_pair_allow_camera" onclick={allowCamera}>
        <Glyph name="camera" />
        {$t("native.ui.pair.error.allowCamera")}
      </Button>
      <p class="gs-hint" data-testid="parent_pair_camera_why">
        {$t("native.ui.pair.parent.cameraWhy")}
      </p>
    {:else if camera === "denied"}
      <p role="alert" data-testid="parent_pair_camera_denied">
        {$t("native.ui.pair.error.cameraDenied")}
      </p>
      <div class="gs-button-row">
        <Button testid="parent_pair_allow_camera" onclick={allowCamera}>
          <Glyph name="camera" />
          {$t("native.ui.pair.error.allowCamera")}
        </Button>
        <Button variant="outline" testid="parent_pair_paste_instead" onclick={() => field?.focus()}>
          {$t("native.ui.pair.error.pasteInstead")}
        </Button>
      </div>
    {:else}
      <p data-testid="parent_pair_camera_unavailable">{$t("webScreens.pair.cameraUnavailable")}</p>
    {/if}
  </section>

  <div class="gs-divider" aria-hidden="true">{$t("webScreens.pair.or")}</div>

  <form class="gs-stack gs-stack--tight" onsubmit={submitPasted}>
    <label for="parent-pair-paste">{$t("native.ui.pair.parent.pasteLabel")}</label>
    <textarea
      id="parent-pair-paste"
      class="gs-input"
      rows="2"
      spellcheck="false"
      autocomplete="off"
      autocapitalize="off"
      placeholder={'{"v":3,…}'}
      data-testid="parent_pair_paste_field"
      bind:this={field}
      bind:value={pasted}
    ></textarea>
    <Button
      type="submit"
      size="large"
      block
      testid="parent_pair_paste_submit"
      disabled={pasted.trim() === "" || connecting}
    >
      {$t("native.ui.pair.parent.pasteSubmit")}
    </Button>
  </form>

  {#if connecting}
    <p class="gs-waiting" role="status" data-testid="parent_pair_connecting">
      <span class="gs-dot" aria-hidden="true"></span>
      {$t("native.ui.pair.parent.connecting")}
    </p>
    {#if $pairing?.error === "unreachable"}
      <Banner tone="warn" message={$t(pairErrorKey("unreachable"))} testid="parent_pair_error" />
    {/if}
  {:else if error}
    <div class="gs-stack gs-stack--tight">
      <Banner tone="bad" message={$t(pairErrorKey(error))} testid="parent_pair_error" />
      <Button variant="tonal" testid="parent_pair_retry" onclick={retry}>
        {$t("native.ui.common.tryAgain")}
      </Button>
    </div>
  {/if}
</div>
