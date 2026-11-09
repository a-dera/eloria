<script lang="ts">
  import { onDestroy } from "svelte";
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, tn, translateError } from "../lib/i18n.svelte";
  import type { AiFicheDraft } from "../lib/types";
  import Modal from "./Modal.svelte";

  const target = app.aiFicheFor!;
  let text = $state("");
  let draft = $state<AiFicheDraft | null>(null);
  let error = $state("");
  let running = $state(false);
  let saving = $state(false);
  let out = $state<HTMLPreElement>();

  async function generate() {
    text = "";
    draft = null;
    error = "";
    running = true;
    try {
      draft = await api.aiFicheGenerate(target.key, (d) => {
        text += d;
        // Follows the text being written, unless the user scrolled up to read.
        if (out && out.scrollHeight - out.scrollTop - out.clientHeight < 80) requestAnimationFrame(() => out && (out.scrollTop = out.scrollHeight));
      });
    } catch (e) {
      error = translateError(e);
    } finally {
      running = false;
    }
  }

  async function save() {
    if (!draft) return;
    saving = true;
    try {
      const path = await api.aiFicheSave($state.snapshot(draft) as AiFicheDraft, target.path);
      await app.refresh();
      // The reading note may already be open: its editor picks up the filled text.
      await app.reloadOpen([path]);
      app.fiches = await api.fichesMap();
      app.aiFicheFor = null;
      await app.openNote(path, { newTab: true });
      app.toast(t("Fiche « méthode Eloria » enregistrée"), "success");
    } catch (e) {
      error = translateError(e);
    } finally {
      saving = false;
    }
  }

  function close() {
    if (running) api.aiCancel();
    app.aiFicheFor = null;
  }

  onDestroy(() => {
    if (running) api.aiCancel();
  });

  if (app.settings?.ai_enabled) generate();

  const sourceLabel = (s: string) => (s === "pdf" ? t("le PDF complet") : s === "texte" ? t("le texte intégral") : t("le résumé seulement"));
</script>

<Modal title={t("Fiche « méthode Eloria »")} width={860} onclose={close}>
  <div class="body">
    <p class="what"><b>{target.title}</b></p>
    {#if !app.settings?.ai_enabled}
      <p>{t("L'IA est désactivée. Active-la et enregistre ta clé API dans Réglages › Intelligence artificielle.")}</p>
    {:else}
      <pre class="stream" bind:this={out}>{text || (running ? t("Lecture de l'article et rédaction…") : "")}</pre>
      {#if error}<p class="error">{error}</p>{/if}
      {#if draft && target.path}<p class="hint-small">{t("La structure de la fiche sera remplacée par ce texte ; la version actuelle reste dans l'historique.")}</p>{/if}
      {#if draft}
        <ul class="facts">
          <li>{t("Lu : {what}", { what: sourceLabel(draft.source) })} · {draft.model}{draft.fell_back ? ` (${t("modèle de repli")})` : ""}</li>
          <li>
            {tn(draft.refs_in_library, "{n} référence dans ta bibliothèque", "{n} références dans ta bibliothèque")} ·
            {tn(draft.refs_in_article, "{n} retrouvée dans l'article", "{n} retrouvées dans l'article")}
            {#if draft.refs_unverified} · <span class="warn">{tn(draft.refs_unverified, "{n} à vérifier", "{n} à vérifier")}</span>{/if}
          </li>
          {#if draft.tags.length}<li>{t("Tags proposés : {list}", { list: draft.tags.map((x) => `#${x}`).join(" ") })}</li>{/if}
          <li class="faint">{t("{inp} jetons lus, {out} écrits", { inp: draft.input_tokens.toLocaleString(), out: draft.output_tokens.toLocaleString() })}</li>
          {#if draft.truncated}<li class="warn">{t("Réponse coupée par la limite de longueur : régénère ou relis attentivement la fin.")}</li>{/if}
        </ul>
      {/if}
    {/if}
  </div>
  {#snippet footer()}
    {#if running}
      <button class="btn" onclick={() => api.aiCancel()}>{t("Arrêter")}</button>
    {:else if !app.settings?.ai_enabled}
      <button class="btn" onclick={close}>{t("Fermer")}</button>
      <button class="btn primary" onclick={() => { app.aiFicheFor = null; app.settingsOpen = true; }}>{t("Ouvrir les réglages")}</button>
    {:else}
      <button class="btn" onclick={generate}>{t("Régénérer")}</button>
      <button class="btn" onclick={close}>{t("Fermer")}</button>
      <button class="btn primary" disabled={!draft || saving} onclick={save}>{target.path ? t("Remplir la fiche") : t("Enregistrer dans une nouvelle fiche")}</button>
    {/if}
  {/snippet}
</Modal>

<style>
  .body {
    padding: 16px 22px;
  }
  .what {
    margin: 0 0 10px;
  }
  .stream {
    height: 52vh;
    overflow: auto;
    margin: 0;
    padding: 14px 16px;
    background: var(--bg-sunk);
    border-radius: var(--radius);
    font: 13px/1.55 var(--font-mono);
    white-space: pre-wrap;
    user-select: text;
  }
  .hint-small {
    margin: 10px 0 0;
    font-size: 12.5px;
    color: var(--text-soft);
  }
  .facts {
    margin: 12px 0 0;
    padding-left: 18px;
    font-size: 12.5px;
    color: var(--text-soft);
    line-height: 1.7;
  }
  .faint {
    color: var(--text-faint);
  }
  .warn {
    color: var(--danger);
  }
  .error {
    color: var(--danger);
    margin: 10px 0 0;
  }
</style>
