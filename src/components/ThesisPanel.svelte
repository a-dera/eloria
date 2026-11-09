<script lang="ts">
  import { locale, t, tn } from "../lib/i18n.svelte";
  import { app } from "../lib/state.svelte";
  import type { NoteMeta } from "../lib/types";
  import { STATUS_ORDER } from "../lib/util";
  import Icon from "./Icon.svelte";

  interface Chapter {
    id: string;
    note: NoteMeta | null;
    target: number;
    linked: NoteMeta[];
  }

  let open = $state<Record<string, boolean>>({});

  const asList = (v: unknown): string[] =>
    Array.isArray(v) ? v.map((x) => String(x).toLowerCase()) : typeof v === "string" && v ? v.split(",").map((x) => x.trim().toLowerCase()) : [];

  const chapters = $derived.by(() => {
    const map = new Map<string, Chapter>();
    for (const n of app.notes) {
      if (n.frontmatter.type === "chapitre" && n.frontmatter.chapitre) {
        const id = String(n.frontmatter.chapitre).toLowerCase();
        map.set(id, { id, note: n, target: Number(n.frontmatter.objectif) || 0, linked: [] });
      }
    }
    for (const n of app.notes) {
      for (const id of asList(n.frontmatter.chapitres)) {
        if (!map.has(id)) map.set(id, { id, note: null, target: 0, linked: [] });
        map.get(id)!.linked.push(n);
      }
    }
    return [...map.values()].sort((a, b) => a.id.localeCompare(b.id, "fr", { numeric: true }));
  });

  const fiches = $derived(app.notes.filter((n) => n.frontmatter.type === "fiche"));
  const byStatus = $derived.by(() => {
    const m = new Map<string, number>();
    for (const f of fiches) {
      const s = String(f.frontmatter.statut ?? t("sans statut"));
      m.set(s, (m.get(s) ?? 0) + 1);
    }
    return [...m.entries()].sort((a, b) => {
      const ia = STATUS_ORDER.indexOf(a[0]);
      const ib = STATUS_ORDER.indexOf(b[0]);
      return (ia < 0 ? 99 : ia) - (ib < 0 ? 99 : ib);
    });
  });
  const totalWords = $derived(chapters.reduce((s, c) => s + (c.note?.words ?? 0), 0));
  const totalTarget = $derived(chapters.reduce((s, c) => s + c.target, 0));

  async function newChapter() {
    const id = await app.ask(t("Identifiant du chapitre (ex. ch3)"), `ch${chapters.length + 1}`);
    if (!id) return;
    const title = await app.ask(t("Titre du chapitre"), "");
    if (title === null) return;
    const content = `---\ntype: chapitre\nchapitre: ${id}\nobjectif: 10000\nstatut: plan\ndepend_de: []\ntags: [thèse/${id}]\n---\n# ${title || id}\n\n## ${t("Objectifs")}\n\n- [ ] \n\n`;
    app.newNote("Thèse", `${id.toUpperCase()} - ${title || t("Sans titre")}`, content);
  }

  const kind = (n: NoteMeta) => String(n.frontmatter.type ?? "note");
</script>

<div class="panel-title">
  <span class="grow">{t("Thèse")}</span>
  <button class="icon-btn" title={t("Espace thèse : ordre, objectifs, questions")} aria-label={t("Espace thèse")} onclick={() => app.openView("these")}><Icon name="graph" /></button>
  <button class="icon-btn" title={t("Nouvelle question de recherche")} aria-label={t("Nouvelle question de recherche")} onclick={() => app.newQuestion()}><Icon name="list" /></button>
  <button class="icon-btn" title={t("Nouveau chapitre")} aria-label={t("Nouveau chapitre")} onclick={newChapter}><Icon name="plus" /></button>
</div>

