<script lang="ts">
  import { lineDiff } from "../lib/util";
  import { t } from "../lib/i18n.svelte";
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { editors } from "../lib/editor/registry";
  import type { Version } from "../lib/types";
  import { escapeHtml, fmtDate } from "../lib/util";
  import Modal from "./Modal.svelte";

  const path = app.historyFor!;
  let versions = $state<Version[]>([]);
  let selected = $state<string | null>(null);
  let content = $state("");
  let current = $state("");
  let showDiff = $state(true);

  $effect(() => {
    api.historyList(path).then((v) => {
      versions = v;
      if (v[0]) select(v[0].id);
    });
    api.readNote(path).then((f) => (current = editors.byPath(path)?.view.state.doc.toString() ?? f.content));
  });

  async function select(id: string) {
    selected = id;
    content = await api.historyRead(path, id);
  }

  async function restore() {
    const ok = await app.confirm(t("Restaurer cette version ?"), t("Le texte actuel sera conservé dans l'historique avant d'être remplacé."), false, t("Restaurer"));
    if (!ok) return;
    await editors.savePath(path);
    await api.historySnapshot(path, current);
    await api.saveNote(path, content);
    editors.pushContent(path, content);
    app.toast(t("Version restaurée"), "success");
    app.historyFor = null;
  }
</script>

<Modal title={t("Historique · {name}", { name: path.replace(/\.md$/, "") })} width={1000} onclose={() => (app.historyFor = null)}>
  <div class="hist">
    <aside>
      {#each versions as v (v.id)}
        <button class:on={selected === v.id} onclick={() => select(v.id)}>
          <span>{fmtDate(v.timestamp)}</span>
          <span class="faint">{t("{n} ko", { n: (v.size / 1024).toFixed(1) })}</span>
        </button>
      {:else}
        <p class="empty">{t("Pas encore de version. Une version est enregistrée au plus toutes les {n} minutes pendant l'écriture.", { n: Math.round((app.settings?.history_interval ?? 300) / 60) })}</p>
      {/each}
    </aside>
    <div class="view">
      {#if selected}
        <div class="bar">
          <label><input type="checkbox" bind:checked={showDiff} /> {t("Comparer avec le texte actuel")}</label>
          {#if showDiff}<span class="legend"><span class="l-del">{t("supprimé depuis")}</span> <span class="l-add">{t("ajouté depuis")}</span></span>{/if}
        </div>
        <div class="text">
          {#if showDiff}{@html lineDiff(content, current)}{:else}<pre>{content}</pre>{/if}
        </div>
      {/if}
    </div>
  </div>
  {#snippet footer()}
    <button class="btn" onclick={() => (app.historyFor = null)}>{t("Fermer")}</button>
    <button class="btn primary" disabled={!selected} onclick={restore}>{t("Restaurer cette version")}</button>
  {/snippet}
</Modal>

<style>
  .hist {
    display: grid;
    grid-template-columns: 220px 1fr;
    height: 62vh;
  }
  aside {
    border-right: 1px solid var(--line);
    overflow: auto;
    padding: 6px;
  }
  aside button {
    display: flex;
    flex-direction: column;
    width: 100%;
    border: 0;
    background: transparent;
    text-align: left;
    padding: 7px 10px;
    border-radius: 6px;
  }
  aside button.on {
    background: var(--accent-soft);
  }
  .view {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .bar {
    display: flex;
    gap: 16px;
    align-items: center;
    padding: 8px 14px;
    border-bottom: 1px solid var(--line);
    font-size: 12.5px;
  }
  .legend {
    display: flex;
    gap: 8px;
  }
  .l-del,
  .l-add {
    padding: 0 6px;
    border-radius: 3px;
  }
  .l-del,
  .text :global(.del) {
    background: color-mix(in srgb, var(--danger) 14%, transparent);
  }
  .l-add,
  .text :global(.add) {
    background: color-mix(in srgb, var(--success) 16%, transparent);
  }
  .text :global(.del) {
    text-decoration: line-through;
    text-decoration-color: color-mix(in srgb, var(--danger) 50%, transparent);
  }
  .text {
    flex: 1;
    overflow: auto;
    padding: 12px 18px;
    font-family: var(--font-mono);
    font-size: 12.5px;
    line-height: 1.6;
    white-space: pre-wrap;
    user-select: text;
  }
  pre {
    margin: 0;
    font: inherit;
    white-space: pre-wrap;
  }
</style>
