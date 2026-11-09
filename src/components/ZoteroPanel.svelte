<script lang="ts">
  import { t, tn } from "../lib/i18n.svelte";
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { editors } from "../lib/editor/registry";
  import type { ItemSummary, ZCollection, ZoteroDetect } from "../lib/types";
  import { debounce, fmtDate } from "../lib/util";
  import Icon from "./Icon.svelte";

  let collections = $state<ZCollection[]>([]);
  let current = $state<string | null>(null);
  let query = $state("");
  let items = $state<ItemSummary[]>([]);
  let showColl = $state(true);
  let detect = $state<ZoteroDetect | null>(null);
  /** Only show items that do not have their note in the vault yet. */
  let onlyNew = $state(false);
  const shown = $derived(onlyNew ? items.filter((it) => !app.fiches[it.citekey.toLowerCase()]) : items);
  let syncing = $state(false);

  const load = debounce(async (c: string | null, q: string) => {
    try {
      items = await api.zoteroItems(c, q || null, 400);
    } catch {
      items = [];
    }
  }, 120);

  $effect(() => {
    void app.revision;
    void app.zoteroStatus;
    api.zoteroCollections().then((c) => (collections = c)).catch(() => {});
  });

  $effect(() => {
    void app.zoteroStatus;
    load(current, query);
  });

  $effect(() => {
    if (app.zoteroStatus && app.zoteroStatus.items === 0 && !detect) api.zoteroDetect().then((d) => (detect = d));
  });

  async function sync() {
    syncing = true;
    await app.zoteroSync();
    syncing = false;
  }

  function tree(parent: string | null, depth: number): [ZCollection, number][] {
    return collections
      .filter((c) => c.parent === parent)
      .flatMap((c) => [[c, depth] as [ZCollection, number], ...tree(c.key, depth + 1)]);
  }

  function cite(it: ItemSummary) {
    const ed = editors.active();
    if (!ed) return app.toast(t("Ouvre une note pour y insérer la citation"), "info");
    ed.insert(`[@${it.citekey}]`);
  }

  function itemMenu(e: MouseEvent, it: ItemSummary) {
    const fiche = app.fiches[it.citekey.toLowerCase()];
    app.openMenu(e, [
      { label: t("Créer une fiche"), icon: "file-plus", run: () => app.createFiche(it.key) },
      { label: fiche ? t("Ouvrir la source") : t("Importer dans le coffre"), icon: "note", run: () => (fiche ? app.openNote(fiche) : app.importSource(it.key)) },
      { label: t("Insérer la citation dans la note"), icon: "at", run: () => cite(it) },
      { label: t("Voir le détail et les annotations"), icon: "eye", run: () => openItem(it) },
      { label: t("Ouvrir dans Zotero"), icon: "external", run: () => api.openUrl(`zotero://select/library/items/${it.key}`) },
      { label: t("Copier la clé de citation"), icon: "link", run: () => navigator.clipboard.writeText(`@${it.citekey}`) },
    ]);
  }

  function importColl(c: ZCollection) {
    app.importCollection = { key: c.key, name: c.name };
    app.importOpen = true;
  }

  function openItem(it: ItemSummary, e?: MouseEvent) {
    app.openView("zotero-item", { key: it.key, title: `@${it.citekey}`, otherPane: e?.altKey });
  }

  const flat = $derived(tree(null, 0));
</script>

<div class="panel-title">
  <span class="grow">Zotero</span>
  <button class="icon-btn" title={t("Synchroniser la bibliothèque")} aria-label={t("Synchroniser")} disabled={syncing} onclick={sync}>
    <span class:spin={syncing}><Icon name="sync" /></span>
  </button>
</div>

