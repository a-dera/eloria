<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, tn, translateError } from "../lib/i18n.svelte";
  import type { Matrix } from "../lib/types";

  // Research matrix: one row per reading note, one column per section. Clicking a
  // cell opens the reading note at that section; empty cells are filled in the note
  // (by hand or with "Eloria method").
  let { citekeys = null, visible = true }: { citekeys?: string[] | null; visible?: boolean } = $props();
  let matrix = $state<Matrix | null>(null);
  let error = $state("");
  let filter = $state("");
  let sortCol = $state(-1);
  let sortDir = $state(1);
  let onlyEmpty = $state(false);

  $effect(() => {
    void app.revision;
    if (!visible) return;
    api.researchMatrix(citekeys).then((m) => (matrix = m)).catch((e) => (error = translateError(e)));
  });

  const rows = $derived.by(() => {
    if (!matrix) return [];
    const q = filter.trim().toLowerCase();
    let r = matrix.rows.filter((x) => !q || [x.author_year, x.title, x.citekey, ...x.cells].some((c) => c.toLowerCase().includes(q)));
    if (onlyEmpty) r = r.filter((x) => x.cells.some((c) => !c));
    if (sortCol >= 0) {
      r = [...r].sort((a, b) => {
        const va = sortCol === 0 ? a.author_year : a.cells[sortCol - 1];
        const vb = sortCol === 0 ? b.author_year : b.cells[sortCol - 1];
        return va.localeCompare(vb, undefined, { sensitivity: "base" }) * sortDir;
      });
    }
    return r;
  });

  const filled = $derived(matrix ? matrix.rows.reduce((n, r) => n + r.cells.filter(Boolean).length, 0) : 0);
  const total = $derived(matrix ? matrix.rows.length * matrix.columns.length : 0);

  function sort(i: number) {
    if (sortCol === i) sortDir = -sortDir;
    else {
      sortCol = i;
      sortDir = 1;
    }
  }

  async function exportAs(format: "csv" | "md" | "tex") {
    try {
      const p = await api.exportMatrix(format, citekeys);
      await app.refresh();
      app.toast(t("Matrice exportée : {path}", { path: p }), "success");
      if (format === "md") app.openNote(p, { newTab: true });
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }

  function open(path: string, heading?: string) {
    app.openNote(path, { newTab: true, heading });
  }
</script>

<div class="mx">
  <header>
    <h2>{t("Matrice de recherche")}</h2>
    {#if matrix}
      <span class="faint">{tn(matrix.rows.length, "{n} fiche", "{n} fiches")} · {t("{pct} % des cases remplies", { pct: total ? Math.round((filled / total) * 100) : 0 })}</span>
    {/if}
    <span class="grow"></span>
    <input class="input" bind:value={filter} placeholder={t("Filtrer (auteur, méthode, mot…)")} aria-label={t("Filtrer la matrice")} />
    <label class="check"><input type="checkbox" bind:checked={onlyEmpty} /> {t("Cases à remplir")}</label>
    <button class="btn small" onclick={() => exportAs("csv")}>CSV</button>
    <button class="btn small" onclick={() => exportAs("md")}>Markdown</button>
    <button class="btn small" onclick={() => exportAs("tex")}>LaTeX</button>
  </header>
  {#if error}<p class="error">{error}</p>{/if}
  {#if !matrix && !error}<p class="loading" role="status">{t("Lecture des fiches…")}</p>{/if}
  {#if matrix}
    {#if !matrix.rows.length}
      <p class="empty">{t("Aucune fiche pour l'instant. Ouvre une source et clique sur « Créer une fiche ».")}</p>
    {:else}
      <div class="scroll">
        <table>
          <thead>
            <tr>
              <th class="first"><button onclick={() => sort(0)}>{t("Article")} {sortCol === 0 ? (sortDir > 0 ? "▲" : "▼") : ""}</button></th>
              {#each matrix.columns as [id, title], i (id)}
                <th><button onclick={() => sort(i + 1)}>{title} {sortCol === i + 1 ? (sortDir > 0 ? "▲" : "▼") : ""}</button></th>
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each rows as r (r.path)}
              <tr>
                <td class="first">
                  <button class="who" onclick={() => open(r.path)} title={r.title}>{r.author_year}</button>
                  {#if r.statut}<span class="statut">{r.statut}</span>{/if}
                </td>
                {#each r.cells as c, i (i)}
                  <td class:empty-cell={!c}>
                    <button class="cell" onclick={() => open(r.path, matrix!.columns[i][1])} title={c || t("Vide : ouvrir la fiche pour compléter")}>{c || "·"}</button>
                  </td>
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/if}
</div>

<style>
  .mx {
    height: 100%;
    display: flex;
    flex-direction: column;
    padding: 14px 18px;
    gap: 10px;
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  h2 {
    margin: 0;
    font-size: 17px;
  }
  .grow {
    flex: 1;
  }
  .faint {
    color: var(--text-faint);
    font-size: 12.5px;
  }
  .input {
    width: 240px;
  }
  .check {
    font-size: 12.5px;
    display: inline-flex;
    gap: 5px;
    align-items: center;
  }
  .scroll {
    flex: 1;
    overflow: auto;
    border: 1px solid var(--line);
    border-radius: var(--radius);
  }
  table {
    border-collapse: separate;
    border-spacing: 0;
    font-size: 12.5px;
    line-height: 1.45;
  }
  th,
  td {
    border-right: 1px solid var(--line);
    border-bottom: 1px solid var(--line);
    vertical-align: top;
    min-width: 190px;
    max-width: 260px;
    padding: 0;
  }
  th {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--bg-soft);
  }
  th button {
    width: 100%;
    padding: 7px 10px;
    border: 0;
    background: none;
    font: 600 12px var(--font-ui);
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .first {
    position: sticky;
    left: 0;
    z-index: 2;
    background: var(--bg-soft);
    min-width: 170px;
  }
  th.first {
    z-index: 3;
  }
  .who {
    display: block;
    padding: 7px 10px 2px;
    border: 0;
    background: none;
    color: var(--accent-text);
    font: 600 12.5px var(--font-ui);
    text-align: left;
    cursor: pointer;
  }
  .statut {
    display: inline-block;
    margin: 0 10px 6px;
    font-size: 11px;
    color: var(--text-faint);
  }
  .cell {
    display: block;
    width: 100%;
    min-height: 100%;
    padding: 7px 10px;
    border: 0;
    background: none;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
    display: -webkit-box;
    -webkit-line-clamp: 6;
    line-clamp: 6;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .cell:hover {
    background: var(--bg-sunk);
  }
  .empty-cell .cell {
    color: var(--text-faint);
    text-align: center;
  }
  .empty,
  .error {
    color: var(--text-soft);
  }
  .error {
    color: var(--danger);
  }
</style>
