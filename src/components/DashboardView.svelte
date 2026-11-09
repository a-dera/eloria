<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { locale, t, tn, translateError } from "../lib/i18n.svelte";
  import type { Dashboard } from "../lib/types";

  // Thesis dashboard: readings, writing, chapters, reading queue.
  let { visible = true }: { visible?: boolean } = $props();
  let d = $state<Dashboard | null>(null);
  let error = $state("");
  $effect(() => {
    void app.revision;
    if (!visible) return;
    api.dashboard().then((x) => (d = x)).catch((e) => (error = translateError(e)));
  });

  const max = $derived(d ? Math.max(1, ...d.written.map((w) => w[1])) : 1);
  const week = $derived(d ? d.written.slice(-7).reduce((n, w) => n + w[1], 0) : 0);
  const totalFiches = $derived(d ? Object.values(d.fiches).reduce((a, b) => a + b, 0) : 0);
  const STATUS: [string, string][] = [
    ["à lire", t("À lire")],
    ["en cours", t("En cours")],
    ["lue", t("Lues")],
    ["exploitée", t("Exploitées")],
  ];
  const n = (x: number) => x.toLocaleString(locale());
  const day = (s: string) => new Date(`${s}T12:00:00`).toLocaleDateString(locale(), { weekday: "short", day: "numeric" });
</script>

