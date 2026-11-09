<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, translateError } from "../lib/i18n.svelte";
  import type { Work } from "../lib/types";
  import Modal from "./Modal.svelte";
  import Icon from "./Icon.svelte";

  // Eloria's library: an article enters the vault as a source, without Zotero.
  // Metadata comes from Crossref; nothing is typed in or made up by the AI.
  let query = $state("");
  let results = $state<Work[] | null>(null);
  let busy = $state(false);
  let error = $state("");
  let input = $state<HTMLInputElement>();
  $effect(() => input?.focus());

  async function search() {
    if (!query.trim()) return;
    busy = true;
    error = "";
    try {
      results = await api.searchWorks(query.trim());
    } catch (e) {
      error = translateError(e);
      results = null;
    } finally {
      busy = false;
    }
  }

  async function add(w: Work) {
    const r = await app.addReference($state.snapshot(w) as Work);
    if (r) app.addRefOpen = false;
  }

  let dropping = $state(false);
  async function onDrop(e: DragEvent) {
    e.preventDefault();
    dropping = false;
    const files = [...(e.dataTransfer?.files ?? [])];
    if (!files.length) return;
    app.addRefOpen = false;
    if (!(await app.dropPdfs(files))) app.toast(t("Dépose un fichier PDF"), "info");
  }
  async function pickPdf() {
    app.addRefOpen = false;
    await app.pickPdf();
  }

  const who = (w: Work) => (w.authors.length > 2 ? `${w.authors[0][1]} et al.` : w.authors.map((a) => a[1]).join(" & "));
  const scholar = (title: string) => `https://scholar.google.com/scholar?q=${encodeURIComponent(title)}`;
</script>

<Modal title={t("Ajouter une référence")} width={720} onclose={() => (app.addRefOpen = false)}>
  <div
    class="body"
    class:dropping
    role="region"
    aria-label={t("Ajouter une référence")}
    ondragover={(e) => { if (e.dataTransfer?.types.includes("Files")) { e.preventDefault(); dropping = true; } }}
    ondragleave={() => (dropping = false)}
    ondrop={onDrop}
  >
    <form class="row" onsubmit={(e) => { e.preventDefault(); search(); }}>
      <input bind:this={input} bind:value={query} placeholder={t("DOI, lien doi.org, ou titre et auteurs")} aria-label={t("DOI ou titre")} />
      <button class="btn primary" type="submit" disabled={busy || !query.trim()}>{busy ? t("Recherche…") : t("Chercher")}</button>
    </form>
    <p class="hint">{t("Métadonnées de Crossref, complétées par OpenAlex et Semantic Scholar (résumé, mots-clés). L'article devient une source du coffre, avec la même vue que les documents Zotero ; tu peux ensuite y joindre des fiches.")}</p>
    <button class="pdfzone" onclick={pickPdf}>
      <Icon name="pdf" size={18} />
      <span><strong>{t("Ajouter un PDF…")}</strong> {t("ou dépose-le ici. Le DOI est lu dans le fichier ; sans DOI, le nom du fichier sert de titre.")}</span>
    </button>
    {#if error}<p class="error">{error}</p>{/if}
    {#if results}
      <div class="list">
        {#each results as w (w.doi || w.title)}
          <div class="work">
            <div class="w-title">{w.title}</div>
            <div class="w-meta">
              {who(w)}{w.year ? ` (${w.year})` : ""}{w.venue ? ` · ${w.venue}` : ""}{w.doi ? ` · ${w.doi}` : ""}
            </div>
            {#if w.abstract_note}<p class="w-abs">{w.abstract_note}</p>{/if}
            {#if w.keywords?.length}<div class="w-kw">{#each w.keywords as k (k)}<span>{k}</span>{/each}</div>{/if}
            <div class="w-actions">
              <button class="btn small primary" onclick={() => add(w)}>{t("Ajouter à ma bibliothèque")}</button>
              {#if w.url}<button class="btn small ghost" onclick={() => api.openUrl(w.url)}>{t("Ouvrir dans le navigateur")}</button>{/if}
              <button class="btn small ghost" onclick={() => api.openUrl(scholar(w.title))}>Google Scholar</button>
            </div>
          </div>
        {:else}
          <p class="faint">{t("Aucun résultat. Essaie le DOI, ou moins de mots.")}</p>
        {/each}
      </div>
    {/if}
  </div>
  {#snippet footer()}
    <button class="btn" onclick={() => api.openUrl(scholar(query))} disabled={!query.trim()}>{t("Chercher sur Google Scholar")}</button>
    <button class="btn" onclick={() => (app.addRefOpen = false)}>{t("Fermer")}</button>
  {/snippet}
</Modal>

<style>
  .body {
    padding: 16px 22px;
  }
  .row {
    display: flex;
    gap: 8px;
  }
  .row input {
    flex: 1;
  }
  .hint {
    margin: 8px 0 0;
    font-size: 12.5px;
    color: var(--text-soft);
  }
  .error {
    color: var(--danger);
  }
  .faint {
    color: var(--text-faint);
  }
  .list {
    margin-top: 12px;
    max-height: 55vh;
    overflow: auto;
  }
  .work {
    padding: 10px 0;
    border-bottom: 1px solid var(--line);
  }
  .w-title {
    font-weight: 600;
  }
  .w-meta {
    font-size: 12.5px;
    color: var(--text-soft);
    margin: 2px 0 6px;
  }
  .w-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .w-abs {
    margin: 0 0 6px;
    font-size: 12.5px;
    color: var(--text-soft);
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .w-kw {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-bottom: 6px;
  }
  .w-kw span {
    padding: 0 7px;
    border-radius: 999px;
    background: var(--bg-sunk);
    font-size: 11.5px;
    color: var(--text-soft);
  }
  .pdfzone {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    margin-top: 10px;
    padding: 10px 12px;
    border: 1px dashed var(--line-strong, var(--line));
    border-radius: 8px;
    background: none;
    color: var(--text-soft);
    font: 12.5px var(--font-ui);
    text-align: left;
    cursor: pointer;
  }
  .pdfzone:hover,
  .dropping .pdfzone {
    border-color: var(--accent);
    color: var(--text);
    background: var(--accent-soft, var(--bg-sunk));
  }
  .pdfzone strong {
    color: var(--text);
  }
</style>
