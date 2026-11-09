<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, tn, translateError } from "../lib/i18n.svelte";
  import type { MethodRow } from "../lib/types";
  import Icon from "./Icon.svelte";

  // Methodologies: the methods your documents use (built-in catalogue and your
  // method notes), with the articles that use them. Detection without AI.
  let { visible = true }: { visible?: boolean } = $props();
  let rows = $state<MethodRow[] | null>(null);
  let error = $state("");
  let filter = $state("");
  let open = $state<Record<string, boolean>>({});
  $effect(() => {
    void app.revision;
    if (!visible) return;
    api.methodsOverview().then((r) => (rows = r)).catch((e) => (error = translateError(e)));
  });
  const fold = (s: string) => s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
  const shown = $derived(
    (rows ?? []).filter((r) => !filter.trim() || fold(`${r.name} ${r.aliases.join(" ")} ${r.documents.map((d) => `${d.citekey} ${d.title}`).join(" ")}`).includes(fold(filter.trim()))),
  );
  const max = $derived(Math.max(1, ...(rows ?? []).map((r) => r.documents.length)));

  async function createNote(r: MethodRow) {
    try {
      const p = await api.createMethodNote(r.name, r.aliases);
      await app.refresh();
      await app.openNote(p, { newTab: true });
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }
  async function newMethod() {
    const name = await app.ask(t("Nom de la méthode"), "");
    if (!name?.trim()) return;
    const al = await app.ask(t("Autres noms, séparés par des virgules (pour la retrouver dans les fiches)"), "");
    if (al === null) return;
    await createNote({ name: name.trim(), aliases: al.split(",").map((x) => x.trim()).filter(Boolean), path: "", documents: [] });
  }
</script>

<div class="meth">
  <div class="top">
    <h2>{t("Méthodologies")}</h2>
    <span class="grow"></span>
    <input class="input" placeholder={t("Filtrer : méthode, article, clé…")} bind:value={filter} aria-label={t("Filtrer les méthodes")} />
    <button class="btn small" onclick={newMethod}><Icon name="plus" size={13} /> {t("Méthode")}</button>
  </div>
  <p class="hint">{t("Repérées dans la section « Méthodes » de tes fiches et dans le titre, le résumé et les tags de tes sources ; une fiche qui renvoie à une note de méthode compte aussi. Sans IA.")}</p>
  {#if error}<p class="error">{error}</p>{/if}
  {#if !rows && !error}<p class="loading" role="status">{t("Analyse de tes notes…")}</p>{/if}
  {#if rows}
    {#if !rows.length}
      <p class="empty">{t("Aucune méthode repérée pour l'instant : remplis la section « Méthodes » de tes fiches, ou complète les résumés de tes sources.")}</p>
    {/if}
    <ul class="list">
      {#each shown as r (r.name)}
        <li class="row">
          <div class="head">
            <button class="name" onclick={() => (open[r.name] = !open[r.name])} aria-expanded={!!open[r.name]}>
              <span class="chev" class:open={open[r.name]}><Icon name="chevron" size={12} stroke={2} /></span>
              {r.name}
            </button>
            <span class="count">{tn(r.documents.length, "{n} article", "{n} articles")}</span>
            <span class="bar"><span style:width="{(r.documents.length / max) * 100}%"></span></span>
            {#if r.path}
              <button class="btn small ghost" onclick={() => app.openNote(r.path, { newTab: true })}>{t("Ouvrir la note")}</button>
            {:else}
              <button class="btn small ghost" title={t("Principe, forces, limites, usage dans ta thèse")} onclick={() => createNote(r)}>{t("Créer la note")}</button>
            {/if}
          </div>
          {#if open[r.name]}
            <ul class="docs">
              {#each r.documents as d (d.citekey)}
                <li>
                  <button class="doc" onclick={() => d.path && app.openNote(d.path, { newTab: true })}>
                    <span class="ck">@{d.citekey}</span>
                    <span class="dt">{d.title}</span>
                    {#if d.year}<span class="faint">{d.year}</span>{/if}
                  </button>
                </li>
              {:else}
                <li class="faint">{t("Aucun article pour l'instant.")}</li>
              {/each}
            </ul>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .meth {
    height: 100%;
    overflow: auto;
    padding: 20px 26px 30px;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .grow {
    flex: 1;
  }
  h2 {
    margin: 0;
    font-size: 20px;
  }
  .top .input {
    width: min(280px, 100%);
  }
  .hint {
    margin: 6px 0 14px;
    font-size: 12.5px;
    color: var(--text-soft);
  }
  .list,
  .docs {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .row {
    border-bottom: 1px solid var(--line);
    padding: 6px 0;
  }
  .head {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto 120px auto;
    align-items: center;
    gap: 10px;
  }
  .name {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    border: 0;
    background: none;
    padding: 2px 0;
    font: 600 13.5px var(--font-ui);
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .chev {
    display: inline-flex;
    transition: transform 0.15s var(--ease);
    color: var(--text-faint);
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .count {
    font-size: 12px;
    color: var(--text-soft);
    white-space: nowrap;
  }
  .bar {
    height: 6px;
    border-radius: 3px;
    background: var(--bg-sunk);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--accent);
    opacity: 0.8;
  }
  .docs {
    padding: 4px 0 4px 22px;
  }
  .doc {
    display: flex;
    align-items: baseline;
    gap: 8px;
    width: 100%;
    min-width: 0;
    border: 0;
    background: none;
    padding: 3px 0;
    text-align: left;
    font: 13px var(--font-ui);
    color: var(--text);
    cursor: pointer;
  }
  .doc:hover .dt {
    color: var(--accent-text);
  }
  .ck {
    font: 12px var(--font-mono);
    color: var(--accent-text);
    flex-shrink: 0;
  }
  .dt {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .faint {
    color: var(--text-faint);
    font-size: 12px;
  }
  .empty {
    color: var(--text-soft);
  }
  .error {
    color: var(--danger);
  }
</style>
