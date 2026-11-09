<script lang="ts">
  import { t, tk, tn, translateError } from "../lib/i18n.svelte";
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { editors } from "../lib/editor/registry";
  import type { NoteMeta } from "../lib/types";
  import { propText, STATUS_ORDER } from "../lib/util";

  let typeFilter = $state("fiche");
  let groupBy = $state("statut");
  let dragging = $state<string | null>(null);
  let over = $state<string | null>(null);

  const NONE = tk("Sans valeur");

  const types = $derived([...new Set(app.notes.map((n) => String(n.frontmatter.type ?? "")).filter(Boolean))].sort());
  const scoped = $derived(
    app.notes.filter((n) => !n.path.startsWith((app.settings?.templates_folder ?? "Modèles") + "/") && (!typeFilter || n.frontmatter.type === typeFilter)),
  );
  const keys = $derived(
    [...new Set(scoped.flatMap((n) => Object.keys(n.frontmatter)))]
      .filter((k) => !["titre", "title", "citekey", "zotero", "auteurs", "aliases", "cree_le", "type"].includes(k))
      .sort(),
  );

  const columns = $derived.by(() => {
    const vals = new Set<string>();
    for (const n of scoped) vals.add(colOf(n));
    let ordered = [...vals].filter((v) => v !== NONE).sort((a, b) => a.localeCompare(b, "fr"));
    if (groupBy === "statut") {
      ordered = [...STATUS_ORDER, ...ordered.filter((v) => !STATUS_ORDER.includes(v))];
    }
    if (vals.has(NONE)) ordered.push(NONE);
    return ordered.map((v) => ({ value: v, cards: scoped.filter((n) => colOf(n) === v) }));
  });

  function colOf(n: NoteMeta): string {
    const v = n.frontmatter[groupBy];
    if (v === undefined || v === null || v === "") return NONE;
    return Array.isArray(v) ? (v.length ? propText(v[0]) : NONE) : propText(v);
  }

  async function moveTo(path: string, value: string) {
    const n = app.noteByPath(path);
    if (!n || colOf(n) === value) return;
    await editors.savePath(path);
    try {
      const content = await api.setProperty(path, groupBy, value === NONE ? null : value);
      editors.pushContent(path, content);
      await app.refresh();
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }

  async function addCard(value: string) {
    const title = await app.ask(t("Titre de la note"), "");
    if (!title) return;
    const fm = [`type: ${typeFilter || "note"}`, value !== NONE ? `${groupBy}: ${value}` : ""].filter(Boolean).join("\n");
    const folder = typeFilter === "fiche" ? (app.settings?.fiches_folder ?? "") : "";
    await app.newNote(folder, title, `---\n${fm}\n---\n# ${title}\n\n`);
  }

  const prioClass = (p: unknown) => (p === "haute" ? "hi" : p === "basse" ? "lo" : "");
</script>

<div class="kanban">
  <div class="toolbar">
    <label>{t("Notes de type")}
      <select class="input" bind:value={typeFilter}>
        <option value="">{t("toutes")}</option>
        {#each types as ty (ty)}<option value={ty}>{ty}</option>{/each}
      </select>
    </label>
    <label>{t("Colonnes selon")}
      <select class="input" bind:value={groupBy}>
        {#each [...new Set(["statut", ...keys])] as k (k)}<option value={k}>{k}</option>{/each}
      </select>
    </label>
    <span class="faint">{tn(scoped.length, "{n} carte", "{n} cartes")} · {t("glisse une carte pour changer sa propriété « {prop} »", { prop: groupBy })}</span>
  </div>

  <div class="board">
    {#each columns as col (col.value)}
      <section
        class="col"
        class:over={over === col.value}
        aria-label={col.value}
        ondragover={(e) => {
          e.preventDefault();
          over = col.value;
        }}
        ondragleave={() => (over = null)}
        ondrop={(e) => {
          e.preventDefault();
          over = null;
          const p = e.dataTransfer?.getData("text/x-eloria-card");
          if (p) moveTo(p, col.value);
        }}
      >
        <header>
          <span class="name">{col.value === NONE ? t(NONE) : col.value}</span>
          <span class="count">{col.cards.length}</span>
        </header>
        <div class="cards">
          {#each col.cards as n (n.path)}
            <div
              class="card"
              class:dragging={dragging === n.path}
              draggable="true"
              role="button"
              tabindex="0"
              ondragstart={(e) => {
                dragging = n.path;
                e.dataTransfer?.setData("text/x-eloria-card", n.path);
              }}
              ondragend={() => (dragging = null)}
              onclick={(e) => app.openNote(n.path, { newTab: true, otherPane: e.altKey })}
              onkeydown={(e) => e.key === "Enter" && app.openNote(n.path, { newTab: true })}
            >
              <div class="t">{n.title}</div>
              <div class="m">
                {#if n.frontmatter.annee}<span>{n.frontmatter.annee}</span>{/if}
                {#if n.frontmatter.citekey}<span class="ck">@{n.frontmatter.citekey}</span>{/if}
              </div>
              <div class="chips">
                {#if n.frontmatter.priorite}<span class="chip prio {prioClass(n.frontmatter.priorite)}">{n.frontmatter.priorite}</span>{/if}
                {#each (Array.isArray(n.frontmatter.chapitres) ? n.frontmatter.chapitres : []) as c (c)}<span class="chip">{c}</span>{/each}
              </div>
            </div>
          {/each}
          <button class="add" onclick={() => addCard(col.value)}>+ {t("Ajouter")}</button>
        </div>
      </section>
    {/each}
  </div>
</div>

<style>
  .kanban {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 18px;
    border-bottom: 1px solid var(--line);
    color: var(--text-soft);
    font-size: 12.5px;
    flex-wrap: wrap;
  }
  .toolbar label {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .toolbar .input {
    width: auto;
    height: 28px;
  }
  .board {
    flex: 1;
    display: flex;
    gap: 14px;
    padding: 16px 18px;
    overflow-x: auto;
    align-items: flex-start;
  }
  .col {
    width: 272px;
    flex-shrink: 0;
    background: var(--bg-soft);
    border-radius: var(--radius-l);
    display: flex;
    flex-direction: column;
    max-height: 100%;
    border: 1px solid transparent;
    transition: border-color 0.12s;
  }
  .col.over {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 14px 8px;
  }
  .name {
    font-weight: 600;
  }
  .name::first-letter {
    text-transform: uppercase;
  }
  .count {
    color: var(--text-faint);
    font-variant-numeric: tabular-nums;
  }
  .cards {
    padding: 0 8px 10px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 7px;
  }
  .card {
    background: var(--bg-raised);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 10px 12px;
    cursor: grab;
  }
  .card:hover {
    border-color: var(--line-strong);
  }
  .card.dragging {
    opacity: 0.4;
  }
  .t {
    font-family: var(--font-editor);
    font-size: 14px;
    line-height: 1.35;
    font-weight: 500;
  }
  .m {
    display: flex;
    gap: 8px;
    margin-top: 4px;
    font-size: 12px;
    color: var(--text-soft);
  }
  .ck {
    color: color-mix(in srgb, #b8721c 85%, var(--text));
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 6px;
  }
  .chips:empty {
    display: none;
  }
  .chip {
    font-size: 11px;
    padding: 0 6px;
    border-radius: 9px;
    background: var(--bg-sunk);
    color: var(--text-soft);
  }
  .prio.hi {
    background: color-mix(in srgb, var(--danger) 16%, transparent);
    color: var(--danger);
  }
  .prio.lo {
    opacity: 0.7;
  }
  .add {
    border: 1px dashed var(--line-strong);
    background: transparent;
    border-radius: var(--radius);
    padding: 6px;
    color: var(--text-faint);
    font-size: 12.5px;
  }
  .add:hover {
    color: var(--text);
    border-color: var(--text-faint);
  }
</style>
