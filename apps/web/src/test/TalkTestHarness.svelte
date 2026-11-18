<script lang="ts">
  import type { WebTalkState } from "../lib/contract";
  import TalkButton from "../lib/ui/TalkButton.svelte";
  import TalkStatus from "../lib/ui/TalkStatus.svelte";
  import type { TalkHint, TalkView } from "../lib/ui/talk-view";

  /** Test only: the talk tile and its status lines wired the way the monitor screen wires them. */
  type Props = {
    talk: TalkView;
    web: WebTalkState;
    nowMs: number;
    nowWallMs: number;
    onpress: () => void;
    onrelease: () => void;
    onkeeplisten: () => void;
  };

  let { talk, web, nowMs, nowWallMs, onpress, onrelease, onkeeplisten }: Props = $props();
  let hint = $state<TalkHint | null>(null);
</script>

<TalkButton {talk} {web} {nowMs} {nowWallMs} {onpress} {onrelease} bind:hint />
<TalkStatus {talk} {web} {nowMs} {hint} {onkeeplisten} />
