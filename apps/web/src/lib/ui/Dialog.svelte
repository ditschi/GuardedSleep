<script lang="ts">
  import type { Snippet } from "svelte";

  type Props = {
    open: boolean;
    title: string;
    testid?: string;
    /** Called when the dialog closes by Escape or backdrop; `open` is set to false first. */
    onclose?: () => void;
    children: Snippet;
    actions?: Snippet;
  };

  let { open = $bindable(), title, testid, onclose, children, actions }: Props = $props();

  let dialog: HTMLDialogElement | undefined = $state();
  const titleId = $props.id();

  // The native <dialog> owns focus trapping, Escape and inertness of the page behind it.
  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    else if (!open && dialog.open) dialog.close();
  });

  function handleClose(): void {
    if (!open) return;
    open = false;
    onclose?.();
  }

  function handleBackdropClick(event: MouseEvent): void {
    // A click on the dialog box itself targets the <dialog>; content clicks target children.
    if (event.target === dialog) dialog?.close();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -- the backdrop click is a pointer convenience; Escape and the action buttons are the keyboard path -->
<dialog
  bind:this={dialog}
  class="gs-dialog"
  data-testid={testid}
  aria-labelledby={titleId}
  onclose={handleClose}
  onclick={handleBackdropClick}
>
  <h2 id={titleId}>{title}</h2>
  <div>{@render children()}</div>
  {#if actions}<div class="gs-dialog__actions">{@render actions()}</div>{/if}
</dialog>
