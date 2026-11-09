<script lang="ts">
  import { t, tn, translateError } from "../lib/i18n.svelte";
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import type { SearchHit } from "../lib/types";
  import { debounce, snippetHtml } from "../lib/util";
  import Icon from "./Icon.svelte";

  let hits = $state<SearchHit[]>([]);
  /** (source, PDF, excerpt): the query found in the full text of PDFs. */
  let pdfHits = $state<[string, string, string][]>([]);
  let busy = $state(false);
  let annHits = $state<[import("../lib/types").ZAnnotation, string][]>([]);
  let scope = $state<"notes" | "sens" | "annotations">("notes");
  let semHits = $state<import("../lib/types").SemanticHit[]>([]);
  let semStatus = $state<import("../lib/types").SemanticStatus | null>(null);
  let semError = $state("");
  let indexing = $state<number | null>(null);

  async function loadSem() {
    semStatus = await api.semanticStatus().catch(() => null);
  }
  $effect(() => {
    // Also reloads when the embedding provider changes in the settings.
    void app.settings?.embed_provider;
    void app.settingsOpen;
    if (scope === "sens" && !app.settingsOpen) loadSem();
  });

  async function buildVectors() {
    indexing = 0;
    semError = "";
    try {
      await api.semanticIndex((n) => (indexing = n));
      await loadSem();
      run(app.searchQuery, scope);
    } catch (e) {
      semError = translateError(e);
    } finally {
      indexing = null;
    }
  }

  const run = debounce(async (q: string, s: typeof scope) => {
    if (!q.trim()) {
      hits = [];
      annHits = [];
      pdfHits = [];
      return;
    }
    busy = true;
    try {
      if (s === "notes") {
        const plain = q.replace(/["#]|(^|\s)-\S+/g, " ").trim();
        [hits, pdfHits] = await Promise.all([api.search(q, 80), plain.length > 2 ? api.searchPdfText(plain).catch(() => []) : Promise.resolve([] as [string, string, string][])]);
      }
      else if (s === "sens") {
        semError = "";
        semHits = semStatus?.configured ? await api.semanticSearch(q, 30) : [];
      } else annHits = await api.zoteroSearchAnnotations(q);
    } catch (e) {
      hits = [];
      if (s === "sens") semError = translateError(e);
    } finally {
      busy = false;
    }
  }, 160);

  $effect(() => {
    run(app.searchQuery, scope);
    void app.revision;
  });

  async function openAnnotation(citekey: string) {
    const path = app.fiches[citekey.toLowerCase()];
    if (path) app.openNote(path);
    else {
      const d = await api.zoteroItemByCitekey(citekey);
      if (d) app.openView("zotero-item", { key: d.item.key, title: `@${citekey}` });
    }
  }
</script>

<div class="panel-title"><span class="grow">{t("Recherche")}</span></div>
<div class="box">
  <div class="field">
    <Icon name="search" size={15} />
    <!-- svelte-ignore a11y_autofocus -->
    <input class="search-input" placeholder={scope === "notes" ? t("Mots, \"expression\", #tag, -exclure") : t("Texte surligné ou commentaire")} bind:value={app.searchQuery} autofocus />
    {#if app.searchQuery}<button class="icon-btn" aria-label={t("Effacer")} onclick={() => (app.searchQuery = "")}><Icon name="x" size={14} /></button>{/if}
  </div>
  <div class="scopes">
    <button class:on={scope === "notes"} onclick={() => (scope = "notes")}>{t("Notes")}</button>
    <button class:on={scope === "sens"} onclick={() => (scope = "sens")}>{t("Par le sens")}</button>
    <button class:on={scope === "annotations"} onclick={() => (scope = "annotations")}>{t("Annotations Zotero")}</button>
  </div>
</div>

<div class="results">
  {#if scope === "notes"}
    {#if app.searchQuery && !busy && !hits.length && !pdfHits.length}<p class="empty">{t("Aucun résultat.")}</p>{/if}
    {#if !app.searchQuery}
      <p class="empty">{t("La recherche ignore les accents et la casse. Le dernier mot est complété au fil de la frappe.")}</p>
    {/if}
    {#each hits as h (h.path)}
      <button class="hit" onclick={(e) => app.openNote(h.path, { newTab: true })}>
        <span class="title">{h.title}</span>
        <span class="path">{h.path.replace(/\.md$/, "")}</span>
        {#if h.snippet}<span class="snip">{@html snippetHtml(h.snippet)}</span>{/if}
      </button>
    {/each}
    {#if pdfHits.length}
      <div class="group">{t("Dans le texte des PDF")} <span class="score">{pdfHits.length}</span></div>
      {#each pdfHits as [src, pdf, snip] (pdf)}
        <button class="hit" onclick={() => app.openNote(src, { newTab: true })}>
          <span class="title"><Icon name="pdf" size={13} /> {app.noteByPath(src)?.title ?? src}</span>
          <span class="path">{pdf}</span>
          <span class="snip">… {snip} …</span>
        </button>
      {/each}
    {/if}
  {:else if scope === "sens"}
    {#if semStatus && !semStatus.configured}
      <p class="empty">{t("Recherche par le sens : choisis un fournisseur de vecteurs (Ollama sur ton ordinateur, Voyage AI ou OpenAI) dans Réglages › Intelligence artificielle.")}</p>
      <button class="btn small" onclick={() => app.openSettings("ia")}>{t("Ouvrir les réglages")}</button>
    {:else if semStatus}
      {#if semStatus.stale}
        <div class="sem-bar">
          <span>{tn(semStatus.stale, "{n} note à vectoriser", "{n} notes à vectoriser")}</span>
          <button class="btn small" disabled={indexing !== null || !semStatus.key_present} onclick={buildVectors}>
            {indexing !== null ? t("{n} faites…", { n: indexing }) : t("Calculer les vecteurs")}
          </button>
        </div>
        {#if !semStatus.key_present}<p class="empty">{t("Clé API du fournisseur de vecteurs manquante (Réglages › Intelligence artificielle).")}</p>{/if}
      {/if}
      {#if semError}<p class="empty err">{semError}</p>{/if}
      {#if !app.searchQuery}<p class="empty">{t("Décris une idée avec tes mots : les notes proches par le sens apparaissent, même sans le mot exact.")}</p>{/if}
      {#each semHits as h (h.path)}
        <button class="hit" onclick={() => app.openNote(h.path, { newTab: true })}>
          <span class="title">{h.title} <span class="score">{Math.round(h.score * 100)} %</span></span>
          <span class="path">{h.path.replace(/\.md$/, "")}</span>
        </button>
      {/each}
    {/if}
  {:else}
    {#if app.searchQuery && !busy && !annHits.length}<p class="empty">{t("Aucune annotation. Synchronise Zotero si besoin.")}</p>{/if}
    {#each annHits as [a, ck] (a.key)}
      <button class="hit" onclick={() => openAnnotation(ck)}>
        <span class="title"><span class="swatch" style:background={a.color || "#ffd400"}></span>@{ck} · {t("p.")} {a.page_label || "?"}</span>
        {#if a.text}<span class="snip quote">{a.text}</span>{/if}
        {#if a.comment}<span class="snip">{a.comment}</span>{/if}
      </button>
    {/each}
  {/if}
</div>

<style>
  .group {
    padding: 10px 12px 2px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-faint);
    text-transform: none;
  }
  .sem-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 6px 12px;
    font-size: 12.5px;
    color: var(--text-soft);
  }
  .score {
    font-weight: 400;
    font-size: 11px;
    color: var(--text-faint);
    margin-left: 4px;
  }
  .err {
    color: var(--danger);
  }
  .box {
    padding: 0 10px 8px;
  }
  .field {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 34px;
    padding: 0 6px 0 10px;
    border-radius: var(--radius);
    border: 1px solid var(--line-strong);
    background: var(--bg-raised);
    color: var(--text-faint);
  }
  .field:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  .field input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: none;
    background: transparent;
    color: var(--text);
    user-select: text;
  }
  .scopes {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 8px;
  }
  .scopes button {
    border: 0;
    background: transparent;
    padding: 3px 8px;
    border-radius: 20px;
    color: var(--text-soft);
    font-size: 12px;
    white-space: nowrap;
  }
  .scopes button.on {
    background: var(--accent-soft);
    color: var(--accent-text);
    font-weight: 600;
  }
  .results {
    flex: 1;
    overflow: auto;
    padding: 0 6px 20px;
  }
  .hit {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    text-align: left;
    padding: 8px 10px;
    border: 0;
    border-radius: 6px;
    background: transparent;
  }
  .hit:hover {
    background: var(--bg-sunk);
  }
  .title {
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .path {
    font-size: 11.5px;
    color: var(--text-faint);
  }
  .snip {
    font-size: 12.5px;
    color: var(--text-soft);
    line-height: 1.5;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .snip.quote {
    font-family: var(--font-editor);
    font-style: italic;
  }
  .swatch {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    display: inline-block;
  }
</style>