<div class="body">
  <section class="summary">
    <div class="big">
      <span class="num">{totalWords.toLocaleString(locale())}</span>
      <span class="muted">{totalTarget ? t("mots rédigés sur {n}", { n: totalTarget.toLocaleString(locale()) }) : t("mots rédigés")}</span>
    </div>
    {#if totalTarget}
      <div class="progress"><span style:width="{Math.min(100, (totalWords / totalTarget) * 100)}%"></span></div>
    {/if}
    <div class="statuses">
      {#each byStatus as [s, n] (s)}
        <button class="pill" onclick={() => app.openView("kanban")}><b>{n}</b> {s}</button>
      {/each}
      {#if !fiches.length}<span class="faint">{t("Pas encore de fiche de lecture.")}</span>{/if}
    </div>
  </section>

  {#each chapters as c (c.id)}
    {@const words = c.note?.words ?? 0}
    <section class="chapter">
      <div class="head">
        <button class="toggle" aria-label={t("Déplier")} onclick={() => (open[c.id] = !open[c.id])}>
          <span class="chev" class:open={open[c.id]}><Icon name="chevron" size={12} stroke={2} /></span>
        </button>
        <button class="title" onclick={() => (c.note ? app.openNote(c.note.path) : (open[c.id] = !open[c.id]))}>
          {c.note?.title ?? c.id}
        </button>
        {#if c.note?.frontmatter.statut}<span class="state">{c.note.frontmatter.statut}</span>{/if}
      </div>
      {#if c.target}
        <div class="progress small" title={t("{a} / {b} mots", { a: words, b: c.target })}><span style:width="{Math.min(100, (words / c.target) * 100)}%"></span></div>
        <div class="faint tiny">{t("{a} / {b} mots", { a: words.toLocaleString(locale()), b: c.target.toLocaleString(locale()) })}</div>
      {/if}
      <div class="faint tiny">
        {tn(c.linked.filter((n) => kind(n) === "fiche").length, "{n} fiche", "{n} fiches")} · {tn(c.linked.filter((n) => kind(n) === "concept").length, "{n} concept", "{n} concepts")} · {tn(c.linked.length, "{n} note rattachée", "{n} notes rattachées")}
      </div>
      {#if open[c.id]}
        <ul>
          {#each c.linked as n (n.path)}
            <li>
              <button onclick={() => app.openNote(n.path)}>
                <span class="k k-{kind(n)}">{kind(n)}</span>
                <span class="nt">{n.title}</span>
                {#if n.frontmatter.statut}<span class="faint">{n.frontmatter.statut}</span>{/if}
              </button>
            </li>
          {:else}
            <li class="faint tiny">{t("Ajoute")} <code>chapitres: [{c.id}]</code> {t("aux propriétés d'une note pour la rattacher.")}</li>
          {/each}
        </ul>
      {/if}
    </section>
  {:else}
    <p class="empty">
      {t("Déclare un chapitre avec les propriétés")}<br /><code>type: chapitre</code> {t("et")} <code>chapitre: ch2</code>,<br />{t("puis rattache fiches et concepts avec")} <code>chapitres: [ch2]</code>.
    </p>
  {/each}
</div>

<style>
  .body {
    flex: 1;
    overflow: auto;
    padding: 0 12px 24px;
  }
  .summary {
    padding: 4px 2px 14px;
    border-bottom: 1px solid var(--line);
    margin-bottom: 8px;
  }
  .big {
    display: flex;
    align-items: baseline;
    gap: 8px;
    flex-wrap: wrap;
  }
  .num {
    font-family: var(--font-editor);
    font-size: 28px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .progress {
    height: 5px;
    border-radius: 3px;
    background: var(--line);
    overflow: hidden;
    margin: 8px 0;
  }
  .progress.small {
    height: 3px;
    margin: 6px 0 3px;
  }
  .progress span {
    display: block;
    height: 100%;
    background: var(--accent);
    border-radius: 3px;
  }
  .statuses {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-top: 4px;
  }
  .pill {
    border: 1px solid var(--line-strong);
    background: var(--bg-raised);
    border-radius: 20px;
    padding: 2px 9px;
    font-size: 12px;
    color: var(--text-soft);
  }
  .pill b {
    color: var(--text);
  }
  .chapter {
    padding: 10px 2px;
    border-bottom: 1px solid var(--line);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .toggle {
    border: 0;
    background: transparent;
    padding: 2px;
    color: var(--text-faint);
    display: inline-grid;
  }
  .chev {
    display: inline-grid;
    transition: transform 0.15s;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .title {
    flex: 1;
    border: 0;
    background: transparent;
    text-align: left;
    font-family: var(--font-editor);
    font-weight: 600;
    font-size: 14px;
    padding: 0;
  }
  .title:hover {
    color: var(--accent-text);
  }
  .state {
    font-size: 11px;
    color: var(--text-soft);
    border: 1px solid var(--line-strong);
    border-radius: 10px;
    padding: 0 6px;
  }
  .tiny {
    font-size: 11.5px;
    padding-left: 20px;
  }
  .chapter .progress.small {
    margin-left: 20px;
  }
  ul {
    list-style: none;
    margin: 6px 0 0;
    padding: 0 0 0 16px;
  }
  li button {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    border: 0;
    background: transparent;
    padding: 4px;
    border-radius: 5px;
    text-align: left;
    font-size: 12.5px;
  }
  li button:hover {
    background: var(--bg-sunk);
  }
  .nt {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .k {
    font-size: 10px;
    border-radius: 4px;
    padding: 0 4px;
    background: var(--bg-sunk);
    color: var(--text-soft);
  }
  .k-fiche {
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  .k-concept {
    background: color-mix(in srgb, #2c8c99 18%, transparent);
  }
</style>
