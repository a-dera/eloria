<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { assetUrl } from "../lib/api";

  let { path }: { path: string } = $props();
  let src = $state("");

  $effect(() => {
    if (!app.vault) return;
    const sep = app.vault.root.includes("\\") ? "\\" : "/";
    assetUrl(app.vault.root + sep + path.split("/").join(sep)).then((u) => (src = u));
  });
</script>

<div class="img">
  {#if src}<img {src} alt={path} />{/if}
  <p class="faint">{path}</p>
</div>

<style>
  .img {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 24px;
    overflow: auto;
    gap: 10px;
  }
  img {
    max-width: 100%;
    max-height: 85%;
    border-radius: var(--radius);
    box-shadow: var(--shadow);
  }
</style>
