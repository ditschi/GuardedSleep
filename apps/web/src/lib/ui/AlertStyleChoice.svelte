<script lang="ts">
  import type { AlertStyle } from "../contract";
  import { useController } from "../contract";
  import { t } from "../i18n/store";

  /** The two alert styles, with the ids shared with Android; used in Settings and More. */
  const controller = useController();
  const { settings } = controller;

  function choose(alertStyle: AlertStyle): void {
    controller.updateSettings((s) => ({ ...s, alertStyle }));
  }
</script>

<fieldset class="gs-group" data-testid="settings_alert_style">
  <legend>{$t("native.ui.settings.alertStyle.label")}</legend>
  <label class="gs-choice">
    <input
      type="radio"
      name="alert-style"
      checked={$settings.alertStyle === "sound"}
      data-testid="alert_style_sound"
      onchange={() => choose("sound")}
    />
    <span>{$t("native.ui.settings.alertStyle.sound")}</span>
  </label>
  <label class="gs-choice">
    <input
      type="radio"
      name="alert-style"
      checked={$settings.alertStyle === "discreet"}
      data-testid="alert_style_discreet"
      onchange={() => choose("discreet")}
    />
    <span>{$t("native.ui.settings.alertStyle.discreet")}</span>
  </label>
</fieldset>
