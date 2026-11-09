<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, tn, translateError } from "../lib/i18n.svelte";
  import type { EntityKind, EntityRow } from "../lib/types";
  import Icon from "./Icon.svelte";

  // Named entities of your notes: people (authors of your library), organisations,
  // methods, technologies, places. Rule-based recognition, no AI.
  let { visible = true }: { visible?: boolean } = $props();
  let rows = $state<EntityRow[] | null>(null);
  let kind = $state<EntityKind | "">("");
  let filter = $state("");
  let open = $state<Record<string, boolean>>({});
  let error = $state("");
  $effect(() => {
    void app.revision;
    if (!visible) return;
    api.entities().then((r) => (rows = r)).catch((e) => (error = translateError(e)));
  });
  const KINDS: [EntityKind, string][] = [
    ["personne", t("Personnes")],
    ["organisation", t("Organisations")],
    ["methode", t("Méthodes")],
    ["technologie", t("Technologies")],
    ["lieu", t("Lieux")],
  ];
  // An entity carries its type in the singular; the filters stay plural.
  const LABEL: Record<EntityKind, string> = {
    personne: t("Personne"),
    organisation: t("Organisation"),
    methode: t("Méthode"),
    technologie: t("Technologie"),
    lieu: t("Lieu"),
  };
  const fold = (s: string) => s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
  const shown = $derived((rows ?? []).filter((r) => (!kind || r.kind === kind) && (!filter.trim() || fold(r.name).includes(fold(filter.trim())))));
  const count = (k: EntityKind) => (rows ?? []).filter((r) => r.kind === k).length;
  function search(name: string) {
    app.searchQuery = `"${name}"`;
    app.leftPanel = "search";
    app.leftOpen = true;
  }
</script>

<div class="ev">
  <div class="top">
    <h2>{t("Entités")}</h2>
    <span class="grow"></span>
    <input class="input" placeholder={t("Filtrer les entités")} aria-label={t("Filtrer les entités")} bind:value={filter} />
  </div>
  <p class="hint">{t("Personnes (auteurs de ta bibliothèque), organisations, méthodes, technologies et lieux repérés dans tes notes, par règles et sans IA. Les sigles qui reviennent dans au moins trois notes sont proposés comme organisations.")}</p>
  <div class="kinds" role="tablist" aria-label={t("Type d'entité")}>
    <button role="tab" aria-selected={kind === ""} class:on={kind === ""} onclick={() => (kind = "")}>{t("Toutes")} <span>{rows?.length ?? 0}</span></button>
    {#each KINDS as [k, label] (k)}
      <button role="tab" aria-selected={kind === k} class:on={kind === k} onclick={() => (kind = k)}>{label} <span>{count(k)}</span></button>
    {/each}
  </div>
  {#if error}<p class="error">{error}</p>{/if}
  {#if !rows && !error}<p class="loading" role="status">{t("Analyse de tes notes…")}</p>{/if}
  <ul class="list">
    {#each shown as r (r.kind + r.name)}
      <li>
        <div class="row">
          <button class="name" onclick={() => (open[r.kind + r.name] = !open[r.kind + r.name])} aria-expanded={!!open[r.kind + r.name]}>
            <span class="chev" class:open={open[r.kind + r.name]}><Icon name="chevron" size={12} stroke={2} /></span>
            {r.name}
          </button>
          <span class="k">{LABEL[r.kind]}</span>
          <span class="c">{tn(r.notes.length, "{n} note", "{n} notes")}</span>
          <button class="icon-btn" title={t("Chercher « {name} » dans les notes", { name: r.name })} aria-label={t("Chercher « {name} » dans les notes", { name: r.name })} onclick={() => search(r.name)}><Icon name="search" size={13} /></button>
        </div>
        {#if open[r.kind + r.name]}
          <ul class="notes">
            {#each r.notes as [p, title] (p)}<li><button onclick={() => app.openNote(p, { newTab: true })}>{title}</button></li>{/each}
          </ul>
        {/if}
      </li>
    {:else}
      <li class="empty">{rows ? t("Aucune entité pour ce filtre.") : ""}</li>
    {/each}
  </ul>
</div>

<style>
  .ev {
    height: 100%;
    overflow: auto;
    padding: 20px 26px 30px;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .grow {
    flex: 1;
  }
  h2 {
    margin: 0;
    font-size: 20px;
  }
  .top .input {
    width: min(260px, 100%);
  }
  .hint {
    margin: 6px 0 10px;
    font-size: 12.5px;
    color: var(--text-soft);
  }
  .kinds {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 10px;
  }
  .kinds button {
    border: 1px solid var(--line);
    border-radius: 999px;
    background: none;
    padding: 2px 10px;
    font: 12.5px var(--font-ui);
    color: var(--text-soft);
    cursor: pointer;
  }
  .kinds button span {
    color: var(--text-faint);
  }
  .kinds button.on {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--text);
  }
  .list,
  .notes {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto 80px auto;
    align-items: center;
    gap: 10px;
    padding: 5px 0;
    border-bottom: 1px solid var(--line);
  }
  .name {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    border: 0;
    background: none;
    padding: 0;
    font: 600 13.5px var(--font-ui);
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .chev {
    display: inline-flex;
    color: var(--text-faint);
    transition: transform 0.15s var(--ease);
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .k {
    font-size: 11px;
    padding: 0 8px;
    border-radius: 999px;
    background: var(--bg-sunk);
    color: var(--text-soft);
  }
  .c {
    font-size: 12px;
    color: var(--text-soft);
    text-align: right;
  }
  .notes {
    padding: 4px 0 6px 22px;
  }
  .notes button {
    border: 0;
    background: none;
    padding: 2px 0;
    font: 13px var(--font-ui);
    color: var(--text);
    cursor: pointer;
    text-align: left;
  }
  .notes button:hover {
    color: var(--accent-text);
  }
  .empty {
    color: var(--text-faint);
    font-size: 12.5px;
    padding: 8px 0;
  }
  .error {
    color: var(--danger);
  }
</style>