{#if app.zoteroStatus && app.zoteroStatus.items > 0}
  <div class="status">
    {tn(app.zoteroStatus.items, "{n} document", "{n} documents")} · {tn(app.zoteroStatus.annotations, "{n} annotation", "{n} annotations")}
    {#if app.zoteroStatus.last_sync}<br /><span class="faint">{t("Synchronisé le {date}", { date: fmtDate(app.zoteroStatus.last_sync) })}</span>{/if}
  </div>

  <div class="box">
    <input class="input" placeholder={t("Titre, auteur, année, clé…")} bind:value={query} />
  </div>
  <div class="box import">
    <label class="only-new"><input type="checkbox" bind:checked={onlyNew} /> {t("Non importés")}</label>
    <button class="btn small" onclick={() => (app.importOpen = true)}>{t("Tout importer dans le coffre…")}</button>
  </div>

  <div class="coll-head">
    <button class="coll-toggle" onclick={() => (showColl = !showColl)}>
      <span class="chev" class:open={showColl}><Icon name="chevron" size={12} stroke={2} /></span> {t("Collections")}
    </button>
  </div>
  {#if showColl}
    <div class="colls">
      <button class="coll" class:on={current === null} onclick={() => (current = null)}>
        <Icon name="book" size={14} /> {t("Toute la bibliothèque")}
      </button>
      {#each flat as [c, d] (c.key)}
        <div class="coll-row">
          <button class="coll" class:on={current === c.key} style:padding-left="{10 + d * 14}px" onclick={() => (current = c.key)}
            oncontextmenu={(e) => app.openMenu(e, [{ label: t("Importer cette collection dans le coffre…"), icon: "download", run: () => importColl(c) }])}>
            <Icon name="folder" size={14} /> <span class="cname">{c.name}</span><span class="faint">{c.item_count}</span>
          </button>
          <button class="coll-import" title={t("Importer cette collection dans le coffre…")} aria-label={t("Importer cette collection dans le coffre…")} onclick={() => importColl(c)}>
            <Icon name="download" size={13} /> {t("Importer")}
          </button>
        </div>
      {/each}
    </div>
  {/if}

  <div class="items">
    {#each shown as it (it.key)}
      {@const fiche = app.fiches[it.citekey.toLowerCase()]}
      <div
        class="item"
        role="button"
        tabindex="0"
        draggable="true"
        ondragstart={(e) => e.dataTransfer?.setData("text/plain", `[@${it.citekey}]`)}
        onclick={(e) => openItem(it, e)}
        onkeydown={(e) => e.key === "Enter" && openItem(it)}
        oncontextmenu={(e) => itemMenu(e, it)}
      >
        <div class="t">{it.title || t("(sans titre)")}</div>
        <div class="meta">
          <span>{it.authors || t("Anonyme")}{it.year ? ` (${it.year})` : ""}</span>
          {#if it.annotation_count}<span class="badge" title={t("Annotations")}>{it.annotation_count}</span>{/if}
          {#if fiche}<span class="fiche" title={t("Source importée dans le coffre")}>{t("source")}</span>{/if}
        </div>
        <div class="actions">
          <button class="btn small" onclick={(e) => { e.stopPropagation(); fiche ? app.openNote(fiche) : app.importSource(it.key); }}>
            {fiche ? t("Ouvrir la source") : t("Importer dans le coffre")}
          </button>
          <button class="btn small ghost" title={t("Insérer [@{key}] dans la note", { key: it.citekey })} onclick={(e) => { e.stopPropagation(); cite(it); }}>
            <Icon name="at" size={14} /> {t("Citer")}
          </button>
        </div>
      </div>
    {:else}
      <p class="empty">{t("Aucun document.")}</p>
    {/each}
  </div>
{:else}
  <div class="setup">
    <h3>{t("Connecter Zotero")}</h3>
    <p>{t("Eloria lit ta bibliothèque en local, sans compte ni Internet. Elle copie la base de Zotero et n'y écrit jamais.")}</p>
    {#if detect}
      <ul class="checks">
        <li class:ok={!!detect.data_dir}>
          <Icon name={detect.data_dir ? "check" : "x"} size={14} />
          {detect.data_dir ? t("Base trouvée : {dir}", { dir: detect.data_dir }) : t("Base zotero.sqlite introuvable (indique son dossier dans les réglages)")}
        </li>
        <li class:ok={detect.api}>
          <Icon name={detect.api ? "check" : "x"} size={14} />
          {detect.api ? t("API locale active") : t("API locale inactive (facultative)")}
        </li>
      </ul>
    {/if}
    <button class="btn primary" disabled={syncing} onclick={sync}><Icon name="sync" size={15} /> {t("Importer ma bibliothèque")}</button>
    <p class="faint small">{t("Pour des clés de citation stables, installe le module Better BibTeX dans Zotero : Eloria reprend ses clés.")}</p>
  </div>
{/if}

<style>
  .coll-row {
    position: relative;
    display: flex;
    align-items: center;
  }
  .coll-row .coll {
    flex: 1;
    min-width: 0;
  }
  .coll-import {
    position: absolute;
    right: 6px;
    display: none;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-s);
    background: var(--bg-raised);
    color: var(--text);
    font-size: 11.5px;
    cursor: pointer;
  }
  .coll-row:hover .coll-import,
  .coll-row:focus-within .coll-import {
    display: inline-flex;
  }
  .import {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 6px 8px;
  }
  .only-new {
    white-space: nowrap;
  }
  .only-new {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--text-soft);
  }
  .only-new input {
    accent-color: var(--accent);
  }
  .status {
    padding: 0 14px 10px;
    font-size: 12px;
    color: var(--text-soft);
  }
  .box {
    padding: 0 10px 8px;
  }
  .coll-head {
    padding: 0 8px;
  }
  .coll-toggle {
    border: 0;
    background: transparent;
    color: var(--text-soft);
    font-weight: 600;
    font-size: 12px;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px;
  }
  .chev {
    display: inline-grid;
    transition: transform 0.15s;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .colls {
    max-height: 32%;
    overflow: auto;
    padding: 0 6px 6px;
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
  }
  .coll {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    height: 26px;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: var(--text-soft);
    text-align: left;
    padding-right: 8px;
  }
  .coll:hover {
    background: var(--bg-sunk);
  }
  .coll.on {
    background: var(--accent-soft);
    color: var(--text);
  }
  .cname {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .items {
    flex: 1;
    overflow: auto;
    padding: 6px 6px 20px;
  }
  .item {
    padding: 8px 10px;
    border-radius: 6px;
    cursor: pointer;
  }
  .item:hover {
    background: var(--bg-sunk);
  }
  .t {
    font-family: var(--font-editor);
    font-size: 14px;
    line-height: 1.35;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 6px;
    min-width: 0;
    margin-top: 3px;
    font-size: 12px;
    color: var(--text-soft);
  }
  .badge {
    font-size: 10.5px;
    padding: 0 5px;
    border-radius: 8px;
    background: color-mix(in srgb, #d9b100 25%, transparent);
    color: var(--text);
  }
  .fiche {
    font-size: 10.5px;
    padding: 0 5px;
    border-radius: 8px;
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  .actions {
    display: none;
    gap: 4px;
    margin-top: 6px;
  }
  .item:hover .actions,
  .item:focus-within .actions {
    display: flex;
  }
  .setup {
    padding: 6px 16px;
  }
  .setup h3 {
    font-family: var(--font-editor);
    font-weight: 600;
    font-size: 18px;
    margin: 4px 0 6px;
  }
  .setup p {
    color: var(--text-soft);
    line-height: 1.55;
  }
  .small {
    font-size: 12px;
  }
  .checks {
    list-style: none;
    padding: 0;
    margin: 10px 0 14px;
    font-size: 12px;
  }
  .checks li {
    display: flex;
    gap: 6px;
    align-items: flex-start;
    color: var(--danger);
    margin-bottom: 4px;
    word-break: break-all;
  }
  .checks li.ok {
    color: var(--success);
  }
  .spin {
    display: inline-grid;
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
