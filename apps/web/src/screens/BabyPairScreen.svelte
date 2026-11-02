<script lang="ts">
  import { onMount } from "svelte";
  import { useController } from "../lib/contract";
  import { t } from "../lib/i18n/store";
  import { useRouter } from "../lib/router-context";
  import Banner from "../lib/ui/Banner.svelte";
  import Button from "../lib/ui/Button.svelte";
  import { formatCountdown, remainingMs } from "../lib/ui/format";
  import Glyph from "../lib/ui/Glyph.svelte";
  import { babyFailureKey } from "../lib/ui/messages";
  import QrCode from "../lib/ui/QrCode.svelte";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";
  import { everySecond } from "../lib/ui/ticker";

  const controller = useController();
  const router = useRouter();
  const { pairing, baby } = controller;

  let bright = $state(false);
  let copied = $state(false);

  // The page holds the code in memory only. Opening this screen (also by its address) makes this
  // device the baby device and starts a fresh code; leaving it ends the pairing run.
  onMount(() => {
    controller.chooseRole("baby");
    controller.startPairing();
    return () => controller.cancelPairing();
  });

  // The other device paired: the same link carries on as the monitoring run.
  $effect(() => {
    if ($pairing?.phase === "paired") router.navigate("/baby");
  });

  const left = $derived(remainingMs($pairing?.expiresAtElapsedMs ?? null, $everySecond.elapsedMs));

  // A run that failed before it could pair names its real reason (the microphone), not "unreachable".
  const failure = $derived(
    $t(babyFailureKey($baby.link.failureKey) ?? "native.ui.pair.baby.failed"),
  );

  async function copyCode(): Promise<void> {
    const text = $pairing?.codePayload;
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
    } catch {
      // Clipboard access can be refused (permissions policy, no secure context); the QR remains.
      copied = false;
    }
  }

  // "Copied" reverts after a moment.
  $effect(() => {
    if (!copied) return;
    const id = setTimeout(() => (copied = false), 2500);
    return () => clearTimeout(id);
  });

  function newCode(): void {
    copied = false;
    controller.refreshPairingCode();
  }
</script>

<div class="gs-screen gs-screen--center" data-screen="baby_pair">
  <ScreenHeader onback={() => router.navigate("/")} backTestid="baby_pair_back" />

  <h1 tabindex="-1" data-route-heading data-testid="baby_pair_title" class="gs-align-start">
    {$t("native.ui.pair.baby.title")}
  </h1>

  {#if $pairing?.phase === "showing_code" && $pairing.codePayload}
    <figure class="gs-figure">
      <QrCode
        text={$pairing.codePayload}
        label={$t("native.ui.pair.baby.qrAlt")}
        {bright}
        testid="baby_pair_qr"
      />
      {#if left !== null}
        <figcaption data-testid="baby_pair_countdown">
          {$t("native.ui.pair.baby.validFor", { duration: formatCountdown(left) })}
        </figcaption>
      {/if}
    </figure>

    <p class="gs-align-start" data-testid="baby_pair_instruction">
      {$t("native.ui.pair.baby.instruction")}
    </p>

    <div class="gs-stack gs-stack--tight gs-fill-width">
      <div class="gs-button-row">
        <Button variant="outline" testid="baby_pair_copy" onclick={copyCode}>
          <Glyph name={copied ? "check" : "copy"} />
          {copied ? $t("native.ui.pair.baby.copied") : $t("native.ui.pair.baby.copy")}
        </Button>
        <Button
          variant="outline"
          testid="baby_pair_brighter"
          pressed={bright}
          onclick={() => (bright = !bright)}
        >
          <Glyph name="sun" />
          {$t("native.ui.pair.baby.brighter")}
        </Button>
      </div>
      <p class="gs-hint" data-testid="baby_pair_copy_hint">{$t("webScreens.pair.copyHint")}</p>
    </div>

    <p class="gs-waiting" role="status" data-testid="baby_pair_waiting">
      <span class="gs-dot" aria-hidden="true"></span>
      {$t("native.ui.pair.baby.waiting")}
    </p>
  {:else if $pairing?.phase === "expired"}
    <div class="gs-stack gs-fill-width">
      <Banner tone="warn" message={$t("native.ui.pair.baby.expired")} testid="baby_pair_expired" />
      <Button size="large" block testid="baby_pair_new_code" onclick={newCode}>
        {$t("native.ui.pair.baby.newCode")}
      </Button>
    </div>
  {:else if $pairing?.phase === "failed"}
    <div class="gs-stack gs-fill-width">
      <Banner tone="bad" message={failure} testid="baby_pair_error" />
      <Button size="large" block testid="baby_pair_new_code" onclick={newCode}>
        {$t("native.ui.pair.baby.newCode")}
      </Button>
    </div>
  {/if}

  <div class="gs-fill"></div>

  <p class="gs-note" data-testid="baby_pair_privacy">
    <Glyph name="shield" />
    <span>{$t("native.ui.pair.baby.privacy")}</span>
  </p>
</div>
