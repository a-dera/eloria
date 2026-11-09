<script lang="ts">
  import { t, tn } from "../lib/i18n.svelte";
  import { app } from "../lib/state.svelte";
  import type { NoteMeta } from "../lib/types";

  let mode = $state<"publication" | "lecture">("publication");

  function yearOf(n: NoteMeta): number | null {
    if (mode === "publication") {
      const y = Number(n.frontmatter.annee ?? n.frontmatter.year);
      return Number.isFinite(y) && y > 1000 ? y : null;
    }
    const d = String(n.frontmatter.lu_le ?? n.frontmatter.cree_le ?? "");
    const m = /^(\d{4})/.exec(d);
    return m ? Number(m[1]) : new Date(n.mtime > 1e11 ? n.mtime : n.mtime * 1000).getFullYear();
  }

  const fiches = $derived(app.notes.filter((n) => n.frontmatter.type === "fiche"));
  const groups = $derived.by(() => {
    const m = new Map<number, NoteMeta[]>();
    const undated: NoteMeta[] = [];
    for (const n of fiches) {
      const y = yearOf(n);
      if (y === null) undated.push(n);
      else {
        if (!m.has(y)) m.set(y, []);
        m.get(y)!.push(n);
      }
    }
    return { years: [...m.entries()].sort((a, b) => a[0] - b[0]), undated };
  });
  const maxCount = $derived(Math.max(1, ...groups.years.map(([, l]) => l.length)));
</script>

<div class="tl">
  <div class="toolbar">
    <div class="seg">
      <button class:on={mode === "publication"} onclick={() => (mode = "publication")}>{t("Par année de publication")}</button>
      <button class:on={mode === "lecture"} onclick={() => (mode = "lecture")}>{t("Par année de lecture")}</button>
    </div>
    <span class="faint">{tn(fiches.length, "{n} fiche de lecture", "{n} fiches de lecture")}</span>
  </div>

  <div class="scroll">
    {#if !fiches.length}
      <p class="empty">{t("Aucune fiche de lecture. Crée-en depuis le panneau Zotero.")}</p>
    {/if}
    <ol class="line">
      {#each groups.years as [year, list] (year)}
        <li>
          <div class="year">
            <span class="y">{year}</span>
            <span class="density"><span style:width="{(list.length / maxCount) * 100}%"></span></span>
          </div>
          <div class="items">
            {#each list as n (n.path)}
              <button class="it" onclick={(e) => app.openNote(n.path, { newTab: true, otherPane: e.altKey })}>
                <span class="t">{n.title}</span>
                <span class="faint">
                  {Array.isArray(n.frontmatter.auteurs) ? (n.frontmatter.auteurs as string[]).slice(0, 2).join(", ") : ""}
                  {#if n.frontmatter.statut}<span class="st">{n.frontmatter.statut}</span>{/if}
                </span>
              </button>
            {/each}
          </div>
        </li>
      {/each}
      {#if groups.undated.length}
        <li>
          <div class="year"><span class="y faint">{t("s.d.")}</span></div>
          <div class="items">
            {#each groups.undated as n (n.path)}
              <button class="it" onclick={() => app.openNote(n.path, { newTab: true })}><span class="t">{n.title}</span></button>
            {/each}
          </div>
        </li>
      {/if}
    </ol>
  </div>
</div>

<style>
  .tl {
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
    font-size: 12.5px;
  }
  .seg {
    display: inline-flex;
    background: var(--bg-sunk);
    border-radius: var(--radius);
    padding: 2px;
  }
  .seg button {
    border: 0;
    background: transparent;
    padding: 4px 10px;
    border-radius: 5px;
    color: var(--text-soft);
  }
  .seg button.on {
    background: var(--bg-raised);
    color: var(--text);
    box-shadow: 0 1px 2px rgb(0 0 0 / 8%);
  }
  .scroll {
    flex: 1;
    overflow: auto;
    padding: 24px 32px 60px;
  }
  .line {
    list-style: none;
    margin: 0 auto;
    padding: 0;
    max-width: 860px;
    position: relative;
  }
  .line::before {
    content: "";
    position: absolute;
    left: 92px;
    top: 6px;
    bottom: 0;
    width: 2px;
    background: var(--line);
  }
  li {
    display: grid;
    grid-template-columns: 84px 1fr;
    gap: 28px;
    margin-bottom: 22px;
    position: relative;
  }
  li::before {
    content: "";
    position: absolute;
    left: 88px;
    top: 6px;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 3px var(--bg);
  }
  .year {
    text-align: right;
  }
  .y {
    font-family: var(--font-editor);
    font-size: 20px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .density {
    display: block;
    height: 3px;
    background: var(--line);
    border-radius: 2px;
    margin-top: 4px;
    margin-left: auto;
    width: 60px;
    overflow: hidden;
  }
  .density span {
    display: block;
    height: 100%;
    margin-left: auto;
    background: var(--accent);
    opacity: 0.55;
  }
  .items {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .it {
    text-align: left;
    border: 0;
    background: transparent;
    border-radius: 6px;
    padding: 5px 10px;
    display: flex;
    flex-direction: column;
  }
  .it:hover {
    background: var(--bg-soft);
  }
  .t {
    font-family: var(--font-editor);
    font-size: 15px;
  }
  .st {
    margin-left: 6px;
    font-size: 11px;
    border: 1px solid var(--line-strong);
    border-radius: 9px;
    padding: 0 6px;
  }
</style>
