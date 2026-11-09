<script lang="ts">
  import { t } from "../lib/i18n.svelte";
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let { title, onclose, width = 640, children, footer }: { title: string; onclose: () => void; width?: number; children: Snippet; footer?: Snippet } = $props();
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<div class="backdrop" role="presentation" onmousedown={onclose}>
  <div class="modal" role="dialog" aria-modal="true" aria-label={title} tabindex="-1" style:width="min({width}px, 94vw)" onmousedown={(e) => e.stopPropagation()}>
    <header>
      <h2>{title}</h2>
      <button class="icon-btn" aria-label={t("Fermer")} onclick={onclose}><Icon name="x" /></button>
    </header>
    <div class="body">{@render children()}</div>
    {#if footer}<footer>{@render footer()}</footer>{/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 150;
    background: rgb(20 15 30 / 24%);
    display: grid;
    place-items: center;
  }
  .modal {
    max-height: 86vh;
    background: var(--bg-raised);
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    box-shadow: var(--shadow);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    padding: 14px 14px 10px 20px;
    border-bottom: 1px solid var(--line);
  }
  h2 {
    flex: 1;
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .body {
    overflow: auto;
    flex: 1;
    min-height: 0;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 16px;
    border-top: 1px solid var(--line);
  }
</style>
