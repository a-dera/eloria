<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { locale, t, tn, translateError } from "../lib/i18n.svelte";
  import type { ResearchStats } from "../lib/types";

  // Research statistics: what you cite (documents, authors, journals, years), what
  // you work on (tags per month) and what each chapter covers (cited topics).
  let { visible = true }: { visible?: boolean } = $props();
  let s = $state<ResearchStats | null>(null);
  let error = $state("");
  $effect(() => {
    void app.revision;
    if (!visible) return;
    const now = new Date();
    api.researchStats(now.getFullYear(), now.getMonth() + 1).then((x) => (s = x)).catch((e) => (error = translateError(e)));
  });

  const n = (x: number) => x.toLocaleString(locale());
  const maxOf = (rows: [unknown, number][] | [string, string, number][]) => Math.max(1, ...rows.map((r) => Number(r[r.length - 1])));
  const heatMax = $derived(Math.max(1, ...(s?.heat.flat() ?? [0])));
  const coverMax = $derived(Math.max(1, ...(s?.cover.flat() ?? [0])));
  const yearMax = $derived(Math.max(1, ...(s?.years.map((y) => y[1]) ?? [0])));
  /** Single sequential hue: from the surface to the accent. */
  const shade = (v: number, max: number) => (v ? `color-mix(in srgb, var(--accent) ${Math.round(18 + (72 * v) / max)}%, var(--bg))` : "var(--bg-sunk)");
  const monthLabel = (m: string) => new Date(`${m}-15T12:00:00`).toLocaleDateString(locale(), { month: "short" });
  function openDoc(ck: string) {
    const p = app.fiches[ck.toLowerCase()];
    if (p) app.openNote(p, { newTab: true });
  }
  function searchTag(tag: string) {
    app.searchQuery = `#${tag}`;
    app.leftPanel = "search";
    app.leftOpen = true;
  }
</script>

