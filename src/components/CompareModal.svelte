<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t } from "../lib/i18n.svelte";
  import { lineDiff } from "../lib/util";
  import Modal from "./Modal.svelte";

  // Comparison of two documents (notes or LaTeX files): removed and added lines.
  const left = app.compareFor!;
  let right = $state("");
  let a = $state("");
  let b = $state("");
  const choices = $derived(app.notes.map((n) => n.path).filter((p) => p !== left).sort());

  $effect(() => {
    api.readNote(left).then((f) => (a = f.content)).catch(() => api.readText(left).then((x) => (a = x)));
  });
  $effect(() => {
    if (!right) return;
    api.readNote(right).then((f) => (b = f.content)).catch(() => api.readText(right).then((x) => (b = x)));
  });
  const stats = $derived.by(() => {
    if (!right) return null;
    const html = lineDiff(a, b);
    return { html, add: (html.match(/class="add"/g) ?? []).length, del: (html.match(/class="del"/g) ?? []).length };
  });
</script>

<Modal title={t("Comparer deux documents")} width={980} onclose={() => (app.compareFor = null)}>
  <div class="body">
    <div class="pick">
      <span class="name">{left}</span>
      <span class="faint">{t("avec")}</span>
      <select class="input" bind:value={right} aria-label={t("Document à comparer")}>
        <option value="">{t("Choisir un document…")}</option>
        {#each choices as c (c)}<option value={c}>{c}</option>{/each}
      </select>
    </div>
    {#if stats}
      <p class="sum"><span class="del-c">− {stats.del}</span> <span class="add-c">+ {stats.add}</span> {t("lignes")}</p>
      <div class="diff">{@html stats.html}</div>
    {:else}
      <p class="faint">{t("Choisis le second document : les lignes retirées apparaissent en rouge, les lignes ajoutées en vert.")}</p>
    {/if}
  </div>
  {#snippet footer()}
    <button class="btn" onclick={() => (app.compareFor = null)}>{t("Fermer")}</button>
  {/snippet}
</Modal>

<style>
  .body {
    padding: 14px 22px;
  }
  .pick {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 10px;
  }
  .name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 40%;
  }
  .pick select {
    flex: 1;
    min-width: 0;
  }
  .faint {
    color: var(--text-faint);
  }
  .sum {
    margin: 0 0 8px;
    font-size: 13px;
  }
  .del-c {
    color: var(--danger);
    font-weight: 600;
  }
  .add-c {
    color: var(--success, #2e7d32);
    font-weight: 600;
  }
  .diff {
    max-height: 60vh;
    overflow: auto;
    font: 12.5px/1.5 var(--font-mono);
    border: 1px solid var(--line);
    border-radius: var(--radius);
  }
  .diff :global(div) {
    padding: 0 10px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .diff :global(.add) {
    background: color-mix(in srgb, #2e9d4c 16%, transparent);
  }
  .diff :global(.del) {
    background: color-mix(in srgb, var(--danger) 14%, transparent);
    text-decoration: line-through;
    opacity: 0.85;
  }
</style>
