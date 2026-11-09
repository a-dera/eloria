<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { locale, t, tn, translateError } from "../lib/i18n.svelte";
  import type { ChapterInfo, ThesisOverview } from "../lib/types";
  import Icon from "./Icon.svelte";

  // Thesis workspace: writing order (dependencies between chapters), progress and
  // objectives of each chapter, research questions and what is attached to them.
  let { visible = true }: { visible?: boolean } = $props();
  let o = $state<ThesisOverview | null>(null);
  let error = $state("");
  $effect(() => {
    void app.revision;
    if (!visible) return;
    api.thesisOverview().then((x) => (o = x)).catch((e) => (error = translateError(e)));
  });

  const n = (x: number) => x.toLocaleString(locale());
  const byId = $derived(new Map((o?.chapters ?? []).map((c) => [c.id, c])));
  const qByPath = $derived(new Map((o?.questions ?? []).map((q) => [q.path, q])));
  const totalWords = $derived(o?.chapters.reduce((s, c) => s + c.words, 0) ?? 0);
  const totalTarget = $derived(o?.chapters.reduce((s, c) => s + c.target, 0) ?? 0);
  const objDone = $derived(o?.chapters.reduce((s, c) => s + c.objectives.filter((x) => x.done).length, 0) ?? 0);
  const objAll = $derived(o?.chapters.reduce((s, c) => s + c.objectives.filter((x) => x.done !== null).length, 0) ?? 0);
  const openQ = $derived(o?.questions.filter((q) => !/répondu|answered|close/i.test(q.status)).length ?? 0);

  // Dependency graph: columns = levels (a chapter comes after those it depends on).
  const BOX_W = 168;
  const BOX_H = 52;
  const GAP_X = 56;
  const GAP_Y = 16;
  const pos = $derived.by(() => {
    const m = new Map<string, { x: number; y: number }>();
    (o?.layers ?? []).forEach((layer, i) => layer.forEach((id, j) => m.set(id, { x: 8 + i * (BOX_W + GAP_X), y: 8 + j * (BOX_H + GAP_Y) })));
    return m;
  });
  const gw = $derived(Math.max(1, o?.layers.length ?? 1) * (BOX_W + GAP_X) - GAP_X + 16);
  const gh = $derived(Math.max(1, ...(o?.layers ?? [[]]).map((l) => l.length)) * (BOX_H + GAP_Y) - GAP_Y + 16);
  const edges = $derived(
    (o?.chapters ?? []).flatMap((c) =>
      c.depends_on.filter((d) => pos.has(d) && pos.has(c.id)).map((d) => ({ from: pos.get(d)!, to: pos.get(c.id)!, key: `${d}-${c.id}` })),
    ),
  );
  const pct = (c: ChapterInfo) => (c.target ? Math.min(100, Math.round((c.words / c.target) * 100)) : 0);
  const short = (s: string, k = 22) => (s.length > k ? `${s.slice(0, k - 1)}…` : s);
</script>

