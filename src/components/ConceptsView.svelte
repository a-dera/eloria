<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, tn, translateError } from "../lib/i18n.svelte";
  import { plainMd } from "../lib/util";
  import type { ConceptDetail, ConceptRow } from "../lib/types";
  import Icon from "./Icon.svelte";

  // Concept base: each concept, its definition, what talks about it; in detail, the
  // timeline (how it appears in your readings and notes over time),
  // how its definition evolved, and its knowledge card.
  let { visible = true }: { visible?: boolean } = $props();
  let rows = $state<ConceptRow[] | null>(null);
  let sel = $state<string>("");
  let detail = $state<ConceptDetail | null>(null);
  let filter = $state("");
  let error = $state("");
  $effect(() => {
    void app.revision;
    if (!visible) return;
    api.conceptsOverview().then((r) => {
      rows = r;
      if (!sel && r.length) sel = r[0].path;
    }).catch((e) => (error = translateError(e)));
  });
  $effect(() => {
    void app.revision;
    const p = sel;
    if (!p || !visible) return;
    api.conceptDetail(p).then((d) => (detail = d)).catch((e) => (error = translateError(e)));
  });
  const fold = (s: string) => s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
  const shown = $derived((rows ?? []).filter((r) => !filter.trim() || fold(`${r.title} ${r.aliases.join(" ")} ${r.definition}`).includes(fold(filter.trim()))));
  const KIND: Record<string, string> = { source: t("source"), fiche: t("fiche"), concept: t("concept"), question: t("question"), chapitre: t("chapitre"), note: t("note") };

  async function card() {
    if (!detail) return;
    try {
      const p = await api.knowledgeCard(detail.path);
      await app.refresh();
      await app.openNote(p, { newTab: true });
      app.toast(t("Fiche de connaissance à jour : ton texte hors du bloc généré est gardé"), "success");
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }
  async function newConcept() {
    const name = await app.ask(t("Nom du concept"), "");
    if (!name?.trim()) return;
    const def = await app.ask(t("Définition (une phrase, tu pourras la reprendre)"), "");
    if (def === null) return;
    try {
      const r = await api.createConceptNote(name.trim(), def, null);
      await app.refresh();
      sel = r.path;
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }
</script>

<div class="cv">
  <aside class="list">
    <div class="lh">
      <h2>{t("Concepts")}</h2>
      <button class="icon-btn" title={t("Nouveau concept")} aria-label={t("Nouveau concept")} onclick={newConcept}><Icon name="plus" size={15} /></button>
    </div>
    <input class="input" placeholder={t("Filtrer les concepts")} aria-label={t("Filtrer les concepts")} bind:value={filter} />
    {#if error}<p class="error">{error}</p>{/if}
    {#if !rows && !error}<p class="loading" role="status">{t("Analyse de tes notes…")}</p>{/if}
    {#each shown as r (r.path)}
      <button class="row" class:on={r.path === sel} onclick={() => (sel = r.path)}>
        <span class="rt">{r.title}</span>
        <span class="rd">{plainMd(r.definition) || t("(sans définition)")}</span>
        <span class="rc">{tn(r.sources, "{n} source", "{n} sources")} · {tn(r.notes, "{n} note", "{n} notes")}</span>
      </button>
    {:else}
      <p class="empty">{rows ? t("Aucun concept. Crée-en un (bouton +) ou depuis une note avec « Concepts de cette note ».") : ""}</p>
    {/each}
  </aside>

  <section class="detail">
    {#if detail}
      <div class="dh">
        <h2>{detail.title}</h2>
        <span class="grow"></span>
        <button class="btn small ghost" onclick={() => app.openNote(detail!.path, { newTab: true })}>{t("Ouvrir la note")}</button>
        {#if detail.card}<button class="btn small ghost" onclick={() => app.openNote(detail!.card, { newTab: true })}>{t("Ouvrir la fiche de connaissance")}</button>{/if}
        <button class="btn small primary" onclick={card}>{detail.card ? t("Mettre à jour la fiche de connaissance") : t("Créer la fiche de connaissance")}</button>
      </div>
      {#if detail.aliases.length}<p class="faint">{t("Aussi appelé : {list}", { list: detail.aliases.join(", ") })}</p>{/if}
      <p class="def">{plainMd(detail.definition) || t("Pas encore de définition : écris-la dans la note du concept (encadré « [!definition] »).")}</p>

      {#if detail.definitions.length > 1}
        <h3>{t("Évolution de la définition")}</h3>
        <ol class="defs">
          {#each detail.definitions as d, i (i)}<li><span class="date">{d.date}</span><span>{plainMd(d.definition)}</span></li>{/each}
        </ol>
      {/if}

      <div class="cols">
        <div>
          <h3>{t("Chronologie")} <span class="faint">{detail.timeline.length}</span></h3>
          <p class="sub">{t("Les notes qui en parlent, de la première lecture à aujourd'hui.")}</p>
          <ol class="tl">
            {#each detail.timeline as m (m.path)}
              <li>
                <span class="date">{m.date}</span>
                <button class="tlb" onclick={() => app.openNote(m.path, { newTab: true })}>
                  <span class="k k-{m.kind}">{KIND[m.kind] ?? m.kind}</span>
                  <span class="tt">{m.title}</span>
                  {#if m.linked}<span class="lk" title={t("Lien vers le concept")}><Icon name="link" size={12} /></span>{/if}
                </button>
                {#if m.excerpt}<p class="ex">{m.excerpt}</p>{/if}
              </li>
            {:else}
              <li class="faint">{t("Aucune note n'en parle encore.")}</li>
            {/each}
          </ol>
        </div>
        <div>
          <h3>{t("Concepts liés")}</h3>
          {#each detail.related as [p, title] (p)}
            <button class="chip" onclick={() => (sel = p)}>{title}</button>
          {:else}
            <p class="faint">{t("Aucun pour l'instant.")}</p>
          {/each}
          <h3>{t("Questions de recherche")}</h3>
          {#each detail.questions as [p, title] (p)}
            <button class="q" onclick={() => app.openNote(p, { newTab: true })}>{title}</button>
          {:else}
            <p class="faint">{t("Aucune question n'en parle.")}</p>
          {/each}
        </div>
      </div>
    {:else if rows && !rows.length}
      <p class="empty">{t("Les concepts sont des notes « type: concept ».")}</p>
    {/if}
  </section>
</div>

<style>
  .cv {
    height: 100%;
    display: grid;
    grid-template-columns: minmax(220px, 300px) minmax(0, 1fr);
    min-height: 0;
  }
  .list {
    border-right: 1px solid var(--line);
    overflow: auto;
    padding: 16px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .lh,
  .dh {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .dh {
    flex-wrap: wrap;
  }
  .grow {
    flex: 1;
  }
  h2 {
    margin: 0;
    font-size: 18px;
    flex: 1;
  }
  .dh h2 {
    flex: 0 1 auto;
    font-size: 20px;
  }
  h3 {
    margin: 16px 0 6px;
    font-size: 13.5px;
  }
  .row {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    border: 1px solid transparent;
    border-radius: var(--radius);
    background: none;
    padding: 7px 8px;
    text-align: left;
    color: var(--text);
    cursor: pointer;
    min-width: 0;
  }
  .row:hover {
    background: var(--bg-soft);
  }
  .row.on {
    background: var(--accent-soft);
    border-color: var(--accent);
  }
  .rt {
    font: 600 13px var(--font-ui);
  }
  .rd {
    font-size: 12px;
    color: var(--text-soft);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .rc {
    font-size: 11px;
    color: var(--text-faint);
  }
  .detail {
    overflow: auto;
    padding: 18px 24px 30px;
    min-width: 0;
  }
  .def {
    margin: 10px 0 0;
    padding: 10px 12px;
    border-left: 3px solid var(--accent);
    background: var(--bg-soft);
    border-radius: 0 var(--radius) var(--radius) 0;
    font: 15px/1.55 var(--font-editor);
  }
  .defs,
  .tl {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .defs li {
    display: grid;
    grid-template-columns: 88px 1fr;
    gap: 8px;
    padding: 4px 0;
    font-size: 13px;
    border-bottom: 1px solid var(--line);
  }
  .cols {
    display: grid;
    grid-template-columns: minmax(0, 2fr) minmax(200px, 1fr);
    gap: 24px;
  }
  .sub {
    margin: 0 0 8px;
    font-size: 12px;
    color: var(--text-soft);
  }
  .tl li {
    position: relative;
    padding: 0 0 12px 16px;
    border-left: 2px solid var(--line);
    margin-left: 4px;
  }
  .tl li::before {
    content: "";
    position: absolute;
    left: -6px;
    top: 3px;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--bg);
    border: 2px solid var(--accent);
  }
  .date {
    font: 11.5px var(--font-mono);
    color: var(--text-faint);
  }
  .tlb {
    display: flex;
    align-items: baseline;
    gap: 6px;
    width: 100%;
    border: 0;
    background: none;
    padding: 1px 0;
    text-align: left;
    color: var(--text);
    cursor: pointer;
    min-width: 0;
  }
  .tlb:hover .tt {
    color: var(--accent-text);
  }
  .tt {
    font: 600 13px var(--font-ui);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .k {
    font-size: 10.5px;
    padding: 0 6px;
    border-radius: 999px;
    background: var(--bg-sunk);
    color: var(--text-soft);
    flex-shrink: 0;
  }
  .lk {
    color: var(--accent-text);
    display: inline-flex;
  }
  .ex {
    margin: 2px 0 0;
    font-size: 12.5px;
    color: var(--text-soft);
  }
  .chip {
    display: inline-block;
    margin: 0 4px 4px 0;
    padding: 1px 10px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: none;
    font: 12.5px var(--font-ui);
    color: var(--text);
    cursor: pointer;
  }
  .chip:hover,
  .q:hover {
    color: var(--accent-text);
  }
  .q {
    display: block;
    border: 0;
    background: none;
    padding: 2px 0;
    font: 13px var(--font-ui);
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .faint {
    color: var(--text-faint);
    font-size: 12px;
  }
  .empty {
    color: var(--text-soft);
    font-size: 12.5px;
  }
  .error {
    color: var(--danger);
  }
  @media (max-width: 900px) {
    .cols {
      grid-template-columns: 1fr;
    }
  }
</style>
