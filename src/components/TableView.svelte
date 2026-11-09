<script lang="ts">
  import { locale, t, tn, translateError } from "../lib/i18n.svelte";
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { editors } from "../lib/editor/registry";
  import type { NoteMeta } from "../lib/types";
  import { fmtDate, parsePropInput, propText } from "../lib/util";
  import { propDef } from "../lib/custom";
  import type { SavedView } from "../lib/types";
  import Icon from "./Icon.svelte";

  let { viewName }: { viewName?: string } = $props();

  let folder = $state("");
  let typeFilter = $state("");
  let tag = $state("");
  let text = $state("");
  let sortKey = $state("title");
  let sortDir = $state<1 | -1>(1);
  let hidden = $state<Record<string, boolean>>({});
  let showCols = $state(false);

  // Saved views (Settings › Customization): filters, sort order and columns.
  const views = $derived(app.settings?.saved_views ?? []);
  let current = $state("");
  function applyView(v: SavedView) {
    current = v.name;
    folder = v.folder;
    typeFilter = v.type_filter;
    tag = v.tag;
    text = v.text;
    sortKey = v.sort_key || "title";
    sortDir = v.sort_dir < 0 ? -1 : 1;
    hidden = Object.fromEntries(v.hidden.map((c) => [c, true]));
  }
  function resetView() {
    current = "";
    folder = typeFilter = tag = text = "";
    sortKey = "title";
    sortDir = 1;
    hidden = {};
  }
  // Opened from the palette ("View: …"): the view applies right away.
  let applied = false;
  $effect(() => {
    if (applied || !viewName) return;
    const v = views.find((x) => x.name === viewName);
    if (v) {
      applied = true;
      applyView(v);
    }
  });
  function pickView(name: string) {
    const v = views.find((x) => x.name === name);
    if (v) applyView(v);
    else resetView();
  }
  async function saveView() {
    const name = (await app.ask(t("Nom de la vue"), current || t("Ma vue")))?.trim();
    if (!name) return;
    const v: SavedView = {
      name,
      folder,
      type_filter: typeFilter,
      tag,
      text,
      sort_key: sortKey,
      sort_dir: sortDir,
      hidden: Object.keys(hidden).filter((c) => hidden[c]),
    };
    await app.updateSettings({ saved_views: [...views.filter((x) => x.name !== name), v] });
    current = name;
    app.toast(t("Vue « {name} » enregistrée", { name }));
  }
  async function deleteView() {
    if (!current) return;
    await app.updateSettings({ saved_views: views.filter((x) => x.name !== current) });
    app.toast(t("Vue « {name} » supprimée", { name: current }));
    current = "";
  }

  const folders = $derived([...new Set(app.notes.map((n) => (n.path.includes("/") ? n.path.slice(0, n.path.lastIndexOf("/")) : "")).filter(Boolean))].sort());
  const types = $derived([...new Set(app.notes.map((n) => String(n.frontmatter.type ?? "")).filter(Boolean))].sort());

  const rows = $derived.by(() => {
    const q = text.trim().toLowerCase();
    const tagq = tag.trim().replace(/^#/, "").toLowerCase();
    const tpl = (app.settings?.templates_folder ?? "Modèles") + "/";
    return app.notes.filter(
      (n) =>
        !n.path.startsWith(tpl) &&
        (!folder || n.path.startsWith(folder + "/")) &&
        (!typeFilter || n.frontmatter.type === typeFilter) &&
        (!tagq || n.tags.some((x) => x.toLowerCase() === tagq || x.toLowerCase().startsWith(tagq + "/"))) &&
        (!q || n.title.toLowerCase().includes(q) || JSON.stringify(n.frontmatter).toLowerCase().includes(q)),
    );
  });

  const propKeys = $derived(
    [...new Set(rows.flatMap((n) => Object.keys(n.frontmatter)))].filter((k) => !["titre", "title"].includes(k)).sort((a, b) => {
      const order = ["type", "statut", "auteurs", "annee", "chapitres", "priorite", "citekey"];
      const ia = order.indexOf(a);
      const ib = order.indexOf(b);
      return (ia < 0 ? 99 : ia) - (ib < 0 ? 99 : ib) || a.localeCompare(b);
    }),
  );
  const cols = $derived(["title", ...propKeys, "words", "mtime"].filter((c) => !hidden[c]));

  function value(n: NoteMeta, c: string): unknown {
    if (c === "title") return n.title;
    if (c === "words") return n.words;
    if (c === "mtime") return n.mtime;
    return n.frontmatter[c];
  }

  const sorted = $derived(
    [...rows].sort((a, b) => {
      const va = value(a, sortKey);
      const vb = value(b, sortKey);
      if (typeof va === "number" && typeof vb === "number") return (va - vb) * sortDir;
      return propText(va).localeCompare(propText(vb), "fr", { numeric: true }) * sortDir || a.title.localeCompare(b.title);
    }),
  );

  function sortBy(c: string) {
    if (sortKey === c) sortDir = sortDir === 1 ? -1 : 1;
    else {
      sortKey = c;
      sortDir = 1;
    }
  }

  const label = (c: string) => ({ title: t("Titre"), words: t("Mots"), mtime: t("Modifiée") })[c] ?? c;

  async function edit(n: NoteMeta, key: string, raw: string) {
    const prev = n.frontmatter[key];
    const v = parsePropInput(raw, prev);
    if (propText(v) === propText(prev)) return;
    await editors.savePath(n.path);
    try {
      const content = await api.setProperty(n.path, key, v === "" && prev === undefined ? null : v);
      editors.pushContent(n.path, content);
      await app.refresh();
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }

  function exportCsv() {
    const esc = (s: string) => `"${s.replace(/"/g, '""')}"`;
    const head = cols.map((c) => esc(label(c))).join(";");
    const body = sorted.map((n) => cols.map((c) => esc(c === "mtime" ? fmtDate(n.mtime) : propText(value(n, c)))).join(";")).join("\n");
    const blob = new Blob(["﻿" + head + "\n" + body], { type: "text/csv;charset=utf-8" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = "notes.csv";
    a.click();
  }
</script>

<div class="table-view">
  <div class="toolbar">
    <select class="input view-pick" aria-label={t("Vue enregistrée")} value={current} onchange={(e) => pickView((e.target as HTMLSelectElement).value)}>
      <option value="">{t("Toutes les notes")}</option>
      {#each views as v (v.name)}<option value={v.name}>{v.name}</option>{/each}
    </select>
    <button class="btn small" onclick={saveView} title={t("Enregistrer les filtres, le tri et les colonnes")}>{t("Enregistrer la vue")}</button>
    {#if current}<button class="icon-btn" title={t("Supprimer la vue")} aria-label={t("Supprimer la vue")} onclick={deleteView}><Icon name="x" size={14} /></button>{/if}
    <input class="input q" placeholder={t("Rechercher titre ou propriété")} bind:value={text} />
    <select class="input" bind:value={folder}>
      <option value="">{t("Tous les dossiers")}</option>
      {#each folders as f (f)}<option value={f}>{f}</option>{/each}
    </select>
    <select class="input" bind:value={typeFilter}>
      <option value="">{t("Tous les types")}</option>
      {#each types as tagq (tagq)}<option value={tagq}>{tagq}</option>{/each}
    </select>
    <input class="input tag" placeholder="#tag" bind:value={tag} />
    <span class="grow"></span>
    <span class="faint">{tn(sorted.length, "{n} note", "{n} notes")}</span>
    <div class="cols-wrap">
      <button class="btn small" onclick={() => (showCols = !showCols)}><Icon name="filter" size={14} /> {t("Colonnes")}</button>
      {#if showCols}
        <div class="cols-menu">
          {#each ["title", ...propKeys, "words", "mtime"] as c (c)}
            <label><input type="checkbox" checked={!hidden[c]} onchange={() => (hidden[c] = !hidden[c])} /> {label(c)}</label>
          {/each}
        </div>
      {/if}
    </div>
    <button class="btn small" onclick={exportCsv}><Icon name="download" size={14} /> CSV</button>
  </div>

  <div class="scroll">
    <table>
      <thead>
        <tr>
          {#each cols as c (c)}
            <th onclick={() => sortBy(c)} class:sorted={sortKey === c}>
              {label(c)}{#if sortKey === c}<span class="dir">{sortDir === 1 ? "↑" : "↓"}</span>{/if}
            </th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#each sorted as n (n.path)}
          <tr>
            {#each cols as c (c)}
              {#if c === "title"}
                <td class="title"><button onclick={(e) => app.openNote(n.path, { newTab: true, otherPane: e.altKey })}>{n.title}</button></td>
              {:else if c === "words"}
                <td class="num">{n.words.toLocaleString(locale())}</td>
              {:else if c === "mtime"}
                <td class="faint nowrap">{fmtDate(n.mtime)}</td>
              {:else}
                {@const d = propDef(c)}
                <td>
                  {#if d?.kind === "case"}
                    <input type="checkbox" class="cbox" aria-label={c} checked={n.frontmatter[c] === true} onchange={(e) => edit(n, c, String((e.target as HTMLInputElement).checked))} />
                  {:else if d?.kind === "choix"}
                    <select class="cell" aria-label={c} value={propText(n.frontmatter[c])} onchange={(e) => edit(n, c, (e.target as HTMLSelectElement).value)}>
                      {#if !d.options.includes(propText(n.frontmatter[c]))}<option value={propText(n.frontmatter[c])}>{propText(n.frontmatter[c])}</option>{/if}
                      {#each d.options as o (o)}<option value={o}>{o}</option>{/each}
                    </select>
                  {:else}
                    <input class="cell" value={propText(n.frontmatter[c])} onchange={(e) => edit(n, c, (e.target as HTMLInputElement).value)} />
                  {/if}
                </td>
              {/if}
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>
    {#if !sorted.length}<p class="empty">{t("Aucune note ne correspond aux filtres.")}</p>{/if}
  </div>
</div>

<style>
  .table-view {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 16px;
    border-bottom: 1px solid var(--line);
    flex-wrap: wrap;
    font-size: 12.5px;
  }
  .toolbar .input {
    height: 28px;
    width: auto;
  }
  .toolbar .q {
    width: 220px;
  }
  .toolbar .tag {
    width: 110px;
  }
  .grow {
    flex: 1;
  }
  .cols-wrap {
    position: relative;
  }
  .cols-menu {
    position: absolute;
    right: 0;
    top: 32px;
    z-index: 20;
    background: var(--bg-raised);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
    padding: 8px 10px;
    max-height: 320px;
    overflow: auto;
    min-width: 180px;
  }
  .cols-menu label {
    display: flex;
    gap: 6px;
    padding: 3px 0;
    white-space: nowrap;
  }
  .scroll {
    flex: 1;
    overflow: auto;
  }
  table {
    border-collapse: separate;
    border-spacing: 0;
    min-width: 100%;
    font-size: 13px;
  }
  th {
    position: sticky;
    top: 0;
    background: var(--bg-soft);
    text-align: left;
    font-weight: 600;
    color: var(--text-soft);
    padding: 8px 12px;
    border-bottom: 1px solid var(--line);
    cursor: pointer;
    white-space: nowrap;
    z-index: 1;
  }
  th.sorted {
    color: var(--text);
  }
  .dir {
    margin-left: 4px;
    color: var(--accent-text);
  }
  td {
    padding: 0 12px;
    overflow: hidden;
    height: 36px;
    border-bottom: 1px solid var(--line);
    max-width: 320px;
  }
  tr:hover td {
    background: var(--bg-soft);
  }
  .title button {
    border: 0;
    background: transparent;
    padding: 0;
    font-family: var(--font-editor);
    font-size: 14px;
    text-align: left;
    color: var(--text);
    max-width: 340px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    display: block;
  }
  .title button:hover {
    color: var(--accent-text);
    text-decoration: underline;
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
    color: var(--text-soft);
  }
  .nowrap {
    white-space: nowrap;
  }
  .cell {
    width: calc(100% + 7px);
    margin-left: -7px;
    min-width: 90px;
    border: 1px solid transparent;
    background: transparent;
    border-radius: 4px;
    padding: 3px 6px;
    outline: none;
    user-select: text;
  }
  .cbox {
    accent-color: var(--accent);
  }
  /* Choice list: the arrow only shows on hover, to keep the table readable. */
  select.cell {
    appearance: none;
    cursor: pointer;
  }
  select.cell:hover,
  select.cell:focus {
    appearance: auto;
  }
  .view-pick {
    max-width: 180px;
  }
  .cell:hover {
    border-color: var(--line);
  }
  .cell:focus {
    border-color: var(--accent);
    background: var(--bg-raised);
  }
</style>
