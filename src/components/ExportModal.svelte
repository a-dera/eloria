<script lang="ts">
  import { t, tk, translateError } from "../lib/i18n.svelte";
  import { app } from "../lib/state.svelte";
  import { api, pickSaveFile } from "../lib/api";
  import { editors } from "../lib/editor/registry";
  import type { ExportOptions } from "../lib/types";
  import { stem } from "../lib/util";
  import Modal from "./Modal.svelte";

  const path = app.exportFor!;
  let format = $state<ExportOptions["format"]>("docx");
  let citations = $state(true);
  let strip = $state(true);
  let toc = $state(false);
  let busy = $state(false);
  let pandoc = $state<string | null | undefined>(undefined);
  let error = $state("");

  $effect(() => {
    api.pandocVersion().then((v) => (pandoc = v));
  });

  const FORMATS: { id: ExportOptions["format"]; label: string; ext: string; needsPandoc: boolean }[] = [
    { id: "docx", label: tk("Word (.docx)"), ext: "docx", needsPandoc: true },
    { id: "pdf", label: tk("PDF"), ext: "pdf", needsPandoc: true },
    { id: "odt", label: tk("LibreOffice (.odt)"), ext: "odt", needsPandoc: true },
    { id: "tex", label: tk("LaTeX (.tex)"), ext: "tex", needsPandoc: true },
    { id: "html", label: tk("Page web (.html)"), ext: "html", needsPandoc: false },
  ];

  async function run() {
    const f = FORMATS.find((x) => x.id === format)!;
    const out = await pickSaveFile(`${stem(path)}.${f.ext}`, f.ext, t(f.label));
    if (!out) return;
    busy = true;
    error = "";
    try {
      await editors.savePath(path);
      const res = await api.exportNote(path, { format, output: out, citations, strip_annotations: strip, toc });
      app.toast(t("Exporté : {path}", { path: res }), "success");
      app.exportFor = null;
    } catch (e) {
      error = translateError(e);
    } finally {
      busy = false;
    }
  }
</script>

<Modal title={t("Exporter « {name} »", { name: stem(path) })} width={520} onclose={() => (app.exportFor = null)}>
  <div class="exp">
    <div class="formats">
      {#each FORMATS as f (f.id)}
        <label class="fmt" class:on={format === f.id} class:off={f.needsPandoc && pandoc === null}>
          <input type="radio" name="fmt" value={f.id} bind:group={format} disabled={f.needsPandoc && pandoc === null} />
          {t(f.label)}
        </label>
      {/each}
    </div>
    {#if pandoc === null}
      <p class="warn">{t("Pandoc n'est pas installé : seul l'export HTML est disponible. Installe-le avec")} <code>winget install JohnMacFarlane.Pandoc</code>.</p>
    {/if}
    <label class="check"><input type="checkbox" bind:checked={citations} /> {t("Résoudre les citations")} <code>[@clé]</code> {t("et ajouter la bibliographie")}</label>
    <label class="check"><input type="checkbox" bind:checked={strip} /> {t("Retirer le bloc d'annotations Zotero")}</label>
    <label class="check"><input type="checkbox" bind:checked={toc} /> {t("Table des matières")}</label>
    <p class="hint">{t("Les liens internes deviennent du texte, les encadrés deviennent des citations. Le style des références se règle dans Réglages › Export.")}</p>
    {#if error}<pre class="error">{error}</pre>{/if}
  </div>
  {#snippet footer()}
    <button class="btn" onclick={() => (app.exportFor = null)}>{t("Annuler")}</button>
    <button class="btn primary" disabled={busy} onclick={run}>{busy ? t("Export en cours…") : t("Exporter")}</button>
  {/snippet}
</Modal>

<style>
  .exp {
    padding: 16px 20px;
  }
  .formats {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
    margin-bottom: 14px;
  }
  .fmt {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 12px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    cursor: pointer;
  }
  .fmt.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .fmt.off {
    opacity: 0.45;
    cursor: default;
  }
  .fmt input,
  .check input {
    accent-color: var(--accent);
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 8px 0;
  }
  .hint {
    color: var(--text-soft);
    font-size: 12px;
    line-height: 1.55;
  }
  .warn {
    color: var(--danger);
    font-size: 12.5px;
  }
  .error {
    color: var(--danger);
    white-space: pre-wrap;
    font-size: 12px;
    background: var(--bg-soft);
    padding: 8px;
    border-radius: 6px;
    max-height: 160px;
    overflow: auto;
  }
  code {
    font-family: var(--font-mono);
    font-size: 11.5px;
  }
</style>
