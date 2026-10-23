<script lang="ts">
  import { useController, type Theme } from "../lib/contract";
  import { isLocale, LOCALES, type Locale } from "../lib/i18n/bundles";
  import type { MessageKey } from "../lib/i18n/keys";
  import { t } from "../lib/i18n/store";
  import { useRouter } from "../lib/router-context";
  import { storeThemeSetting } from "../lib/theme";
  import Banner from "../lib/ui/Banner.svelte";
  import Button from "../lib/ui/Button.svelte";
  import Dialog from "../lib/ui/Dialog.svelte";
  import { isSignalingUrl, sensitivityOf, thresholdOf } from "../lib/ui/sensitivity";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";
  import Switch from "../lib/ui/Switch.svelte";

  const controller = useController();
  const router = useRouter();
  const { app, settings } = controller;

  const THEMES: ReadonlyArray<{ value: Theme; label: MessageKey }> = [
    { value: "dark", label: "native.ui.settings.theme.dark" },
    { value: "light", label: "native.ui.settings.theme.light" },
    { value: "system", label: "native.ui.settings.theme.system" },
  ];
  const LANGUAGE_LABELS: Record<Locale, MessageKey> = {
    en: "native.ui.settings.lang.en",
    de: "native.ui.settings.lang.de",
    fr: "native.ui.settings.lang.fr",
    es: "native.ui.settings.lang.es",
  };

  function back(): void {
    const a = $app;
    router.navigate(a.monitoring && a.role !== null ? `/${a.role}` : "/");
  }

  // ---- screen ----------------------------------------------------------------------------

  function chooseTheme(theme: Theme): void {
    controller.updateSettings((s) => ({ ...s, theme }));
    storeThemeSetting(localStorage, theme);
  }

  function chooseLanguage(event: Event): void {
    const value = (event.currentTarget as HTMLSelectElement).value;
    controller.updateSettings((s) => ({ ...s, language: isLocale(value) ? value : null }));
  }

  // ---- sound detection (baby device) -----------------------------------------------------

  function chooseSensitivity(event: Event): void {
    const value = Number((event.currentTarget as HTMLInputElement).value);
    controller.updateSettings((s) => ({ ...s, threshold: thresholdOf(value) }));
  }

  // ---- advanced: signaling addresses -----------------------------------------------------

  let confirmAdvanced = $state(false);
  // A declined warning must put the switch back: a new key rebuilds it from the setting.
  let switchKey = $state(0);
  let newUrl = $state("");
  let urlInvalid = $state(false);

  function toggleAdvanced(on: boolean): void {
    if (on) confirmAdvanced = true;
    else controller.updateSettings((s) => ({ ...s, advancedEnabled: false }));
  }

  function declineAdvanced(): void {
    confirmAdvanced = false;
    switchKey++;
  }

  function showAdvanced(): void {
    confirmAdvanced = false;
    controller.updateSettings((s) => ({ ...s, advancedEnabled: true }));
  }

  function addUrl(event: SubmitEvent): void {
    event.preventDefault();
    const url = newUrl.trim();
    if (url === "") return;
    if (!isSignalingUrl(url)) {
      urlInvalid = true;
      return;
    }
    urlInvalid = false;
    newUrl = "";
    controller.updateSettings((s) =>
      s.signalingUrls.includes(url) ? s : { ...s, signalingUrls: [...s.signalingUrls, url] },
    );
  }

  function removeUrl(url: string): void {
    controller.updateSettings((s) => ({
      ...s,
      signalingUrls: s.signalingUrls.filter((u) => u !== url),
    }));
  }

  // ---- forget pairing --------------------------------------------------------------------

  let forgetOpen = $state(false);
  const forgetTitle = $derived.by(() => {
    const name = $app.peerName;
    if (name !== null) return $t("native.ui.forget.title", { name });
    return $app.role === "parent"
      ? $t("native.ui.forget.titleBabyDefault")
      : $t("native.ui.forget.titleParentDefault");
  });

  function forget(): void {
    forgetOpen = false;
    controller.forgetPairing();
    router.navigate("/");
  }
</script>

