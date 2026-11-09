<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api, pickFile, pickSaveFile } from "../lib/api";
  import { locale, t, tn, translateError } from "../lib/i18n.svelte";
  import type { DiskReport } from "../lib/types";
  import Icon from "./Icon.svelte";

  // Import and export centre, and disk usage analysis of the vault (with the clean-up
  // options: LaTeX auxiliary files, trash, temporary files).
  let { visible = true }: { visible?: boolean } = $props();
  let disk = $state<DiskReport | null>(null);
  /** Note to export: the displayed one, otherwise the last open note tab. */
  const notePath = $derived(app.contextNotePath ?? app.panes.flatMap((p) => p.tabs).filter((tb) => tb.kind === "note" && tb.path).at(-1)?.path ?? null);
  let error = $state("");
  async function loadDisk() {
    try {
      disk = await api.diskReport();
    } catch (e) {
      error = translateError(e);
    }
  }
  $effect(() => {
    void app.revision;
    if (visible) loadDisk();
  });

  const size = (b: number) => {
    const u = [t("o"), t("Ko"), t("Mo"), t("Go")];
    let i = 0;
    let v = b;
    while (v >= 1024 && i < u.length - 1) {
      v /= 1024;
      i++;
    }
    return `${v.toLocaleString(locale(), { maximumFractionDigits: v < 10 && i > 0 ? 1 : 0 })} ${u[i]}`;
  };
  const TYPE: Record<string, string> = { notes: t("Notes"), PDF: "PDF", images: t("Images"), LaTeX: "LaTeX", "auxiliaires LaTeX": t("Auxiliaires LaTeX"), autres: t("Autres fichiers") };
  const INTERNAL: Record<string, string> = { "index.db": t("Index de recherche (jetable)"), history: t("Historique des versions"), trash: t("Corbeille"), pdftext: t("Texte des PDF"), tmp: t("Fichiers temporaires"), annotations: t("Annotations des PDF") };
  const maxType = $derived(Math.max(1, ...(disk?.by_type.map((x) => x[1]) ?? [1])));
  const maxFolder = $derived(Math.max(1, ...(disk?.by_folder.map((x) => x[1]) ?? [1])));

  async function importBib() {
    const p = await pickFile(["bib"], "BibTeX");
    if (!p) return;
    try {
      const r = await api.importBibtex(p);
      await app.refresh();
      app.fiches = await api.fichesMap();
      app.toast(t("BibTeX : {a}, {b}{f}", { a: tn(r.created, "{n} source créée", "{n} sources créées"), b: tn(r.existing, "{n} déjà présente", "{n} déjà présentes"), f: r.failed.length ? t(", {n} en échec", { n: r.failed.length }) : "" }), r.failed.length ? "info" : "success");
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }
  async function exportBib() {
    const out = await pickSaveFile("bibliographie.bib", "bib", "BibTeX");
    if (!out) return;
    try {
      const n = await api.exportBibtex(out, false);
      app.toast(tn(n, "{n} référence exportée", "{n} références exportées"), "success");
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }
  async function cleanAux() {
    if (!disk?.aux.length) return;
    if (!(await app.confirm(t("Retirer les fichiers auxiliaires de LaTeX ?"), tn(disk.aux.length, "{n} fichier (.aux, .log, .synctex…) va dans la corbeille du coffre ; la prochaine compilation le recrée.", "{n} fichiers (.aux, .log, .synctex…) vont dans la corbeille du coffre ; la prochaine compilation les recrée.")))) return;
    try {
      const n = await api.cleanLatexAux();
      app.toast(tn(n, "{n} fichier auxiliaire retiré", "{n} fichiers auxiliaires retirés"), "success");
      await app.refresh();
      await loadDisk();
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }
  async function emptyTrash() {
    if (!(await app.confirm(t("Vider la corbeille du coffre ?"), t("Les fichiers de la corbeille seront supprimés définitivement."), true, t("Vider")))) return;
    try {
      const b = await api.emptyTrash();
      app.toast(t("Corbeille vidée : {s} libérés", { s: size(b) }), "success");
      await loadDisk();
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }
  async function clearTemp() {
    try {
      const b = await api.clearTemp();
      app.toast(t("Fichiers temporaires supprimés : {s}", { s: size(b) }), "success");
      await loadDisk();
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }
  function openFile(p: string) {
    if (/\.md$/i.test(p)) app.openNote(p, { newTab: true });
    else if (/\.(tex|bib)$/i.test(p)) app.openView("latex", { path: p, title: p.split("/").pop() });
    else app.openNote(p, { newTab: true });
  }
</script>

<div class="ex">
  <h2>{t("Import et export")}</h2>
  <div class="cols">
    <section class="card">
      <h3>{t("Importer")}</h3>
      <button class="act" onclick={() => (app.importOpen = true)}><Icon name="book" size={16} /><span><b>{t("Bibliothèque Zotero")}</b>{t("Une source par document, avec PDF et annotations.")}</span></button>
      <button class="act" onclick={importBib}><Icon name="quote" size={16} /><span><b>{t("Fichier BibTeX (.bib)")}</b>{t("Mendeley, JabRef, Google Scholar : les clés d'origine sont gardées.")}</span></button>
      <button class="act" onclick={() => app.pickPdf()}><Icon name="pdf" size={16} /><span><b>{t("PDF")}</b>{t("Le DOI est lu dans le fichier, les métadonnées viennent des bases en ligne.")}</span></button>
      <button class="act" onclick={() => (app.addRefOpen = true)}><Icon name="plus" size={16} /><span><b>{t("Référence par DOI ou titre")}</b>{t("Crossref, OpenAlex, Semantic Scholar.")}</span></button>
      <button class="act" onclick={() => app.importMarkdownDir()}><Icon name="folder" size={16} /><span><b>{t("Dossier Markdown")}</b>{t("Obsidian, Notion, Logseq : liens et pièces jointes repris.")}</span></button>
    </section>
    <section class="card">
      <h3>{t("Exporter")}</h3>
      <button class="act" disabled={!notePath} onclick={() => (app.exportFor = notePath)}><Icon name="download" size={16} /><span><b>{notePath ? t("Note « {name} »", { name: app.noteByPath(notePath)?.title ?? notePath }) : t("Note ouverte")}</b>{t("Word, PDF ou HTML, avec la bibliographie.")}</span></button>
      <button class="act" onclick={exportBib}><Icon name="quote" size={16} /><span><b>{t("Bibliographie (.bib)")}</b>{t("Zotero et sources propres à Eloria.")}</span></button>
      <button class="act" onclick={() => app.assembleThesis()}><Icon name="code" size={16} /><span><b>{t("Thèse en LaTeX")}</b>{t("Les chapitres deviennent main.tex et references.bib.")}</span></button>
      <button class="act" onclick={() => app.openView("matrix")}><Icon name="table" size={16} /><span><b>{t("Matrice de recherche")}</b>{t("CSV, Markdown ou LaTeX.")}</span></button>
      <button class="act" onclick={() => app.backupNow()}><Icon name="vault" size={16} /><span><b>{t("Sauvegarde du coffre")}</b>{t("Une archive datée, dans le dossier de sauvegarde.")}</span></button>
    </section>
  </div>

  <section class="card">
    <div class="dh">
      <h3>{t("Espace disque")}</h3>
      {#if disk}<span class="tot">{size(disk.total)} · {tn(disk.files, "{n} fichier", "{n} fichiers")}</span>{/if}
      <span class="grow"></span>
      <button class="btn small ghost" onclick={loadDisk}>{t("Actualiser")}</button>
    </div>
    {#if error}<p class="error">{error}</p>{/if}
    {#if !disk && !error}<p class="loading" role="status">{t("Analyse du coffre…")}</p>{/if}
    {#if disk}
      <div class="cols">
        <div>
          <h4>{t("Par type")}</h4>
          {#each disk.by_type as [k, b, n] (k)}
            <div class="bar" title={tn(n, "{n} fichier", "{n} fichiers")}><span class="l">{TYPE[k] ?? k}</span><span class="tr"><span style:width="{(b / maxType) * 100}%"></span></span><span class="v">{size(b)}</span></div>
          {/each}
          <h4>{t("Par dossier")}</h4>
          {#each disk.by_folder.slice(0, 10) as [k, b, n] (k)}
            <div class="bar" title={tn(n, "{n} fichier", "{n} fichiers")}><span class="l">{k}</span><span class="tr"><span style:width="{(b / maxFolder) * 100}%"></span></span><span class="v">{size(b)}</span></div>
          {/each}
        </div>
        <div>
          <h4>{t("Plus gros fichiers")}</h4>
          {#each disk.largest as [p, b] (p)}
            <button class="file" title={p} onclick={() => openFile(p)}><span class="fp">{p}</span><span class="v">{size(b)}</span></button>
          {/each}
          <h4>{t("Espace interne d'Eloria (.eloria)")}</h4>
          {#each disk.internal as [k, b] (k)}
            <div class="file static"><span class="fp">{INTERNAL[k] ?? k}</span><span class="v">{size(b)}</span></div>
          {/each}
        </div>
      </div>
      <h4>{t("Ménage")}</h4>
      <div class="clean">
        <button class="btn small" disabled={!disk.aux.length} onclick={cleanAux}>{tn(disk.aux.length, "Retirer le fichier auxiliaire LaTeX ({n} fichier, {s})", "Retirer les fichiers auxiliaires LaTeX ({n} fichiers, {s})", { s: size(disk.aux_bytes) })}</button>
        <button class="btn small" disabled={!disk.trash_bytes} onclick={emptyTrash}>{t("Vider la corbeille ({s})", { s: size(disk.trash_bytes) })}</button>
        <button class="btn small" onclick={clearTemp}>{t("Supprimer les fichiers temporaires")}</button>
      </div>
    {/if}
  </section>
</div>

<style>
  .ex {
    height: 100%;
    overflow: auto;
    padding: 20px 26px 30px;
  }
  h2 {
    margin: 0 0 14px;
    font-size: 20px;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 14px;
  }
  h4 {
    margin: 12px 0 6px;
    font-size: 12px;
    color: var(--text-soft);
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: 12px;
  }
  .card {
    padding: 12px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--bg);
    min-width: 0;
    margin-bottom: 12px;
  }
  .cols > .card {
    margin-bottom: 0;
  }
  .ex > .cols {
    margin-bottom: 12px;
  }
  .act {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    width: 100%;
    border: 0;
    border-radius: var(--radius);
    background: none;
    padding: 7px 8px;
    text-align: left;
    color: var(--text);
    cursor: pointer;
  }
  .act:hover:not(:disabled) {
    background: var(--bg-soft);
  }
  .act:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .act span {
    display: flex;
    flex-direction: column;
    font-size: 12px;
    color: var(--text-soft);
    min-width: 0;
  }
  .act b {
    font-size: 13px;
    color: var(--text);
  }
  .dh {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .dh h3 {
    margin: 0;
  }
  .grow {
    flex: 1;
  }
  .tot {
    font-size: 12.5px;
    color: var(--text-soft);
  }
  .bar {
    display: grid;
    grid-template-columns: minmax(0, 150px) minmax(0, 1fr) 70px;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    padding: 2px 0;
  }
  .l {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tr {
    height: 8px;
  }
  .tr span {
    display: block;
    height: 100%;
    min-width: 2px;
    border-radius: 0 4px 4px 0;
    background: var(--accent);
  }
  .v {
    text-align: right;
    font-variant-numeric: tabular-nums;
    color: var(--text-soft);
    white-space: nowrap;
  }
  .file {
    display: flex;
    align-items: baseline;
    gap: 8px;
    width: 100%;
    border: 0;
    background: none;
    padding: 2px 0;
    font: 12.5px var(--font-ui);
    color: var(--text);
    text-align: left;
    cursor: pointer;
    min-width: 0;
  }
  .file.static {
    cursor: default;
  }
  button.file:hover .fp {
    color: var(--accent-text);
  }
  .fp {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .clean {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .error {
    color: var(--danger);
  }
</style>
