<script lang="ts">
  type Props = {
    checked: boolean;
    label: string;
    disabled?: boolean;
    testid?: string;
    onchange?: (checked: boolean) => void;
  };

  let { checked = $bindable(), label, disabled = false, testid, onchange }: Props = $props();
</script>

<label class="gs-switch">
  <span>{label}</span>
  <input
    class="gs-switch__input"
    type="checkbox"
    role="switch"
    data-testid={testid}
    {disabled}
    bind:checked
    onchange={() => onchange?.(checked)}
  />
</label>
