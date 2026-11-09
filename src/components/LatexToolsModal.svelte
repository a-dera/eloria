<script lang="ts">
  import katex from "katex";
  import * as pdfjs from "pdfjs-dist";
  import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
  import { app } from "../lib/state.svelte";
  import { api, assetUrl, isTauri } from "../lib/api";
  import { t, translateError } from "../lib/i18n.svelte";
  import { BUILTIN, SYMBOLS, tableCode } from "../lib/formulas";
  import type { TexIssue } from "../lib/types";
  import Modal from "./Modal.svelte";

  pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

  // LaTeX tools: formulas (library, palette, preview), table generator, TikZ
  // preview compiled on the computer, cross-references to the document's labels. In a note,
  // formulas are inserted between $$ and tables as Markdown.
  const req = app.latexToolsFor!;
  const isTex = /\.tex$/i.test(req.path);
  let tab = $state(req.tab);

  // ---- Formules
  let mine = $state<{ name: string; latex: string; group: string }[]>([]);
  let formula = $state("");
  let query = $state("");
  let area = $state<HTMLTextAreaElement>();
  api.formulas().then((f) => (mine = f)).catch(() => {});
  const fold = (s: string) => s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
  const library = $derived([...mine.map((m) => ({ ...m, group: t("Mes formules") })), ...BUILTIN].filter((f) => !query.trim() || fold(`${f.name} ${f.group}`).includes(fold(query.trim()))));
  const groups = $derived([...new Set(library.map((f) => f.group))]);
  const preview = $derived.by(() => {
    if (!formula.trim()) return "";
    try {
      return katex.renderToString(formula, { displayMode: true, throwOnError: true });
    } catch (e) {
      return `<span class="kerr">${String((e as Error).message).replace(/</g, "&lt;")}</span>`;
    }
  });
  function symbol(code: string) {
    const el = area;
    if (!el) return;
    const [before, after] = code.split("▢");
    const s = el.selectionStart;
    const e = el.selectionEnd;
    const sel = formula.slice(s, e);
    formula = formula.slice(0, s) + before + sel + (after ?? "") + formula.slice(e);
    requestAnimationFrame(() => {
      el.focus();
      const pos = s + before.length + sel.length;
      el.setSelectionRange(pos, pos);
    });
  }
  function insertFormula() {
    const code = isTex ? `\\begin{equation}\n  ${formula.trim()}\n  \\label{eq:}\n\\end{equation}` : `$$\n${formula.trim()}\n$$`;
    insert(code);
  }
  async function saveFormula() {
    const name = await app.ask(t("Nom de la formule"), "");
    if (!name?.trim()) return;
    try {
      await api.addFormula(name.trim(), formula.trim());
      mine = await api.formulas();
      app.toast(t("Formule ajoutée à ta bibliothèque (note « Formules »)"), "success");
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }

  // ---- Tableau
  let rows = $state(3);
  let cols = $state(3);
  let header = $state(true);
  let cells = $state<string[][]>(Array.from({ length: 3 }, () => Array(3).fill("")));
  let align = $state<("l" | "c" | "r")[]>(["l", "c", "c"]);
  let caption = $state("");
  let label = $state("");
  $effect(() => {
    const r = Math.max(1, Math.min(30, rows));
    const c = Math.max(1, Math.min(12, cols));
    if (cells.length !== r || cells[0]?.length !== c) {
      cells = Array.from({ length: r }, (_, i) => Array.from({ length: c }, (_, j) => cells[i]?.[j] ?? ""));
      align = Array.from({ length: c }, (_, j) => align[j] ?? (j === 0 ? "l" : "c"));
    }
  });
  const tableOut = $derived(tableCode(cells, { header, align, latex: isTex, caption, label }));

  // ---- TikZ
  const TIKZ: [string, string][] = [
    [t("Chaîne de traitement"), "\\begin{tikzpicture}[node distance=12mm, box/.style={draw, rounded corners, minimum height=8mm, inner xsep=3mm}]\n  \\node[box] (a) {Transactions};\n  \\node[box, right=of a] (b) {Graphe};\n  \\node[box, right=of b] (c) {GNN};\n  \\node[box, right=of c] (d) {Score de fraude};\n  \\draw[-Stealth] (a) -- (b);\n  \\draw[-Stealth] (b) -- (c);\n  \\draw[-Stealth] (c) -- (d);\n\\end{tikzpicture}"],
    [t("Graphe de comptes"), "\\begin{tikzpicture}[acc/.style={circle, draw, minimum size=7mm}, mule/.style={acc, fill=red!20}]\n  \\node[acc] (v) at (0,0) {V};\n  \\node[mule] (m1) at (2,1) {M1};\n  \\node[mule] (m2) at (2,-1) {M2};\n  \\node[acc] (f) at (4,0) {F};\n  \\draw[-Stealth] (v) -- (m1);\n  \\draw[-Stealth] (v) -- (m2);\n  \\draw[-Stealth] (m1) -- (f);\n  \\draw[-Stealth] (m2) -- (f);\n\\end{tikzpicture}"],
  ];
  let tikz = $state(TIKZ[0][1]);
  let tikzBusy = $state(false);
  let tikzIssues = $state<TexIssue[]>([]);
  let tikzError = $state("");
  let canvas = $state<HTMLCanvasElement>();
  async function previewTikz() {
    tikzBusy = true;
    tikzError = "";
    tikzIssues = [];
    try {
      const r = await api.latexPreview(tikz);
      tikzIssues = r.issues.filter((i) => i.level === "error");
      if (!r.pdf) {
        tikzError = t("La compilation a échoué.");
        return;
      }
      const url = isTauri ? `${await assetUrl(r.pdf)}?v=${Date.now()}` : r.pdf;
      const data = await fetch(url).then((x) => x.arrayBuffer());
      const task = pdfjs.getDocument({ data });
      const doc = await task.promise;
      const page = await doc.getPage(1);
      const ratio = window.devicePixelRatio || 1;
      const vp = page.getViewport({ scale: 1.6 });
      const c = canvas!;
      c.width = Math.floor(vp.width * ratio);
      c.height = Math.floor(vp.height * ratio);
      c.style.width = `${vp.width}px`;
      c.style.height = `${vp.height}px`;
      await page.render({ canvas: c, viewport: vp, transform: ratio !== 1 ? [ratio, 0, 0, ratio, 0, 0] : undefined }).promise;
      void task.destroy();
    } catch (e) {
      tikzError = translateError(e);
    } finally {
      tikzBusy = false;
    }
  }

  // ---- Renvois
  let labels = $state<string[] | null>(null);
  $effect(() => {
    if (tab === "renvois" && isTex && labels === null) api.latexLabels(req.path).then((l) => (labels = l)).catch(() => (labels = []));
  });
  const KIND: Record<string, string> = { fig: t("Figures"), tab: t("Tableaux"), eq: t("Équations"), sec: t("Sections"), chap: t("Chapitres") };
  const kindOf = (l: string) => (l.includes(":") ? l.split(":")[0] : "");
  function refCode(l: string) {
    const k = kindOf(l);
    if (k === "fig") return `${t("Figure")}~\\ref{${l}}`;
    if (k === "tab") return `${t("Tableau")}~\\ref{${l}}`;
    if (k === "eq") return `\\eqref{${l}}`;
    if (k === "sec" || k === "chap") return `${t("Section")}~\\ref{${l}}`;
    return `\\ref{${l}}`;
  }

  function insert(code: string) {
    if (app.insertInEditor(code)) {
      app.latexToolsFor = null;
      app.toast(t("Inséré dans le document"), "success");
    } else app.toast(t("Ouvre d'abord une note ou un document LaTeX"), "info");
  }
</script>

<Modal title={t("Outils LaTeX")} width={860} onclose={() => (app.latexToolsFor = null)}>
  <div class="body">
    <div class="tabs" role="tablist" aria-label={t("Outil")}>
      <button role="tab" aria-selected={tab === "formules"} class:on={tab === "formules"} onclick={() => (tab = "formules")}>{t("Formules")}</button>
      <button role="tab" aria-selected={tab === "tableau"} class:on={tab === "tableau"} onclick={() => (tab = "tableau")}>{t("Tableau")}</button>
      <button role="tab" aria-selected={tab === "tikz"} class:on={tab === "tikz"} onclick={() => (tab = "tikz")}>{t("Aperçu TikZ")}</button>
      <button role="tab" aria-selected={tab === "renvois"} class:on={tab === "renvois"} onclick={() => (tab = "renvois")} disabled={!isTex}>{t("Renvois")}</button>
    </div>

    {#if tab === "formules"}
      <div class="fgrid">
        <div class="lib">
          <input class="input" placeholder={t("Chercher une formule")} aria-label={t("Chercher une formule")} bind:value={query} />
          <div class="list">
            {#each groups as g (g)}
              <div class="g">{g}</div>
              {#each library.filter((f) => f.group === g) as f (f.group + f.name)}
                <button class="fl" title={f.latex} onclick={() => (formula = f.latex)}>{f.name}</button>
              {/each}
            {/each}
          </div>
        </div>
        <div class="ed">
          {#each SYMBOLS as s (s.group)}
            <div class="syms" aria-label={s.group}>
              {#each s.items as [label, code] (code)}
                <button class="sym" title={code.replace("▢", "")} onclick={() => symbol(code)}>{label}</button>
              {/each}
            </div>
          {/each}
          <textarea bind:this={area} rows="4" bind:value={formula} placeholder={t("Formule LaTeX, par exemple F_1 = 2PR / (P+R)")} aria-label={t("Formule LaTeX")}></textarea>
          <div class="kprev" aria-label={t("Aperçu de la formule")}>{#if preview}{@html preview}{:else}<span class="faint">{t("L'aperçu s'affiche ici.")}</span>{/if}</div>
        </div>
      </div>
    {:else if tab === "tableau"}
      <div class="tbar">
        <label>{t("Lignes")} <input type="number" min="1" max="30" bind:value={rows} /></label>
        <label>{t("Colonnes")} <input type="number" min="1" max="12" bind:value={cols} /></label>
        <label class="ck"><input type="checkbox" bind:checked={header} /> {t("Première ligne = en-tête")}</label>
        {#if isTex}
          <input class="input" placeholder={t("Légende")} aria-label={t("Légende")} bind:value={caption} />
          <input class="input lbl" placeholder={t("étiquette")} aria-label={t("Étiquette")} bind:value={label} />
        {/if}
      </div>
      <div class="grid" style:grid-template-columns="repeat({cells[0]?.length ?? 1}, minmax(70px, 1fr))">
        {#each align as _a, j (j)}
          <select bind:value={align[j]} aria-label={t("Alignement de la colonne {n}", { n: j + 1 })}>
            <option value="l">{t("gauche")}</option><option value="c">{t("centre")}</option><option value="r">{t("droite")}</option>
          </select>
        {/each}
        {#each cells as row, i (i)}
          {#each row as _c, j (j)}
            <input class="cell" class:head={header && i === 0} bind:value={cells[i][j]} aria-label={t("Cellule {r}, {c}", { r: i + 1, c: j + 1 })} />
          {/each}
        {/each}
      </div>
      <pre class="code">{tableOut}</pre>
    {:else if tab === "tikz"}
      <div class="tpl">
        {#each TIKZ as [name, code] (name)}<button class="btn small ghost" onclick={() => (tikz = code)}>{name}</button>{/each}
      </div>
      <textarea rows="8" class="mono" bind:value={tikz} aria-label={t("Code TikZ")}></textarea>
      <div class="row">
        <button class="btn" disabled={tikzBusy || !tikz.trim()} onclick={previewTikz}>{tikzBusy ? t("Compilation…") : t("Aperçu")}</button>
        <span class="faint">{t("Compilé sur ton ordinateur, à part du document.")}</span>
      </div>
      {#if tikzError}<p class="error">{tikzError}</p>{/if}
      {#each tikzIssues as i, k (k)}<p class="error">{i.line ? t("ligne {n}", { n: i.line }) + " : " : ""}{i.message}</p>{/each}
      <div class="tikzprev"><canvas bind:this={canvas}></canvas></div>
    {:else}
      <p class="hint">{t("Clique une étiquette pour insérer le renvoi au curseur.")}</p>
      {#if labels === null}
        <p class="faint">{t("Lecture des étiquettes…")}</p>
      {:else if !labels.length}
        <p class="faint">{t("Aucune étiquette \\label{} dans ce document ni dans ses chapitres.")}</p>
      {:else}
        {#each [...new Set(labels.map(kindOf))] as k (k)}
          <div class="g">{KIND[k] ?? (k || t("Autres"))}</div>
          <div class="refs">
            {#each labels.filter((l) => kindOf(l) === k) as l (l)}<button class="btn small" title={refCode(l)} onclick={() => insert(refCode(l))}>{l}</button>{/each}
          </div>
        {/each}
      {/if}
    {/if}
  </div>
  {#snippet footer()}
    {#if tab === "formules"}<button class="btn" disabled={!formula.trim()} onclick={saveFormula}>{t("Ajouter à mes formules")}</button>{/if}
    <button class="btn" onclick={() => (app.latexToolsFor = null)}>{t("Fermer")}</button>
    {#if tab === "formules"}
      <button class="btn primary" disabled={!formula.trim()} onclick={insertFormula}>{t("Insérer")}</button>
    {:else if tab === "tableau"}
      <button class="btn primary" onclick={() => insert(tableOut)}>{t("Insérer")}</button>
    {:else if tab === "tikz"}
      <button class="btn primary" disabled={!tikz.trim()} onclick={() => insert(isTex ? `\\begin{figure}[ht]\n  \\centering\n${tikz}\n  \\caption{}\n  \\label{fig:}\n\\end{figure}` : `\`\`\`latex\n${tikz}\n\`\`\``)}>{t("Insérer")}</button>
    {/if}
  {/snippet}
</Modal>

<style>
  .body {
    padding: 12px 22px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-height: 72vh;
    overflow: auto;
  }
  .tabs {
    display: flex;
    gap: 4px;
    border-bottom: 1px solid var(--line);
    padding-bottom: 8px;
  }
  .tabs button {
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    padding: 4px 10px;
    font: 13px var(--font-ui);
    color: var(--text-soft);
    cursor: pointer;
  }
  .tabs button.on {
    background: var(--accent-soft);
    color: var(--text);
    font-weight: 600;
  }
  .tabs button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .fgrid {
    display: grid;
    grid-template-columns: minmax(180px, 240px) minmax(0, 1fr);
    gap: 14px;
    min-height: 0;
  }
  .lib {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .list {
    max-height: 46vh;
    overflow: auto;
  }
  .g {
    margin: 8px 0 2px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-faint);
  }
  .fl {
    display: block;
    width: 100%;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    padding: 3px 6px;
    text-align: left;
    font: 13px var(--font-ui);
    color: var(--text);
    cursor: pointer;
  }
  .fl:hover {
    background: var(--bg-soft);
  }
  .ed {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .syms {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
  }
  .sym {
    min-width: 30px;
    border: 1px solid var(--line);
    border-radius: var(--radius-s);
    background: var(--bg-raised);
    padding: 2px 6px;
    font: 12.5px var(--font-ui);
    color: var(--text);
    cursor: pointer;
  }
  .sym:hover {
    border-color: var(--accent);
  }
  textarea {
    width: 100%;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg-raised);
    color: var(--text);
    font: 13px var(--font-mono);
    padding: 8px;
    resize: vertical;
  }
  .kprev {
    min-height: 70px;
    padding: 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
    overflow-x: auto;
  }
  .kprev :global(.kerr) {
    color: var(--danger);
    font-size: 12.5px;
  }
  .tbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    font-size: 13px;
  }
  .tbar input[type="number"] {
    width: 56px;
  }
  .tbar .lbl {
    width: 140px;
  }
  .ck {
    display: inline-flex;
    gap: 5px;
    align-items: center;
  }
  .grid {
    display: grid;
    gap: 3px;
    overflow-x: auto;
  }
  .grid select,
  .cell {
    min-width: 0;
    border: 1px solid var(--line);
    border-radius: var(--radius-s);
    background: var(--bg-raised);
    color: var(--text);
    font: 13px var(--font-ui);
    padding: 4px 6px;
  }
  .cell.head {
    font-weight: 600;
    background: var(--bg-soft);
  }
  .code {
    margin: 0;
    padding: 8px 10px;
    border-radius: var(--radius);
    background: var(--code-bg);
    font: 12px/1.45 var(--font-mono);
    white-space: pre-wrap;
    max-height: 22vh;
    overflow: auto;
  }
  .tpl,
  .row,
  .refs {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .tikzprev {
    overflow: auto;
    max-height: 40vh;
    border: 1px solid var(--line);
    border-radius: var(--radius);
  }
  .tikzprev canvas {
    display: block;
  }
  .hint,
  .faint {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-soft);
  }
  .faint {
    color: var(--text-faint);
  }
  .error {
    margin: 0;
    color: var(--danger);
    font-size: 12.5px;
  }
</style>
