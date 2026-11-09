<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, tn, translateError } from "../lib/i18n.svelte";
  import type { ImportPreview, ImportReport } from "../lib/types";
  import Modal from "./Modal.svelte";

  let byCollection = $state(true);
  let busy = $state(false);
  let report = $state<ImportReport | null>(null);
  let error = $state("");

  const s = $derived(app.settings!);
  /** Only items without a note are imported: never a duplicate. */
  let preview = $state<ImportPreview | null>(null);
  const coll = app.importCollection;
  $effect(() => {
    api.zoteroImportPreview(coll?.key).then((p) => (preview = p)).catch(() => (preview = null));
  });
  /** Notes of the old import left empty, duplicates of the sources. */
  let legacy = $state<string[]>([]);
  api.legacyImports().then((l) => (legacy = l)).catch(() => {});

  async function cleanLegacy() {
    try {
      const n = await api.cleanLegacyImports();
      legacy = [];
      await app.refresh();
      app.fiches = await api.fichesMap();
      app.toast(tn(n, "{n} doublon mis à la corbeille du coffre", "{n} doublons mis à la corbeille du coffre"), "success");
    } catch (e) {
      error = translateError(e);
    }
  }

  async function run() {
    busy = true;
    error = "";
    try {
      report = await api.zoteroImportAll(byCollection, coll?.key);
      await app.refresh();
      app.fiches = await api.fichesMap();
    } catch (e) {
      error = translateError(e);
    } finally {
      busy = false;
    }
  }
</script>

<Modal title={coll ? t("Importer la collection « {name} »", { name: coll.name }) : t("Importer la bibliothèque Zotero dans le coffre")} width={560} onclose={() => { app.importOpen = false; app.importCollection = null; }}>
  <div class="body">
    {#if !report}
      {#if preview}
        <p>
          {#if preview.new}
            <b>{tn(preview.new, "{n} nouveau document à importer", "{n} nouveaux documents à importer")}</b>
            {coll ? t("sur {total} dans cette collection (sous-collections comprises).", { total: preview.total }) : t("sur {total} dans la bibliothèque.", { total: preview.total })}
          {:else}
            <b>{t("Tout est déjà importé.")}</b>
          {/if}
        </p>
        {#if preview.existing}
          <p class="hint">{tn(preview.existing, "{n} document est déjà une source du coffre : il n'est pas réimporté. Sa source reçoit seulement les tags Zotero qui lui manquent ; ton texte reste intact.", "{n} documents sont déjà des sources du coffre : ils ne sont pas réimportés. Leurs sources reçoivent seulement les tags Zotero qui leur manquent ; ton texte reste intact.")}</p>
        {/if}
      {/if}
      <p class="hint">{t("Chaque document devient une source : la même vue que dans le panneau Zotero (notice, résumé, PDF, annotations). Tu y joins ensuite tes fiches avec « Créer une fiche ».")}</p>
      <label class="check"><input type="checkbox" bind:checked={byCollection} /> {t("Ranger dans des sous-dossiers qui reprennent les collections Zotero")}</label>
      <label class="check">
        <input type="checkbox" checked={s.copy_pdfs} onchange={() => app.updateSettings({ copy_pdfs: !s.copy_pdfs })} />
        {t("Copier aussi les PDF dans le coffre")}
      </label>
      <p class="hint">
        {t("Tes sources et leurs PDF restent alors disponibles même si tu supprimes Zotero ou son dossier de fichiers. Les PDF prennent de la place sur le disque.")}
      </p>
      <dl>
        <dt>{t("Sources")}</dt>
        <dd><code>{s.sources_folder}{byCollection ? "/…" : ""}</code></dd>
        {#if s.copy_pdfs}<dt>PDF</dt><dd><code>{s.pdf_folder}{byCollection ? "/…" : ""}</code></dd>{/if}
      </dl>
      {#if legacy.length}
        <div class="legacy">
          <p>{tn(legacy.length, "{n} note de l'ancien import est restée vide et fait doublon avec sa source.", "{n} notes de l'ancien import sont restées vides et font doublon avec leurs sources.")}</p>
          <p class="hint">{t("Seules les notes où tu n'as rien écrit sont concernées. Elles vont dans la corbeille du coffre (.eloria/trash) : rien n'est perdu.")}</p>
          <button class="btn small" onclick={cleanLegacy}>{t("Mettre ces doublons à la corbeille")}</button>
        </div>
      {/if}
      {#if error}<p class="error">{error}</p>{/if}
    {:else}
      <p class="ok">{tn(report.created, "{n} source créée.", "{n} sources créées.")}</p>
      <ul>
        {#if report.updated}<li>{tn(report.updated, "{n} source existante complétée (tags)", "{n} sources existantes complétées (tags)")}</li>{/if}
        {#if report.pdfs}<li>{tn(report.pdfs, "{n} PDF copié dans le coffre", "{n} PDF copiés dans le coffre")}</li>{/if}
        {#if report.skipped}<li>{tn(report.skipped, "{n} document était déjà une source", "{n} documents étaient déjà des sources")}</li>{/if}
        {#if report.failed.length}
          <li class="error">{tn(report.failed.length, "{n} échec :", "{n} échecs :")}</li>
          {#each report.failed.slice(0, 8) as f (f)}<li class="error small">{f}</li>{/each}
        {/if}
      </ul>
    {/if}
  </div>
  {#snippet footer()}
    {#if !report}
      <button class="btn" onclick={() => { app.importOpen = false; app.importCollection = null; }}>{t("Annuler")}</button>
      <button class="btn primary" disabled={busy || !preview?.total} onclick={run}>
        {busy ? t("Import en cours…") : preview && !preview.new ? t("Synchroniser les sources existantes") : t("Importer")}
      </button>
    {:else}
      <button class="btn" onclick={() => { app.importOpen = false; app.importCollection = null; }}>{t("Fermer")}</button>
    {/if}
  {/snippet}
</Modal>

<style>
  .legacy {
    margin: 12px 0;
    padding: 10px 12px;
    border-radius: var(--radius);
    background: var(--bg-sunk);
  }
  .legacy p {
    margin: 0 0 6px;
  }
  .body {
    padding: 18px 22px;
    user-select: text;
  }
  p {
    margin: 0 0 10px;
  }
  .hint {
    font-size: 12.5px;
    color: var(--text-soft);
  }
  .check {
    display: flex;
    gap: 8px;
    align-items: center;
    margin: 10px 0;
  }
  .check input {
    accent-color: var(--accent);
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 4px 14px;
    margin: 14px 0 0;
    font-size: 12.5px;
  }
  dt {
    color: var(--text-faint);
  }
  dd {
    margin: 0;
  }
  code {
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .ok {
    color: var(--success);
    font-weight: 600;
  }
  .error {
    color: var(--danger);
  }
  .small {
    font-size: 12px;
  }
</style>