<div class="these">
  <div class="top">
    <h2>{t("Espace thèse")}</h2>
    <span class="grow"></span>
    <button class="btn small" onclick={() => app.newQuestion()}><Icon name="plus" size={13} /> {t("Question de recherche")}</button>
  </div>
  {#if error}<p class="error">{error}</p>{/if}
  {#if !o && !error}<p class="loading" role="status">{t("Analyse de tes chapitres…")}</p>{/if}
  {#if o}
    <div class="kpis">
      <div class="kpi"><span class="v">{n(totalWords)}</span><span class="l">{totalTarget ? t("mots sur {n}", { n: n(totalTarget) }) : t("mots rédigés")}</span></div>
      <div class="kpi"><span class="v">{o.chapters.length}</span><span class="l">{tn(o.chapters.length, "chapitre", "chapitres")}</span></div>
      <div class="kpi"><span class="v">{objDone}/{objAll}</span><span class="l">{t("objectifs atteints")}</span></div>
      <div class="kpi"><span class="v">{openQ}/{o.questions.length}</span><span class="l">{t("questions ouvertes")}</span></div>
    </div>

    {#if !o.chapters.length}
      <p class="empty">{t("Aucun chapitre. Crée-en un depuis le panneau Thèse (bouton +) : propriétés type: chapitre et chapitre: ch1.")}</p>
    {:else}
      <section class="card">
        <h3>{t("Ordre de rédaction")}</h3>
        <p class="hint">{t("Un chapitre vient après ceux dont il dépend (propriété depend_de: [ch1]). Clic : ouvrir le chapitre.")}</p>
        <div class="gwrap">
          <svg width={gw} height={gh} role="img" aria-label={t("Dépendances entre chapitres")}>
            <defs>
              <marker id="arr" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
                <path d="M0,0 L10,5 L0,10 z" class="ah" />
              </marker>
            </defs>
            {#each edges as e (e.key)}
              <path class="edge" marker-end="url(#arr)" d="M{e.from.x + BOX_W},{e.from.y + BOX_H / 2} C{e.from.x + BOX_W + GAP_X / 2},{e.from.y + BOX_H / 2} {e.to.x - GAP_X / 2},{e.to.y + BOX_H / 2} {e.to.x},{e.to.y + BOX_H / 2}" />
            {/each}
            {#each [...pos.entries()] as [id, p] (id)}
              {@const c = byId.get(id)!}
              <g class="node" class:cyc={o.cycle.includes(id)} role="button" tabindex="0" transform="translate({p.x},{p.y})" onclick={() => app.openNote(c.path, { newTab: true })} onkeydown={(ev) => ev.key === "Enter" && app.openNote(c.path, { newTab: true })}>
                <title>{c.title}</title>
                <rect width={BOX_W} height={BOX_H} rx="8" />
                <text x="10" y="20" class="nt">{id.toUpperCase()} · {short(c.title.replace(/^chapitre\s*\d+\s*[·:.-]\s*/i, ""))}</text>
                <text x="10" y="38" class="ns">{c.status || "—"}{c.target ? ` · ${pct(c)} %` : ""}</text>
                {#if c.target}<rect x="0" y={BOX_H - 4} width={(BOX_W * pct(c)) / 100} height="4" class="bar" />{/if}
              </g>
            {/each}
          </svg>
        </div>
        {#if o.missing.length}
          <p class="warn">{t("Dépendances inconnues : {list}", { list: o.missing.map(([a, b]) => `${a} → ${b}`).join(", ") })}</p>
        {/if}
        {#if o.cycle.length}
          <p class="warn">{t("Dépendances circulaires : {list}", { list: o.cycle.join(", ") })}</p>
        {/if}
      </section>

      <div class="grid">
        {#each o.chapters as c (c.id)}
          <section class="card chap">
            <div class="ch-head">
              <button class="link" onclick={() => app.openNote(c.path, { newTab: true })}>{c.title}</button>
              {#if c.status}<span class="pill">{c.status}</span>{/if}
            </div>
            {#if c.target}
              <div class="progress"><span style:width="{pct(c)}%"></span></div>
              <div class="faint">{t("{a} / {b} mots", { a: n(c.words), b: n(c.target) })} · {tn(c.fiches, "{n} fiche", "{n} fiches")}</div>
            {/if}
            {#if c.depends_on.length}<div class="faint">{t("Dépend de : {list}", { list: c.depends_on.map((d) => d.toUpperCase()).join(", ") })}</div>{/if}
            <h4>{t("Objectifs")}</h4>
            {#if c.objectives.length}
              <ul class="obj">
                {#each c.objectives as ob, i (i)}<li class:done={ob.done === true}><span class="box">{ob.done === true ? "✓" : ob.done === false ? "○" : "•"}</span>{ob.text}</li>{/each}
              </ul>
            {:else}
              <p class="faint">{t("Ajoute une section « ## Objectifs » (liste à cocher) dans le chapitre.")}</p>
            {/if}
            <h4>{t("Questions de recherche")}</h4>
            {#each c.questions as qp (qp)}
              {@const q = qByPath.get(qp)}
              {#if q}<button class="q" onclick={() => app.openNote(q.path, { newTab: true })}><span class="qs">{q.status}</span> {q.title}</button>{/if}
            {:else}
              <p class="faint">{t("Aucune question rattachée.")}</p>
            {/each}
            <div class="acts">
              <button class="btn small ghost" onclick={() => app.newQuestion(c.id)}>{t("+ Question pour ce chapitre")}</button>
              <button class="btn small ghost" onclick={() => (app.outlineFor = { scope: "chapter", value: c.id })}>{t("Proposer un plan")}</button>
            </div>
          </section>
        {/each}
      </div>
    {/if}

    <section class="card">
      <h3>{t("Questions de recherche")} <span class="faint">{o.questions.length}</span></h3>
      {#if o.questions.length}
        <table>
          <thead><tr><th>{t("Question")}</th><th>{t("Statut")}</th><th>{t("Chapitres")}</th><th>{t("Sources")}</th><th>{t("Notes liées")}</th></tr></thead>
          <tbody>
            {#each o.questions as q (q.path)}
              <tr>
                <td><button class="link" onclick={() => app.openNote(q.path, { newTab: true })}>{q.title}</button></td>
                <td><span class="pill">{q.status}</span></td>
                <td>{q.chapters.map((x) => x.toUpperCase()).join(", ") || "—"}</td>
                <td><div class="srcs">{#each q.sources as s (s)}<button class="ck" onclick={() => { const p = app.fiches[s.toLowerCase()]; if (p) app.openNote(p, { newTab: true }); }}>@{s}</button>{:else}—{/each}</div></td>
                <td>{q.notes.length}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {:else}
        <p class="faint">{t("Une question de recherche est une note (type: question) rattachée à des chapitres ; cite ou relie les sources qui y répondent.")}</p>
      {/if}
    </section>
  {/if}
</div>

<style>
  .these {
    height: 100%;
    overflow: auto;
    padding: 20px 26px 30px;
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
    margin: 0 0 6px;
    font-size: 13.5px;
  }
  h4 {
    margin: 10px 0 4px;
    font-size: 12px;
    color: var(--text-soft);
  }
  .kpis {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
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
  }
  .v {
    font-size: 22px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
  .l,
  .hint {
    font-size: 12px;
    color: var(--text-soft);
  }
  .hint {
    margin: 0 0 8px;
  }
  .card {
    padding: 12px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--bg);
    min-width: 0;
    margin-bottom: 12px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 12px;
    margin-bottom: 12px;
  }
  .grid .card {
    margin: 0;
  }
  .gwrap {
    overflow-x: auto;
  }
  svg {
    display: block;
  }
  .edge {
    fill: none;
    stroke: var(--line-strong);
    stroke-width: 1.5;
  }
  .ah {
    fill: var(--line-strong);
  }
  .node {
    cursor: pointer;
  }
  .node rect:first-of-type {
    fill: var(--bg-soft);
    stroke: var(--line-strong);
  }
  .node:hover rect:first-of-type,
  .node:focus-visible rect:first-of-type {
    stroke: var(--accent);
  }
  .node.cyc rect:first-of-type {
    stroke: var(--danger);
  }
  .node .bar {
    fill: var(--accent);
    opacity: 0.8;
  }
  .nt {
    font: 600 12px var(--font-ui);
    fill: var(--text);
  }
  .ns {
    font: 11px var(--font-ui);
    fill: var(--text-soft);
  }
  .warn {
    margin: 8px 0 0;
    font-size: 12.5px;
    color: var(--warning);
  }
  .ch-head {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin-bottom: 6px;
  }
  .link {
    border: 0;
    background: none;
    padding: 0;
    font: 600 13.5px var(--font-ui);
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .link:hover {
    color: var(--accent-text);
  }
  .pill {
    padding: 0 8px;
    border-radius: 999px;
    background: var(--bg-sunk);
    font-size: 11.5px;
    color: var(--text-soft);
    white-space: nowrap;
  }
  .progress {
    height: 6px;
    border-radius: 3px;
    background: var(--bg-sunk);
    overflow: hidden;
    margin: 4px 0;
  }
  .progress span {
    display: block;
    height: 100%;
    background: var(--accent);
  }
  .faint {
    color: var(--text-faint);
    font-size: 12px;
    margin: 2px 0;
  }
  .obj {
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 13px;
  }
  .obj li {
    display: flex;
    gap: 6px;
    padding: 1px 0;
  }
  .obj li.done {
    color: var(--text-soft);
  }
  .box {
    width: 14px;
    flex-shrink: 0;
    color: var(--text-faint);
  }
  .obj li.done .box {
    color: var(--success);
  }
  .q {
    display: block;
    width: 100%;
    border: 0;
    background: none;
    padding: 2px 0;
    text-align: left;
    font: 13px var(--font-ui);
    color: var(--text);
    cursor: pointer;
  }
  .q:hover {
    color: var(--accent-text);
  }
  .qs {
    font-size: 11px;
    color: var(--text-faint);
    margin-right: 4px;
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 6px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }
  th {
    text-align: left;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-faint);
    padding: 4px 8px 4px 0;
    border-bottom: 1px solid var(--line);
  }
  td {
    padding: 6px 8px 6px 0;
    border-bottom: 1px solid var(--line);
    vertical-align: top;
  }
  .srcs {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .ck {
    border: 0;
    background: none;
    padding: 0;
    font: 12px var(--font-mono);
    color: var(--accent-text);
    cursor: pointer;
  }
  .empty,
  .error {
    color: var(--text-soft);
  }
  .error {
    color: var(--danger);
  }
</style>