<div class="stats">
  <h2>{t("Statistiques de recherche")}</h2>
  {#if error}<p class="error">{error}</p>{/if}
  {#if !s && !error}<p class="loading" role="status">{t("Analyse de tes notes…")}</p>{/if}
  {#if s}
    <div class="kpis">
      <div class="kpi"><span class="v">{n(s.documents_cited)}</span><span class="l">{t("documents cités sur {n}", { n: n(s.documents_total) })}</span></div>
      <div class="kpi"><span class="v">{n(s.citations_total)}</span><span class="l">{t("citations dans tes notes de travail")}</span></div>
      <div class="kpi"><span class="v">{s.top_authors[0]?.[0] ?? "—"}</span><span class="l">{t("auteur le plus cité")}</span></div>
      <div class="kpi"><span class="v small">{s.top_venues[0]?.[0] ?? "—"}</span><span class="l">{t("revue la plus citée")}</span></div>
    </div>

    <div class="grid">
      <section class="card">
        <h3>{t("Documents les plus cités")}</h3>
        <p class="sub">{t("Nombre de notes de travail qui les citent (hors sources et fiches).")}</p>
        {#each s.top_documents as [ck, title, c] (ck)}
          <button class="hrow" title={tn(c, "{title} : cité dans {n} note", "{title} : cité dans {n} notes", { title: title || ck })} onclick={() => openDoc(ck)}>
            <span class="lab"><span class="ck">@{ck}</span> {title}</span>
            <span class="track"><span class="fill" style:width="{(c / maxOf(s.top_documents)) * 100}%"></span></span>
            <span class="val">{c}</span>
          </button>
        {:else}
          <p class="empty">{t("Aucune citation dans tes notes de travail pour l'instant.")}</p>
        {/each}
      </section>

      <section class="card">
        <h3>{t("Auteurs les plus cités")}</h3>
        {#each s.top_authors as [name, c] (name)}
          <div class="hrow" title={tn(c, "{name} : {n} citation", "{name} : {n} citations", { name })}>
            <span class="lab">{name}</span>
            <span class="track"><span class="fill" style:width="{(c / maxOf(s.top_authors)) * 100}%"></span></span>
            <span class="val">{c}</span>
          </div>
        {:else}
          <p class="empty">—</p>
        {/each}
        <h3 class="mt">{t("Revues les plus citées")}</h3>
        {#each s.top_venues as [name, c] (name)}
          <div class="hrow" title={tn(c, "{name} : {n} citation", "{name} : {n} citations", { name })}>
            <span class="lab">{name}</span>
            <span class="track"><span class="fill" style:width="{(c / maxOf(s.top_venues)) * 100}%"></span></span>
            <span class="val">{c}</span>
          </div>
        {:else}
          <p class="empty">—</p>
        {/each}
      </section>

      <section class="card">
        <h3>{t("Années des documents cités")}</h3>
        {#if s.years.length}
          <div class="cols" role="img" aria-label={t("Années des documents cités")}>
            {#each s.years as [y, c] (y)}
              <div class="col" title={tn(c, "{y} : {n} document", "{y} : {n} documents", { y })}>
                <span class="cval">{c}</span>
                <span class="bar" style:height="{(c / yearMax) * 100}%"></span>
                <span class="clab">{y}</span>
              </div>
            {/each}
          </div>
        {:else}
          <p class="empty">—</p>
        {/if}
      </section>
    </div>

    <section class="card">
      <h3>{t("Carte de chaleur des thématiques")}</h3>
      <p class="sub">{t("Notes modifiées par mois, pour tes tags les plus employés. Clic sur un tag : ses notes.")}</p>
      {#if s.heat_tags.length}
        <div class="heat" style:grid-template-columns="minmax(90px, 160px) repeat({s.heat_months.length}, minmax(22px, 1fr))">
          <span></span>
          {#each s.heat_months as m (m)}<span class="mh">{monthLabel(m)}</span>{/each}
          {#each s.heat_tags as tag, i (tag)}
            <button class="rh" onclick={() => searchTag(tag)}>#{tag}</button>
            {#each s.heat[i] as v, j (j)}
              <span class="cell" style:background={shade(v, heatMax)} title={tn(v, "#{tag} · {m} : {n} note", "#{tag} · {m} : {n} notes", { tag, m: s.heat_months[j] })}></span>
            {/each}
          {/each}
        </div>
        <div class="legend"><span>{t("moins")}</span><span class="ramp"></span><span>{t("plus")}</span></div>
      {:else}
        <p class="empty">{t("Pas encore de tags.")}</p>
      {/if}
    </section>

    <section class="card">
      <h3>{t("Couverture de la recherche")}</h3>
      <p class="sub">{t("Documents cités par chaque chapitre (et ses notes rattachées), par thème de source.")}</p>
      {#if s.cover_chapters.length && s.cover_themes.length}
        <div class="heat" style:grid-template-columns="minmax(90px, 160px) repeat({s.cover_chapters.length}, minmax(44px, 90px))">
          <span></span>
          {#each s.cover_chapters as c (c)}<span class="mh">{c.toUpperCase()}</span>{/each}
          {#each s.cover_themes as th, i (th)}
            <button class="rh" onclick={() => searchTag(th)}>#{th}</button>
            {#each s.cover[i] as v, j (j)}
              <span class="cell big" style:background={shade(v, coverMax)} title={tn(v, "#{th} dans {c} : {n} document", "#{th} dans {c} : {n} documents", { th, c: s.cover_chapters[j].toUpperCase() })}>{v || ""}</span>
            {/each}
          {/each}
        </div>
      {:else}
        <p class="empty">{t("Il faut des chapitres (type: chapitre) et des sources avec des tags.")}</p>
      {/if}
      {#if s.gaps.length}
        <p class="gaps">
          {t("Thèmes de ta bibliothèque qu'aucun chapitre ne cite encore :")}
          {#each s.gaps as [g, c] (g)}<button class="tag" onclick={() => searchTag(g)}>#{g} <span>{tn(c, "{n} source", "{n} sources")}</span></button>{/each}
        </p>
      {/if}
    </section>
  {/if}
</div>

<style>
  .stats {
    height: 100%;
    overflow: auto;
    padding: 20px 26px 30px;
  }
  h2 {
    margin: 0 0 14px;
    font-size: 20px;
  }
  h3 {
    margin: 0 0 4px;
    font-size: 13.5px;
  }
  h3.mt {
    margin-top: 14px;
  }
  .sub {
    margin: 0 0 10px;
    font-size: 12px;
    color: var(--text-soft);
  }
  .kpis {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 10px;
    margin-bottom: 12px;
  }
  .kpi {
    display: flex;
    flex-direction: column;
    padding: 12px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--bg-soft);
    min-width: 0;
  }
  .v {
    font-size: 22px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .v.small {
    font-size: 16px;
    line-height: 27px;
  }
  .l {
    font-size: 12px;
    color: var(--text-soft);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 12px;
    margin-bottom: 12px;
  }
  .card {
    padding: 12px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--bg);
    min-width: 0;
    margin-bottom: 12px;
  }
  .grid .card {
    margin: 0;
  }
  .hrow {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 30% 26px;
    align-items: center;
    gap: 8px;
    width: 100%;
    border: 0;
    background: none;
    padding: 3px 0;
    font: 12.5px var(--font-ui);
    color: var(--text);
    text-align: left;
  }
  button.hrow {
    cursor: pointer;
  }
  button.hrow:hover .lab {
    color: var(--accent-text);
  }
  .lab {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ck {
    font: 11.5px var(--font-mono);
    color: var(--text-soft);
  }
  .track {
    height: 8px;
  }
  .fill {
    display: block;
    height: 100%;
    min-width: 2px;
    background: var(--accent);
    border-radius: 0 4px 4px 0;
  }
  .val {
    text-align: right;
    font-variant-numeric: tabular-nums;
    color: var(--text-soft);
  }
  .cols {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    height: 140px;
    padding-top: 4px;
  }
  .col {
    flex: 1;
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    align-items: center;
    gap: 2px;
    min-width: 0;
  }
  .col .bar {
    width: 70%;
    max-width: 28px;
    min-height: 2px;
    background: var(--accent);
    border-radius: 4px 4px 0 0;
  }
  .cval,
  .clab {
    font-size: 10.5px;
    color: var(--text-soft);
    font-variant-numeric: tabular-nums;
  }
  .heat {
    display: grid;
    gap: 2px;
    align-items: center;
    overflow-x: auto;
  }
  .mh {
    font-size: 10.5px;
    color: var(--text-faint);
    text-align: center;
    white-space: nowrap;
  }
  .rh {
    border: 0;
    background: none;
    padding: 0 6px 0 0;
    font: 12px var(--font-ui);
    color: var(--text-soft);
    text-align: left;
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rh:hover {
    color: var(--accent-text);
  }
  .cell {
    height: 20px;
    border-radius: 3px;
  }
  .cell.big {
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11.5px;
    color: var(--text);
    font-variant-numeric: tabular-nums;
  }
  .legend {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
    font-size: 11px;
    color: var(--text-faint);
  }
  .ramp {
    width: 120px;
    height: 8px;
    border-radius: 4px;
    background: linear-gradient(to right, color-mix(in srgb, var(--accent) 18%, var(--bg)), var(--accent));
  }
  .gaps {
    margin: 10px 0 0;
    font-size: 12.5px;
    color: var(--text-soft);
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .tag {
    border: 1px solid var(--line);
    border-radius: 999px;
    background: none;
    padding: 1px 9px;
    font: 12px var(--font-ui);
    color: var(--text);
    cursor: pointer;
  }
  .tag span {
    color: var(--text-faint);
  }
  .empty {
    color: var(--text-faint);
    font-size: 12.5px;
  }
  .error {
    color: var(--danger);
  }
</style>