<div class="gs-screen" data-screen="settings">
  <ScreenHeader onback={back} backTestid="settings_back" />

  <h1 tabindex="-1" data-route-heading data-testid="settings_title">
    {$t("webShell.route.settings")}
  </h1>

  <fieldset class="gs-group" data-testid="settings_theme">
    <legend>{$t("native.ui.settings.group.screen")}</legend>
    {#each THEMES as theme (theme.value)}
      <label class="gs-choice">
        <input
          type="radio"
          name="theme"
          value={theme.value}
          checked={$settings.theme === theme.value}
          data-testid={`settings_theme_${theme.value}`}
          onchange={() => chooseTheme(theme.value)}
        />
        <span>{$t(theme.label)}</span>
      </label>
    {/each}
  </fieldset>

  <div class="gs-group">
    <label for="settings-language" class="gs-group__title">
      {$t("native.ui.settings.group.language")}
    </label>
    <select
      id="settings-language"
      class="gs-select"
      data-testid="settings_language"
      value={$settings.language ?? ""}
      onchange={chooseLanguage}
    >
      <option value="">{$t("native.ui.settings.lang.system")}</option>
      {#each LOCALES as code (code)}
        <option value={code}>{$t(LANGUAGE_LABELS[code])}</option>
      {/each}
    </select>
  </div>

  {#if $app.role === "baby"}
    <div class="gs-group" data-testid="settings_sound">
      <span class="gs-group__title">{$t("native.ui.settings.group.soundDetection")}</span>
      <label for="settings-sensitivity">{$t("native.ui.settings.sensitivity")}</label>
      <input
        id="settings-sensitivity"
        class="gs-range"
        type="range"
        min="0"
        max="100"
        step="5"
        value={sensitivityOf($settings.threshold)}
        data-testid="settings_sensitivity"
        onchange={chooseSensitivity}
      />
      <div class="gs-range-labels" aria-hidden="true">
        <span>{$t("native.ui.settings.lessSensitive")}</span>
        <span>{$t("native.ui.settings.moreSensitive")}</span>
      </div>
    </div>
  {:else if $app.role === "parent"}
    <p class="gs-muted" data-testid="settings_sensitivity_note">
      {$t("webScreens.settings.sensitivityBabyOnly")}
    </p>
  {/if}

  <div class="gs-group" data-testid="settings_advanced">
    {#key switchKey}
      <Switch
        label={$t("native.ui.settings.advanced.toggle")}
        checked={$settings.advancedEnabled}
        testid="settings_advanced_toggle"
        onchange={toggleAdvanced}
      />
    {/key}
    {#if $settings.advancedEnabled}
      <div class="gs-stack gs-stack--tight">
        <span class="gs-group__title">{$t("native.ui.settings.urlsLabel")}</span>
        <ul class="gs-stack gs-stack--tight" role="list" data-testid="settings_url_list">
          {#each $settings.signalingUrls as url, i (url)}
            <li class="gs-url-row">
              <span>{url}</span>
              <Button
                variant="text"
                testid={`settings_url_remove_${i}`}
                onclick={() => removeUrl(url)}
              >
                {$t("native.ui.common.remove")}
              </Button>
            </li>
          {/each}
        </ul>
        <form class="gs-stack gs-stack--tight" onsubmit={addUrl}>
          <label for="settings-url" class="gs-hint">{$t("native.ui.settings.url.new")}</label>
          <input
            id="settings-url"
            class="gs-input"
            type="text"
            inputmode="url"
            spellcheck="false"
            autocomplete="off"
            autocapitalize="off"
            data-testid="settings_url_input"
            bind:value={newUrl}
            oninput={() => (urlInvalid = false)}
          />
          {#if urlInvalid}
            <Banner
              tone="bad"
              message={$t("native.ui.settings.url.invalid")}
              testid="settings_url_error"
            />
          {/if}
          <Button type="submit" variant="tonal" testid="settings_url_add">
            {$t("native.ui.common.add")}
          </Button>
        </form>
        <p class="gs-hint">{$t("webScreens.settings.urlHint")}</p>
      </div>
    {/if}
  </div>

  {#if $app.paired}
    <Button variant="outline" block testid="settings_forget" onclick={() => (forgetOpen = true)}>
      {$app.role === "parent"
        ? $t("native.ui.settings.forgetBaby")
        : $t("native.ui.settings.forgetParent")}
    </Button>
  {/if}
</div>

<Dialog
  bind:open={confirmAdvanced}
  title={$t("native.ui.settings.advanced.warning.title")}
  testid="dialog_advanced"
  onclose={() => switchKey++}
>
  <p>{$t("native.ui.settings.advanced.warning.body")}</p>
  {#snippet actions()}
    <Button variant="text" testid="dialog_advanced_cancel" onclick={declineAdvanced}>
      {$t("native.ui.settings.advanced.cancel")}
    </Button>
    <Button testid="dialog_advanced_confirm" onclick={showAdvanced}>
      {$t("native.ui.settings.advanced.confirm")}
    </Button>
  {/snippet}
</Dialog>

<Dialog bind:open={forgetOpen} title={forgetTitle} testid="dialog_forget">
  <p>{$t("native.ui.forget.body")}</p>
  {#snippet actions()}
    <Button variant="text" testid="dialog_forget_cancel" onclick={() => (forgetOpen = false)}>
      {$t("native.ui.common.cancel")}
    </Button>
    <Button variant="danger" testid="dialog_forget_confirm" onclick={forget}>
      {$t("native.ui.forget.confirm")}
    </Button>
  {/snippet}
</Dialog>