<div class="dash">
  <button class="btn small ghost more" onclick={() => app.openView("statistiques")}>{t("Statistiques de recherche")}</button>
  <h2>{t("Tableau de bord")}</h2>
  {#if error}<p class="error">{error}</p>{/if}
  {#if !d && !error}<p class="loading" role="status">{t("Chargement du tableau de bord…")}</p>{/if}
  {#if d}
    <div class="kpis">
      <div class="kpi"><span class="v">{n(d.written_today)}</span><span class="l">{t("mots écrits aujourd'hui")}</span></div>
      <div class="kpi"><span class="v">{n(week)}</span><span class="l">{t("mots sur 7 jours")}</span></div>
      <div class="kpi"><span class="v">{n(d.words)}</span><span class="l">{t("mots dans le coffre")}</span></div>
      <div class="kpi"><span class="v">{n(d.sources)}</span><span class="l">{t("sources")}</span></div>
      <div class="kpi"><span class="v">{n(totalFiches)}</span><span class="l">{t("fiches")}</span></div>
    </div>

    <div class="grid">
      <section class="card">
        <h3>{t("Rédaction, 14 derniers jours")}</h3>
        <div class="bars" role="img" aria-label={t("Mots écrits par jour")}>
          {#each d.written as [date, w] (date)}
            <div class="bar" title="{day(date)} : {n(w)}">
              <span class="fill" style:height="{(w / max) * 100}%"></span>
              <span class="dl">{day(date).split(" ")[1] ?? ""}</span>
            </div>
          {/each}
        </div>
      </section>

      <section class="card">
        <h3>{t("Objectifs")}</h3>
        {#each [
          { label: t("Mots aujourd'hui"), v: d.written_today, goal: app.settings?.goal_words_day ?? 0 },
          { label: t("Fiches lues cette semaine"), v: d.fiches_read_week, goal: app.settings?.goal_fiches_week ?? 0 },
        ] as g (g.label)}
          <div class="goal">
            <span class="rt">{g.label}</span>
            <span class="prog"><span style:width="{g.goal ? Math.min(100, (g.v / g.goal) * 100) : 0}%"></span></span>
            <span class="faint">{n(g.v)}{g.goal ? ` / ${n(g.goal)}` : ""}</span>
          </div>
        {/each}
        <p class="faint small">{t("Concentration : {a} min aujourd'hui, {b} min sur 7 jours", { a: d.focus_today, b: d.focus_week })}</p>
        <div class="acts">
          <button class="btn small" onclick={() => (app.focus ? app.stopFocus() : app.startFocus())}>{app.focus ? t("Arrêter la session") : t("Démarrer une session")}</button>
          <button class="btn small" onclick={() => app.weeklyReview()}>{t("Revue de la semaine")}</button>
        </div>
      </section>

      {#if d.reviews_due.length}
        <section class="card">
          <h3>{t("À revoir aujourd'hui")}</h3>
          {#each d.reviews_due as [path, title] (path)}
            <div class="row">
              <button class="rt linkb" onclick={() => app.openNote(path, { newTab: true })}>{title}</button>
              <button class="btn small ghost" title={t("Revue faite : prochaine dans une semaine")} onclick={() => app.scheduleReview(path, 7)}>{t("Fait, +7 j")}</button>
            </div>
          {/each}
        </section>
      {/if}

      <section class="card">
        <h3>{t("Lectures")}</h3>
        <div class="stack" role="img" aria-label={t("Fiches par statut")}>
          {#each STATUS as [k, label] (k)}
            {#if d.fiches[k]}<span class="seg s{STATUS.findIndex((x) => x[0] === k)}" style:flex={d.fiches[k]} title="{label} : {d.fiches[k]}"></span>{/if}
          {/each}
        </div>
        <ul class="legend">
          {#each STATUS as [k, label], i (k)}<li><span class="dot s{i}"></span>{label} <b>{d.fiches[k] ?? 0}</b></li>{/each}
        </ul>
      </section>

      <section class="card">
        <h3>{t("Chapitres")}</h3>
        {#each d.chapters as [path, title, words, target] (path)}
          <button class="row" onclick={() => app.openNote(path, { newTab: true })}>
            <span class="rt">{title}</span>
            <span class="prog"><span style:width="{target ? Math.min(100, (words / target) * 100) : 0}%"></span></span>
            <span class="faint">{n(words)}{target ? ` / ${n(target)}` : ""}</span>
          </button>
        {:else}
          <p class="faint">{t("Aucun chapitre : crée-en un depuis le panneau Thèse.")}</p>
        {/each}
      </section>

      <section class="card">
        <h3>{t("À lire ensuite")}</h3>
        {#each d.to_read as [path, title] (path)}
          <button class="row" onclick={() => app.openNote(path, { newTab: true })}><span class="rt">{title}</span></button>
        {:else}
          <p class="faint">{t("Rien en attente.")}</p>
        {/each}
      </section>

      <section class="card">
        <h3>{t("Modifiées récemment")}</h3>
        {#each d.recent as [path, title, mtime] (path)}
          <button class="row" onclick={() => app.openNote(path, { newTab: true })}>
            <span class="rt">{title}</span><span class="faint">{new Date(mtime).toLocaleDateString(locale(), { day: "numeric", month: "short" })}</span>
          </button>
        {/each}
      </section>

      <section class="card">
        <h3>{t("Contrôles")}</h3>
        <button class="row" onclick={() => (app.integrityOpen = true)}><span class="rt">{t("Vérifier le coffre")}</span></button>
        <button class="row" onclick={() => (app.checkFor = { kind: "bib" })}><span class="rt">{t("Audit de la bibliographie")}</span></button>
        <button class="row" onclick={() => (app.checkFor = { kind: "notes" })}><span class="rt">{t("Notes orphelines et doublons")}</span></button>
        <p class="faint small">{t("IA ce mois-ci : {i} jetons lus, {o} écrits", { i: n(d.ai_tokens[0]), o: n(d.ai_tokens[1]) })}</p>
      </section>
    </div>
  {/if}
</div>

<style>
  .dash {
    height: 100%;
    overflow: auto;
    padding: 20px 26px 30px;
    position: relative;
  }
  .more {
    position: absolute;
    top: 18px;
    right: 26px;
  }
  h2 {
    margin: 0 0 14px;
    font-size: 20px;
  }
  .kpis {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    gap: 10px;
    margin-bottom: 14px;
  }
  .kpi {
    display: flex;
    flex-direction: column;
    padding: 12px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--bg-soft);
  }
  .v {
    font-size: 22px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
  .l {
    font-size: 12px;
    color: var(--text-soft);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 12px;
  }
  .card {
    padding: 12px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--bg);
    min-width: 0;
  }
  h3 {
    margin: 0 0 10px;
    font-size: 13.5px;
  }
  .bars {
    display: flex;
    align-items: flex-end;
    gap: 4px;
    height: 120px;
  }
  .bar {
    flex: 1;
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    align-items: center;
    gap: 3px;
  }
  .fill {
    width: 100%;
    min-height: 2px;
    border-radius: 3px 3px 0 0;
    background: var(--accent);
    opacity: 0.85;
  }
  .dl {
    font-size: 10px;
    color: var(--text-faint);
  }
  .stack {
    display: flex;
    height: 14px;
    border-radius: 7px;
    overflow: hidden;
    background: var(--bg-sunk);
    gap: 2px;
  }
  .seg,
  .dot {
    background: var(--text-faint);
  }
  .s1 {
    background: color-mix(in srgb, var(--accent) 45%, var(--bg));
  }
  .s2 {
    background: var(--accent);
  }
  .s3 {
    background: color-mix(in srgb, var(--accent) 70%, #000);
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 14px;
    margin: 10px 0 0;
    padding: 0;
    list-style: none;
    font-size: 12.5px;
    color: var(--text-soft);
  }
  .legend li {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 5px 0;
    border: 0;
    border-bottom: 1px solid var(--line);
    background: none;
    color: var(--text);
    font: 13px var(--font-ui);
    text-align: left;
    cursor: pointer;
  }
  .row:hover .rt {
    color: var(--accent-text);
  }
  .rt {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .prog {
    width: 90px;
    height: 6px;
    border-radius: 3px;
    background: var(--bg-sunk);
    overflow: hidden;
    flex-shrink: 0;
  }
  .prog span {
    display: block;
    height: 100%;
    background: var(--accent);
  }
  .faint {
    color: var(--text-faint);
    font-size: 12px;
    flex-shrink: 0;
  }
  .small {
    margin: 8px 0 0;
  }
  .goal {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 0;
    font-size: 13px;
  }
  .acts {
    display: flex;
    gap: 6px;
    margin-top: 8px;
  }
  .linkb {
    border: 0;
    background: none;
    padding: 0;
    text-align: left;
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  .linkb:hover {
    color: var(--accent-text);
  }
  .error {
    color: var(--danger);
  }
</style>
