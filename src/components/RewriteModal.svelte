<script lang="ts">
  import { onDestroy } from "svelte";
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, tk, translateError } from "../lib/i18n.svelte";
  import type { RewriteTask } from "../lib/types";
  import Modal from "./Modal.svelte";

  // Writing assistant: suggestion shown next to the original, to accept or
  // reject. Citations and links are protected by the core: a suggestion that
  // touches them is rejected before reaching this point.
  const req = app.rewriteFor!;
  const TASKS: { id: RewriteTask; label: string }[] = [
    { id: "rephrase", label: tk("Reformuler") },
    { id: "fix", label: tk("Corriger") },
    { id: "academic", label: tk("Style académique") },
    { id: "expand", label: tk("Développer") },
    { id: "shorten", label: tk("Raccourcir") },
    { id: "translate_en", label: tk("En anglais") },
    { id: "translate_fr", label: tk("En français") },
    { id: "summarize", label: tk("Résumer") },
  ];
  let task = $state<RewriteTask>(req.task);
  let out = $state("");
  let result = $state("");
  let error = $state("");
  let running = $state(false);
  let meta = $state("");

  async function run(tk: RewriteTask) {
    task = tk;
    out = "";
    result = "";
    error = "";
    running = true;
    try {
      const r = await api.aiRewrite(tk, req.text, (d) => (out += d));
      result = r.text;
      meta = `${r.model} · ${r.input_tokens.toLocaleString()} / ${r.output_tokens.toLocaleString()}`;
    } catch (e) {
      error = translateError(e);
    } finally {
      running = false;
    }
  }

  async function accept() {
    if (!result) return;
    const insert = task === "summarize" ? await api.aiSummaryCallout(result) : result;
    req.apply(insert, task === "summarize");
    app.rewriteFor = null;
  }

  function close() {
    if (running) api.aiCancel();
    app.rewriteFor = null;
  }
  onDestroy(() => {
    if (running) api.aiCancel();
  });

  if (app.settings?.ai_enabled) run(req.task);
</script>

<Modal title={t("Assistant de rédaction")} width={900} onclose={close}>
  <div class="body">
    {#if !app.settings?.ai_enabled}
      <p>{t("L'IA est désactivée. Active-la et enregistre ta clé API dans Réglages › Intelligence artificielle.")}</p>
    {:else}
      <div class="tasks" role="toolbar">
        {#each TASKS as x (x.id)}
          <button class="chip" class:active={task === x.id} disabled={running} onclick={() => run(x.id)}>{t(x.label)}</button>
        {/each}
      </div>
      <div class="cols">
        <div>
          <p class="lbl">{t("Original")}</p>
          <pre class="txt">{req.text}</pre>
        </div>
        <div>
          <p class="lbl">{task === "summarize" ? t("Résumé proposé") : t("Proposition")}</p>
          <pre class="txt proposal">{result || out || (running ? t("Rédaction…") : "")}</pre>
        </div>
      </div>
      {#if error}<p class="error">{error}</p>{/if}
      <p class="hint">{t("Citations [@…], liens [[…]] et formules sont protégés : l'IA ne peut ni les modifier ni en ajouter.")} {meta}</p>
    {/if}
  </div>
  {#snippet footer()}
    {#if !app.settings?.ai_enabled}
      <button class="btn" onclick={close}>{t("Fermer")}</button>
      <button class="btn primary" onclick={() => { close(); app.openSettings("ia"); }}>{t("Ouvrir les réglages")}</button>
    {:else if running}
      <button class="btn" onclick={() => api.aiCancel()}>{t("Arrêter")}</button>
    {:else}
      <button class="btn" onclick={close}>{t("Refuser")}</button>
      <button class="btn primary" disabled={!result} onclick={accept}>{task === "summarize" ? t("Insérer le résumé") : t("Remplacer la sélection")}</button>
    {/if}
  {/snippet}
</Modal>

<style>
  .body {
    padding: 14px 22px;
  }
  .tasks {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 12px;
  }
  .chip {
    padding: 3px 10px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: var(--bg-raised);
    color: var(--text-soft);
    font: 12.5px var(--font-ui);
    cursor: pointer;
  }
  .chip.active {
    border-color: var(--accent);
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .cols {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
  }
  .lbl {
    margin: 0 0 4px;
    font-size: 12px;
    color: var(--text-faint);
  }
  .txt {
    height: 42vh;
    overflow: auto;
    margin: 0;
    padding: 12px 14px;
    background: var(--bg-sunk);
    border-radius: var(--radius);
    font: 14px/1.6 var(--font-editor);
    white-space: pre-wrap;
    user-select: text;
  }
  .proposal {
    background: var(--bg-raised);
    border: 1px solid var(--line);
  }
  .hint {
    margin: 10px 0 0;
    font-size: 12px;
    color: var(--text-soft);
  }
  .error {
    color: var(--danger);
    margin: 10px 0 0;
  }
</style>
