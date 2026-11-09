<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, tn, translateError } from "../lib/i18n.svelte";
  import type { ConceptSuggestion } from "../lib/types";
  import Modal from "./Modal.svelte";

  // AI suggestions for a note, always to approve: concepts (linked concept
  // notes) or tags (preferably those already in the vault).
  const req = app.suggestFor!;
  let concepts = $state<(ConceptSuggestion & { on: boolean })[]>([]);
  let tags = $state<{ tag: string; on: boolean; known: boolean }[]>([]);
  let busy = $state(true);
  let error = $state("");
  let applying = $state(false);
  let linkBack = $state(true);
  const knownTags = new Set(app.tags.map(([tg]) => tg.toLowerCase()));
  const current = new Set((app.noteByPath(req.path)?.tags ?? []).map((x) => x.toLowerCase()));

  (async () => {
    if (!app.settings?.ai_enabled) {
      busy = false;
      return;
    }
    try {
      if (req.kind === "concepts") {
        concepts = (await api.aiConcepts(req.path)).map((c) => ({ ...c, on: !app.noteByPath(`Concepts/${c.name}.md`) }));
      } else {
        tags = (await api.aiTags(req.path)).filter((tg) => !current.has(tg.toLowerCase())).map((tg) => ({ tag: tg, on: true, known: knownTags.has(tg.toLowerCase()) }));
      }
    } catch (e) {
      error = translateError(e);
    } finally {
      busy = false;
    }
  })();

  async function apply() {
    applying = true;
    try {
      if (req.kind === "concepts") {
        const made: string[] = [];
        for (const c of concepts.filter((x) => x.on && x.name.trim())) {
          const r = await api.createConceptNote(c.name.trim(), c.definition, req.path);
          made.push(r.path);
        }
        if (linkBack && made.length) {
          const links = made.map((p) => `[[${p.split("/").pop()!.replace(/\.md$/, "")}]]`).join(" · ");
          const ed = app.activeNotePath === req.path ? (await import("../lib/editor/registry")).editors.active() : undefined;
          if (ed) ed.insert(`\n\n**${t("Concepts")} :** ${links}\n`);
        }
        await app.refresh();
        app.toast(tn(made.length, "{n} note de concept prête", "{n} notes de concept prêtes"), "success");
      } else {
        const chosen = tags.filter((x) => x.on).map((x) => x.tag);
        const all = [...(app.noteByPath(req.path)?.tags ?? []), ...chosen];
        await api.setProperty(req.path, "tags", [...new Set(all)]);
        await app.reloadOpen([req.path]);
        await app.refresh();
        app.toast(tn(chosen.length, "{n} tag ajouté", "{n} tags ajoutés"), "success");
      }
      app.suggestFor = null;
    } catch (e) {
      error = translateError(e);
    } finally {
      applying = false;
    }
  }
</script>

<Modal title={req.kind === "concepts" ? t("Concepts proposés") : t("Tags proposés")} width={640} onclose={() => (app.suggestFor = null)}>
  <div class="body">
    {#if !app.settings?.ai_enabled}
      <p>{t("L'IA est désactivée. Active-la et enregistre ta clé API dans Réglages › Intelligence artificielle.")}</p>
    {:else if busy}
      <p class="faint">{t("Lecture de la note…")}</p>
    {:else if error}
      <p class="error">{error}</p>
    {:else if req.kind === "concepts"}
      <p class="hint">{t("Coche les concepts à créer dans le dossier Concepts ; chaque note reçoit la définition et un lien vers cette note. Les noms se modifient.")}</p>
      {#each concepts as c, i (i)}
        <div class="row">
          <input type="checkbox" bind:checked={c.on} aria-label={t("Créer {name}", { name: c.name })} />
          <div class="col">
            <input class="input name" bind:value={c.name} />
            {#if c.definition}<p class="def">{c.definition}</p>{:else}<p class="def faint">{t("Pas de définition dans la note : à compléter.")}</p>{/if}
          </div>
        </div>
      {:else}
        <p class="faint">{t("Aucun concept trouvé.")}</p>
      {/each}
      {#if concepts.length}<label class="check"><input type="checkbox" bind:checked={linkBack} /> {t("Ajouter les liens vers ces concepts dans la note")}</label>{/if}
    {:else}
      <p class="hint">{t("Les tags déjà utilisés dans ton coffre sont privilégiés. Clique pour retirer ou remettre un tag.")}</p>
      <div class="tags">
        {#each tags as tg (tg.tag)}
          <button class="tag" class:on={tg.on} onclick={() => (tg.on = !tg.on)} title={tg.known ? t("Tag déjà utilisé") : t("Nouveau tag")}>
            #{tg.tag}{#if !tg.known}<span class="new">{t("nouveau")}</span>{/if}
          </button>
        {:else}
          <p class="faint">{t("Aucun tag à ajouter.")}</p>
        {/each}
      </div>
    {/if}
  </div>
  {#snippet footer()}
    {#if !app.settings?.ai_enabled}
      <button class="btn" onclick={() => (app.suggestFor = null)}>{t("Fermer")}</button>
      <button class="btn primary" onclick={() => { app.suggestFor = null; app.openSettings("ia"); }}>{t("Ouvrir les réglages")}</button>
    {:else}
    <button class="btn" onclick={() => (app.suggestFor = null)}>{t("Annuler")}</button>
    <button class="btn primary" disabled={busy || applying || (req.kind === "concepts" ? !concepts.some((c) => c.on) : !tags.some((x) => x.on))} onclick={apply}>
      {req.kind === "concepts" ? t("Créer les notes de concept") : t("Ajouter les tags")}
    </button>
    {/if}
  {/snippet}
</Modal>

<style>
  .body {
    padding: 14px 22px;
    max-height: 62vh;
    overflow: auto;
  }
  .hint {
    font-size: 12.5px;
    color: var(--text-soft);
    margin: 0 0 10px;
  }
  .row {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 8px 0;
    border-bottom: 1px solid var(--line);
  }
  .row > input {
    margin-top: 9px;
    accent-color: var(--accent);
  }
  .col {
    flex: 1;
    min-width: 0;
  }
  .name {
    width: 100%;
    font-weight: 600;
  }
  .def {
    margin: 4px 0 0;
    font-size: 12.5px;
    color: var(--text-soft);
  }
  .faint {
    color: var(--text-faint);
  }
  .error {
    color: var(--danger);
  }
  .check {
    display: flex;
    gap: 8px;
    align-items: center;
    margin-top: 12px;
    font-size: 13px;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .tag {
    padding: 4px 10px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: var(--bg-raised);
    color: var(--text-faint);
    font: 13px var(--font-ui);
    cursor: pointer;
    text-decoration: line-through;
  }
  .tag.on {
    color: var(--accent-text);
    border-color: var(--accent);
    background: var(--accent-soft);
    text-decoration: none;
  }
  .new {
    margin-left: 6px;
    font-size: 10.5px;
    color: var(--text-faint);
  }
</style>
