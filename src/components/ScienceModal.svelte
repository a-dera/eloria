<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, tn, translateError } from "../lib/i18n.svelte";
  import type { SciTask, SupportVerdict } from "../lib/types";
  import Modal from "./Modal.svelte";

  // Scientific writing assistant (AI): produce LaTeX, explain or fix a
  // formula, suggest improvements, check that each citation is supported by the
  // cited document. No reference is added; nothing is inserted without your click.
  const req = app.scienceFor!;
  let tab = $state<"generer" | "formule" | "ameliorer" | "citations">(req.tab ?? (req.text ? "formule" : "generer"));
  let gen = $state<SciTask>("latex_table");
  let formula = $state<SciTask>("explain_formula");
  let input = $state(req.text);
  let output = $state("");
  let busy = $state(false);
  let error = $state("");
  let verdicts = $state<SupportVerdict[] | null>(null);
  const isLatex = /\.tex$/i.test(req.path);

  const task = $derived<SciTask>(tab === "generer" ? gen : tab === "formule" ? formula : "improve");
  const canInsert = $derived(!!output && (tab === "generer" || (tab === "formule" && formula === "fix_formula")));

  async function run() {
    busy = true;
    error = "";
    output = "";
    try {
      const r = await api.aiScience(task, input, (d) => (output += d));
      output = r.text;
    } catch (e) {
      error = translateError(e);
    } finally {
      busy = false;
    }
  }
  async function check() {
    busy = true;
    error = "";
    verdicts = null;
    try {
      verdicts = await api.aiCheckSupport(req.path);
    } catch (e) {
      error = translateError(e);
    } finally {
      busy = false;
    }
  }
  function insert() {
    // Formula fix: only the code block is inserted.
    const code = /```(?:latex|tex)?\n([\s\S]*?)```/.exec(output)?.[1]?.trim() ?? output;
    if (app.insertInEditor(tab === "formule" ? code : output)) {
      app.scienceFor = null;
      app.toast(t("Inséré à la place de la sélection"), "success");
    } else app.toast(t("Ouvre d'abord une note ou un document LaTeX"), "info");
  }
  function goto(line: number) {
    app.scienceFor = null;
    if (isLatex) app.openView("latex", { path: req.path, title: req.path.split("/").pop() });
    else app.openNote(req.path, { line });
  }
  const LABEL: Record<string, string> = { soutenue: t("soutenue"), partielle: t("partielle"), non_soutenue: t("non soutenue"), inconnue: t("pas de matériau") };
  const HINT: Record<string, string> = {
    latex_table: t("Décris le tableau ou colle des données (CSV, tableau Markdown)."),
    latex_equation: t("Décris l'équation en mots, ou écris-la approximativement."),
    tikz: t("Décris la figure : éléments, flèches, disposition."),
    explain_formula: t("Colle la formule LaTeX à expliquer."),
    fix_formula: t("Colle la formule LaTeX à corriger."),
    improve: t("Colle le passage à améliorer (les références restent intactes)."),
  };
</script>

