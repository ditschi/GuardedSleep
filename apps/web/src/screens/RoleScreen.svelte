<script lang="ts">
  import { useController, type Role } from "../lib/contract";
  import { t } from "../lib/i18n/store";
  import { link } from "../lib/router";
  import { useRouter } from "../lib/router-context";
  import Glyph from "../lib/ui/Glyph.svelte";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";

  const controller = useController();
  const router = useRouter();

  // The role is set before the route changes, so the pairing screen already sees it. Modified
  // clicks (new tab) keep the browser's behaviour.
  function choose(event: MouseEvent, role: Role, path: string): void {
    if (event.defaultPrevented || event.button !== 0) return;
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    event.preventDefault();
    controller.chooseRole(role);
    router.navigate(path);
  }
</script>

<div class="gs-screen" data-screen="role">
  <ScreenHeader>
    {#snippet actions()}
      <a
        class="gs-icon-btn"
        href="/settings"
        use:link={router}
        data-testid="home_settings"
        aria-label={$t("layout.settingsAria")}
      >
        <Glyph name="settings" />
      </a>
    {/snippet}
  </ScreenHeader>

  <div class="gs-stack gs-stack--tight">
    <h1 tabindex="-1" data-route-heading data-testid="role_title">{$t("webScreens.role.title")}</h1>
    <p class="gs-muted" data-testid="role_order_hint">{$t("webScreens.role.subtitle")}</p>
  </div>

  <nav class="gs-stack" aria-label={$t("webScreens.role.navLabel")}>
    <a
      class="gs-role-card"
      data-role="baby"
      href="/baby/pair"
      data-testid="role_baby"
      onclick={(event) => choose(event, "baby", "/baby/pair")}
    >
      <span class="gs-role-card__icon"><Glyph name="moon" /></span>
      <span class="gs-role-card__text">
        <span class="gs-role-card__title">{$t("native.ui.role.baby.title")}</span>
        <span class="gs-muted">{$t("native.ui.role.baby.hint")}</span>
      </span>
    </a>
    <a
      class="gs-role-card"
      data-role="parent"
      href="/parent/pair"
      data-testid="role_parent"
      onclick={(event) => choose(event, "parent", "/parent/pair")}
    >
      <span class="gs-role-card__icon"><Glyph name="bell" /></span>
      <span class="gs-role-card__text">
        <span class="gs-role-card__title">{$t("native.ui.role.parent.title")}</span>
        <span class="gs-muted">{$t("native.ui.role.parent.hint")}</span>
      </span>
    </a>
  </nav>

  <p class="gs-note gs-note--end" data-testid="role_privacy_note">
    <Glyph name="shield" />
    <span>{$t("webScreens.role.privacyNote")}</span>
  </p>
</div>
