<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { t } from "../lib/i18n.svelte";
  import PaneView from "./PaneView.svelte";
  import Resizer from "./Resizer.svelte";

  // Two panes side by side: the split moves (as a share of the width).
  let host = $state<HTMLDivElement>();
  let width = $state(1000);
  $effect(() => {
    if (!host) return;
    const ro = new ResizeObserver(() => (width = host!.clientWidth));
    ro.observe(host);
    return () => ro.disconnect();
  });
</script>

<div class="workspace" bind:this={host}>
  {#each app.panes as pane, i (pane.id)}
    {#if i > 0}
      <Resizer
        value={app.splitRatio * width}
        min={width * 0.2}
        max={width * 0.8}
        defaultValue={width / 2}
        onchange={(v) => (app.splitRatio = v / width)}
        onend={() => app.saveLayout()}
        label={t("Partage entre les deux panneaux")}
      />
    {/if}
    <div class="slot" style:flex={app.panes.length > 1 ? (i === 0 ? app.splitRatio : 1 - app.splitRatio) : 1}>
      <PaneView {pane} index={i} />
    </div>
  {/each}
</div>

<style>
  .workspace {
    flex: 1;
    display: flex;
    min-width: 0;
  }
  .slot {
    display: flex;
    min-width: 0;
    flex-basis: 0;
  }
  .slot > :global(*) {
    flex: 1;
    min-width: 0;
  }
</style>
