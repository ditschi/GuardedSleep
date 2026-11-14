<script lang="ts">
  import { t } from "../i18n/store";
  import Button from "./Button.svelte";
  import Dialog from "./Dialog.svelte";

  /**
   * The stop confirmation of `device-setup` "Stopping monitoring on the parent": one dialog for the
   * More sheet, the quiet Stop button and "Wrong connection?" of the alert screens. Opening or
   * cancelling it does nothing else: it silences and acknowledges no alert, so a running alarm keeps
   * sounding underneath. "Keep monitoring" is the filled default; Escape and the backdrop keep
   * monitoring too.
   */
  type Props = {
    open: boolean;
    /** The connection name, when it has one. */
    name?: string | null;
    onconfirm: () => void;
    testid?: string;
    confirmTestid?: string;
    cancelTestid?: string;
  };

  let {
    open = $bindable(),
    name = null,
    onconfirm,
    testid = "dialog_stop",
    confirmTestid = "dialog_stop_confirm",
    cancelTestid = "dialog_stop_cancel",
  }: Props = $props();
</script>

<Dialog
  bind:open
  title={name ? $t("native.ui.stop.titleNamed", { name }) : $t("native.ui.stop.title")}
  {testid}
  status
>
  <p>
    {name ? $t("native.ui.stop.parentBody", { name }) : $t("native.ui.stop.parentBodyDefault")}
  </p>
  {#snippet actions()}
    <Button variant="danger" testid={confirmTestid} onclick={onconfirm}>
      {$t("native.ui.stop.confirm")}
    </Button>
    <Button testid={cancelTestid} onclick={() => (open = false)}>
      {$t("native.ui.stop.cancel")}
    </Button>
  {/snippet}
</Dialog>