<Modal title={t("Assistant d'écriture scientifique")} width={760} onclose={() => (app.scienceFor = null)}>
  <div class="body">
    {#if !app.settings?.ai_enabled}
      <p class="ai-off">{t("L'IA est désactivée. Active-la et enregistre ta clé API dans Réglages › Intelligence artificielle.")}</p>
    {/if}
    <div class="tabs" role="tablist" aria-label={t("Tâche")}>
      <button role="tab" aria-selected={tab === "generer"} class:on={tab === "generer"} onclick={() => (tab = "generer")}>{t("Générer du LaTeX")}</button>
      <button role="tab" aria-selected={tab === "formule"} class:on={tab === "formule"} onclick={() => (tab = "formule")}>{t("Formule")}</button>
      <button role="tab" aria-selected={tab === "ameliorer"} class:on={tab === "ameliorer"} onclick={() => (tab = "ameliorer")}>{t("Améliorer")}</button>
      <button role="tab" aria-selected={tab === "citations"} class:on={tab === "citations"} onclick={() => (tab = "citations")} disabled={!req.path}>{t("Cohérence des citations")}</button>
    </div>

    {#if tab === "citations"}
      <p class="hint">{t("Chaque phrase qui cite est confrontée à ce que ton coffre sait du document cité (résumé, fiche, texte du PDF). Un verdict à vérifier, pas une preuve.")}</p>
      <button class="btn primary" disabled={busy || !req.path} onclick={check}>{busy ? t("Vérification…") : t("Vérifier les citations de {name}", { name: req.path.split("/").pop() ?? "" })}</button>
      {#if verdicts}
        <p class="sum">
          {tn(verdicts.length, "{n} citation vérifiée", "{n} citations vérifiées")} ·
          <span class="bad">{tn(verdicts.filter((v) => v.verdict === "non_soutenue").length, "{n} non soutenue", "{n} non soutenues")}</span> ·
          <span class="warn">{tn(verdicts.filter((v) => v.verdict === "partielle").length, "{n} partielle", "{n} partielles")}</span>
        </p>
        <ul class="verdicts">
          {#each verdicts as v, i (i)}
            <li class={v.verdict}>
              <div class="vh">
                <span class="badge">{LABEL[v.verdict] ?? v.verdict}</span>
                <span class="ck">@{v.citekey}</span>
                <button class="loc" onclick={() => goto(v.line)}>{t("ligne {n}", { n: v.line })}</button>
              </div>
              <p class="sent">{v.sentence}</p>
              {#if v.reason}<p class="why">{v.reason}</p>{/if}
            </li>
          {:else}
            <li class="faint">{t("Aucune phrase avec citation dans ce document.")}</li>
          {/each}
        </ul>
      {/if}
    {:else}
      {#if tab === "generer"}
        <div class="chips" role="radiogroup" aria-label={t("Que générer ?")}>
          <label class:on={gen === "latex_table"}><input type="radio" bind:group={gen} value="latex_table" /> {t("Tableau")}</label>
          <label class:on={gen === "latex_equation"}><input type="radio" bind:group={gen} value="latex_equation" /> {t("Équation")}</label>
          <label class:on={gen === "tikz"}><input type="radio" bind:group={gen} value="tikz" /> {t("Figure TikZ")}</label>
        </div>
      {:else if tab === "formule"}
        <div class="chips" role="radiogroup" aria-label={t("Que faire de la formule ?")}>
          <label class:on={formula === "explain_formula"}><input type="radio" bind:group={formula} value="explain_formula" /> {t("Expliquer")}</label>
          <label class:on={formula === "fix_formula"}><input type="radio" bind:group={formula} value="fix_formula" /> {t("Corriger")}</label>
        </div>
      {/if}
      <textarea rows="5" bind:value={input} placeholder={HINT[task]} aria-label={t("Texte de départ")}></textarea>
      <div class="row">
        <button class="btn primary" disabled={busy || !input.trim()} onclick={run}>{busy ? t("Rédaction…") : t("Lancer")}</button>
        {#if busy}<button class="btn" onclick={() => api.aiCancel()}>{t("Arrêter")}</button>{/if}
      </div>
      {#if output}
        <pre class="out" aria-label={t("Réponse de l'IA")}>{output}</pre>
      {/if}
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
  </div>
  {#snippet footer()}
    {#if output && tab !== "citations"}<button class="btn" onclick={() => navigator.clipboard.writeText(output)}>{t("Copier")}</button>{/if}
    <button class="btn" onclick={() => (app.scienceFor = null)}>{t("Fermer")}</button>
    {#if !app.settings?.ai_enabled}<button class="btn primary" onclick={() => { app.scienceFor = null; app.openSettings("ia"); }}>{t("Ouvrir les réglages")}</button>{/if}
    {#if canInsert}<button class="btn primary" onclick={insert}>{t("Insérer dans le document")}</button>{/if}
  {/snippet}
</Modal>

<style>
  .ai-off {
    margin: 0 0 12px;
    padding: 10px 12px;
    border-radius: var(--radius);
    background: var(--bg-sunk);
    color: var(--text-soft);
    font-size: 13px;
  }
  .body {
    padding: 14px 22px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-height: 70vh;
    overflow: auto;
  }
  .tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    border-bottom: 1px solid var(--line);
    padding-bottom: 8px;
  }
  .tabs button {
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    padding: 4px 10px;
    font: 13px var(--font-ui);
    color: var(--text-soft);
    cursor: pointer;
  }
  .tabs button.on {
    background: var(--accent-soft);
    color: var(--text);
    font-weight: 600;
  }
  .tabs button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chips label {
    padding: 2px 12px;
    border: 1px solid var(--line);
    border-radius: 999px;
    font-size: 13px;
    color: var(--text-soft);
    cursor: pointer;
  }
  .chips label.on {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--text);
  }
  .chips input {
    display: none;
  }
  textarea {
    width: 100%;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg-raised);
    color: var(--text);
    font: 13px var(--font-mono);
    padding: 8px;
    resize: vertical;
  }
  .row {
    display: flex;
    gap: 6px;
  }
  .out {
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--radius);
    background: var(--code-bg);
    font: 12.5px/1.5 var(--font-mono);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    max-height: 40vh;
    overflow: auto;
  }
  .hint,
  .why {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-soft);
  }
  .sum {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
  }
  .verdicts {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .verdicts li {
    padding: 8px 0 8px 10px;
    border-bottom: 1px solid var(--line);
    border-left: 3px solid var(--line-strong);
  }
  .verdicts li.non_soutenue {
    border-left-color: var(--danger);
  }
  .verdicts li.partielle {
    border-left-color: var(--warning);
  }
  .verdicts li.soutenue {
    border-left-color: var(--success);
  }
  .vh {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .badge {
    font-size: 11.5px;
    font-weight: 600;
    padding: 0 8px;
    border-radius: 999px;
    background: var(--bg-sunk);
  }
  .non_soutenue .badge {
    color: var(--danger);
  }
  .partielle .badge {
    color: var(--warning);
  }
  .soutenue .badge {
    color: var(--success);
  }
  .ck {
    font: 12px var(--font-mono);
    color: var(--text-soft);
  }
  .loc {
    border: 0;
    background: none;
    padding: 0;
    font: 600 12px var(--font-ui);
    color: var(--accent-text);
    cursor: pointer;
  }
  .sent {
    margin: 4px 0 2px;
    font-size: 13px;
  }
  .bad {
    color: var(--danger);
  }
  .warn {
    color: var(--warning);
  }
  .faint {
    color: var(--text-faint);
    font-size: 12.5px;
  }
  .error {
    color: var(--danger);
  }
</style>
