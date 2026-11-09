<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, tn, translateError } from "../lib/i18n.svelte";
  import type { TaxonomyOverview } from "../lib/types";

  // Personal taxonomy: the "Taxonomie" note (nested list) becomes a
  // hierarchical classification; each term is a tag. Filing suggestions for
  // notes that mention a term without carrying its tag.
  let { visible = true }: { visible?: boolean } = $props();
  let tax = $state<TaxonomyOverview | null>(null);
  let sel = $state("");
  let proposals = $state<{ path: string; title: string; tags: string[]; on: boolean }[] | null>(null);
  let error = $state("");
  $effect(() => {
    void app.revision;
    if (!visible) return;
    api.taxonomy().then((x) => (tax = x)).catch((e) => (error = translateError(e)));
  });
  const node = $derived(tax?.nodes.find((n) => n.tag === sel) ?? null);

  async function create() {
    try {
      const p = await api.taxonomyCreate();
      await app.refresh();
      await app.openNote(p, { newTab: true, otherPane: true });
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }
  async function propose() {
    try {
      proposals = (await api.taxonomyProposals()).map(([path, title, tags]) => ({ path, title, tags, on: true }));
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }
  async function apply() {
    let n = 0;
    for (const p of proposals ?? []) {
      if (!p.on) continue;
      try {
        if (await api.addTags(p.path, p.tags)) n++;
      } catch {
        /* unreadable note: skipped */
      }
    }
    proposals = null;
    await app.refresh();
    app.toast(tn(n, "{n} note classée", "{n} notes classées"), "success");
  }
</script>

<div class="tx">
  <div class="top">
    <h2>{t("Taxonomie")}</h2>
    <span class="grow"></span>
    {#if tax?.exists}
      <button class="btn small ghost" onclick={() => app.openNote(tax!.path, { newTab: true, otherPane: true })}>{t("Modifier la taxonomie")}</button>
      <button class="btn small" onclick={propose}>{t("Proposer des classements")}</button>
    {/if}
  </div>
  {#if error}<p class="error">{error}</p>{/if}
  {#if !tax && !error}<p class="loading" role="status">{t("Lecture de la taxonomie…")}</p>{/if}
  {#if tax && !tax.exists}
    <div class="empty">
      <p>{t("Ta taxonomie est une note « Taxonomie » : une liste imbriquée de termes, avec des alias entre parenthèses. Chaque terme devient un tag hiérarchique (fraude/comptes-mules).")}</p>
      <button class="btn primary" onclick={create}>{t("Créer la taxonomie à partir de mes tags")}</button>
    </div>
  {:else if tax}
    <div class="cols">
      <ul class="tree" aria-label={t("Termes de la taxonomie")}>
        {#each tax.nodes as n (n.tag)}
          <li style:padding-left="{n.depth * 18}px">
            <button class:on={sel === n.tag} onclick={() => (sel = n.tag)}>
              <span class="lab">{n.label}</span>
              <span class="cnt">{n.notes.length}</span>
            </button>
          </li>
        {:else}
          <li class="faint">{t("La note « Taxonomie » ne contient pas encore de liste.")}</li>
        {/each}
      </ul>
      <section class="pane">
        {#if proposals}
          <h3>{t("Propositions de classement")} <span class="faint">{proposals.length}</span></h3>
          <p class="sub">{t("Ces notes mentionnent un terme (ou un alias) sans en porter le tag. Décoche ce que tu ne veux pas.")}</p>
          {#each proposals as p (p.path)}
            <label class="prop"><input type="checkbox" bind:checked={p.on} /> <span class="pt">{p.title}</span> {#each p.tags as tg (tg)}<span class="tag">#{tg}</span>{/each}</label>
          {:else}
            <p class="faint">{t("Rien à proposer : tes notes sont déjà classées.")}</p>
          {/each}
          <div class="acts">
            <button class="btn small" onclick={() => (proposals = null)}>{t("Annuler")}</button>
            {#if proposals.length}<button class="btn small primary" onclick={apply}>{t("Classer les notes cochées")}</button>{/if}
          </div>
        {:else if node}
          <h3>{node.label} <span class="faint">#{node.tag}</span></h3>
          {#if node.terms.length > 1}<p class="sub">{t("Aussi cherché sous : {list}", { list: node.terms.slice(1).join(", ") })}</p>{/if}
          <ul class="notes">
            {#each node.notes as [p, title] (p)}<li><button onclick={() => app.openNote(p, { newTab: true })}>{title}</button></li>{:else}<li class="faint">{t("Aucune note classée ici.")}</li>{/each}
          </ul>
        {:else}
          <p class="faint">{t("Choisis un terme pour voir ses notes, ou « Proposer des classements ».")}</p>
        {/if}
      </section>
    </div>
  {/if}
</div>

<style>
  .tx {
    height: 100%;
    overflow: auto;
    padding: 20px 26px 30px;
    container-type: inline-size;
  }
  /* Narrow panel (split screen): the tree, then the notes, one below the other. */
  @container (max-width: 640px) {
    .cols {
      grid-template-columns: minmax(0, 1fr) !important;
    }
    .top {
      flex-wrap: wrap;
    }
  }
  .top {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
  }
  .grow {
    flex: 1;
  }
  h2 {
    margin: 0;
    font-size: 20px;
  }
  h3 {
    margin: 0 0 4px;
    font-size: 14px;
  }
  .cols {
    display: grid;
    grid-template-columns: minmax(160px, 260px) minmax(0, 1fr);
    gap: 20px;
  }
  .tree,
  .notes {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .tree button {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    border: 1px solid transparent;
    border-radius: var(--radius-s);
    background: none;
    padding: 3px 6px;
    font: 13px var(--font-ui);
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .tree button:hover {
    background: var(--bg-soft);
  }
  .tree button.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .lab {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cnt {
    font-size: 11.5px;
    color: var(--text-faint);
    font-variant-numeric: tabular-nums;
  }
  .pane {
    min-width: 0;
  }
  .sub {
    margin: 0 0 8px;
    font-size: 12px;
    color: var(--text-soft);
  }
  .notes button {
    border: 0;
    background: none;
    padding: 3px 0;
    font: 13px var(--font-ui);
    color: var(--text);
    cursor: pointer;
    text-align: left;
  }
  .notes button:hover {
    color: var(--accent-text);
  }
  .prop {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    padding: 4px 0;
    font-size: 13px;
    border-bottom: 1px solid var(--line);
    cursor: pointer;
  }
  .pt {
    font-weight: 600;
  }
  .tag {
    font-size: 11.5px;
    padding: 0 7px;
    border-radius: 999px;
    background: var(--bg-sunk);
    color: var(--text-soft);
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 10px;
  }
  .empty {
    max-width: 560px;
    margin: 32px auto 0;
    color: var(--text-soft);
    font-size: 13px;
  }
  .faint {
    color: var(--text-faint);
    font-size: 12px;
  }
  .error {
    color: var(--danger);
  }
</style>
