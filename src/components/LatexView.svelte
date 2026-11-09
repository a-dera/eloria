<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { EditorState } from "@codemirror/state";
  import { EditorView, keymap, lineNumbers, highlightActiveLine, drawSelection } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
  import { StreamLanguage, syntaxHighlighting, bracketMatching } from "@codemirror/language";
  import { highlight } from "../lib/editor/setup";
  import { stex } from "@codemirror/legacy-modes/mode/stex";
  import { autocompletion, type CompletionContext } from "@codemirror/autocomplete";
  import { searchKeymap, highlightSelectionMatches } from "@codemirror/search";
  import { app } from "../lib/state.svelte";
  import { api, isTauri } from "../lib/api";
  import { t, tn, translateError } from "../lib/i18n.svelte";
  import type { CompileResult, TexEngine } from "../lib/types";
  import PdfView from "./PdfView.svelte";
  import Resizer from "./Resizer.svelte";
  import Icon from "./Icon.svelte";

  // LaTeX editor: source on the left, PDF on the right, compiled locally (Tectonic, MiKTeX
  // or TeX Live) on every save. Nothing goes to an online service.
  let { path, visible = true }: { path: string; visible?: boolean } = $props();
  let host = $state<HTMLDivElement>();
  let view: EditorView | null = null;
  let saved = "";
  let dirty = $state(false);
  let engine = $state<TexEngine | null | undefined>(undefined);
  let result = $state<CompileResult | null>(null);
  let compiling = $state(false);
  let pdfVersion = $state(0);
  /** SyncTeX: PDF point to show (going from the source to the PDF). */
  let pdfTarget = $state<{ page: number; y: number; n: number } | null>(null);

  /** Source -> PDF: the cursor line in the compiled PDF. */
  async function syncForward() {
    if (!view) return;
    const line = view.state.doc.lineAt(view.state.selection.main.head).number;
    try {
      const r = await api.latexSyncForward(path, line);
      if (r) pdfTarget = { page: r[0], y: r[1], n: (pdfTarget?.n ?? 0) + 1 };
      else app.toast(t("Rien à montrer : compile d'abord le document (la synchronisation vient de la compilation)."), "info");
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }

  /** PDF -> source: double-click in the PDF, the matching line. */
  async function syncInverse(pg: number, y: number) {
    try {
      const r = await api.latexSyncInverse(path, pg, y);
      if (!r) return;
      const [file, line] = r;
      if (file !== path) {
        app.openView("latex", { path: file, title: file.split("/").pop() });
        return;
      }
      if (!view) return;
      const l = view.state.doc.line(Math.min(Math.max(1, line), view.state.doc.lines));
      view.dispatch({ selection: { anchor: l.from, head: l.to }, scrollIntoView: true });
      view.focus();
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }
  let error = $state("");
  let autoCompile = $state(true);
  let installing = $state(false);
  // Layout, remembered between sessions: source share, height and state of the
  // output panel (folded, hidden).
  const LAYOUT = "eloria:latex-layout";
  const layoutSaved = (() => {
    try {
      return JSON.parse(localStorage.getItem(LAYOUT) ?? "{}");
    } catch {
      return {};
    }
  })();
  let ratio = $state<number>(layoutSaved.ratio ?? 0.5);
  let outHeight = $state<number>(layoutSaved.outHeight ?? 160);
  let outOpen = $state<boolean>(layoutSaved.outOpen ?? true);
  let outHidden = $state(false);
  let split = $state<HTMLDivElement>();
  let splitW = $state(1000);
  $effect(() => {
    if (!split) return;
    const ro = new ResizeObserver(() => (splitW = split!.clientWidth));
    ro.observe(split);
    return () => ro.disconnect();
  });
  function saveLayout() {
    try {
      localStorage.setItem(LAYOUT, JSON.stringify({ ratio, outHeight, outOpen }));
    } catch {
      /* stockage indisponible */
    }
  }
  const isTex = $derived(path.toLowerCase().endsWith(".tex"));

  /** Known citation keys (Zotero and Eloria's own sources). */
  function citekeys(): { label: string; detail: string }[] {
    const seen = new Set<string>();
    const out: { label: string; detail: string }[] = [];
    for (const i of app.zoteroItems) {
      if (seen.has(i.citekey)) continue;
      seen.add(i.citekey);
      out.push({ label: i.citekey, detail: `${i.authors}${i.year ? ` (${i.year})` : ""} · ${i.title}` });
    }
    for (const n of app.notes) {
      const ck = n.frontmatter.citekey;
      if (n.frontmatter.type === "source" && typeof ck === "string" && !seen.has(ck)) {
        seen.add(ck);
        out.push({ label: ck, detail: n.title });
      }
    }
    return out;
  }

  const COMMANDS = ["section", "subsection", "subsubsection", "chapter", "paragraph", "textbf", "textit", "emph", "autocite", "textcite", "parencite", "cite", "ref", "eqref", "autoref", "label", "includegraphics", "begin", "end", "item", "footnote", "caption", "centering", "printbibliography", "addbibresource", "usepackage", "newcommand", "frac", "sum", "int", "mathbb", "mathcal", "alpha", "beta", "lambda", "sigma"];

  function complete(ctx: CompletionContext) {
    // \cite{…}, \autocite[p. 3]{…}, several keys separated by commas.
    const cite = ctx.matchBefore(/\\[a-zA-Z]*cite[a-zA-Z]*\*?(?:\[[^\]]*\]){0,2}\{[^}]*$/);
    if (cite) {
      const word = ctx.matchBefore(/[^{,\s]*$/)!;
      return { from: word.from, options: citekeys().map((c) => ({ label: c.label, detail: c.detail, type: "variable" })), validFor: /^[^,}\s]*$/ };
    }
    const ref = ctx.matchBefore(/\\(?:eq|auto|page|c|C)?ref\{[^}]*$/);
    if (ref) {
      const word = ctx.matchBefore(/[^{]*$/)!;
      const labels = [...ctx.state.doc.toString().matchAll(/\\label\{([^}]+)\}/g)].map((m) => m[1]);
      return { from: word.from, options: [...new Set(labels)].map((l) => ({ label: l, type: "constant" })) };
    }
    const env = ctx.matchBefore(/\\(?:begin|end)\{[a-z*]*$/);
    if (env) {
      const word = ctx.matchBefore(/[a-z*]*$/)!;
      return { from: word.from, options: ["figure", "table", "equation", "align", "itemize", "enumerate", "abstract", "tabular", "frame", "quote", "center"].map((e) => ({ label: e, type: "type" })) };
    }
    const cmd = ctx.matchBefore(/\\[a-zA-Z]*$/);
    if (cmd && (cmd.text.length > 1 || ctx.explicit)) return { from: cmd.from + 1, options: COMMANDS.map((c) => ({ label: c, type: "function" })) };
    return null;
  }

  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  function scheduleSave() {
    dirty = true;
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => save(autoCompile), 1200);
  }

  async function save(thenCompile = false) {
    if (!view) return;
    const content = view.state.doc.toString();
    if (content !== saved) {
      try {
        await api.writeText(path, content);
        saved = content;
      } catch (e) {
        error = translateError(e);
        return;
      }
    }
    dirty = false;
    if (thenCompile && isTex && engine) compile();
  }

  async function compile() {
    if (compiling || !isTex) return;
    if (dirty) await save(false);
    compiling = true;
    error = "";
    try {
      result = await api.latexCompile(path);
      if (result.pdf) pdfVersion++;
      // A new compilation shows the output again; opened automatically if there are errors.
      outHidden = false;
      if (result.issues.some((i) => i.level === "error")) outOpen = true;
    } catch (e) {
      error = translateError(e);
    } finally {
      compiling = false;
    }
  }

  function goto(line: number) {
    if (!view || !line) return;
    const l = view.state.doc.line(Math.min(line, view.state.doc.lines));
    view.dispatch({ selection: { anchor: l.from }, scrollIntoView: true });
    view.focus();
  }

  async function installTectonic() {
    installing = true;
    try {
      engine = await api.latexInstallTectonic();
      app.toast(t("Tectonic installé : {v}", { v: engine.version }), "success");
    } catch (e) {
      app.toast(translateError(e), "error");
    } finally {
      installing = false;
    }
  }

  onMount(async () => {
    const enginePromise = api.latexEngine().then((e) => (engine = e)).catch(() => (engine = null));
    try {
      saved = await api.readText(path);
    } catch (e) {
      error = translateError(e);
      return;
    }
    view = new EditorView({
      parent: host!,
      state: EditorState.create({
        doc: saved,
        extensions: [
          lineNumbers(),
          history(),
          drawSelection(),
          highlightActiveLine(),
          highlightSelectionMatches(),
          bracketMatching(),
          StreamLanguage.define(stex),
          syntaxHighlighting(highlight),
          autocompletion({ override: [complete] }),
          EditorView.lineWrapping,
          keymap.of([
            { key: "Mod-s", run: () => (save(true), true) },
            { key: "Mod-Enter", run: () => (compile(), true) },
            { key: "Mod-Alt-j", run: () => (syncForward(), true) },
            indentWithTab,
            ...defaultKeymap,
            ...historyKeymap,
            ...searchKeymap,
          ]),
          EditorView.updateListener.of((u) => {
            if (u.docChanged) scheduleSave();
            if (u.focusChanged && u.view.hasFocus) app.latexEditor = u.view;
          }),
        ],
      }),
    });
    app.latexEditor = view;
    // When a .tex opens: compiled right away, the PDF shows without a click.
    await enginePromise;
    if (isTex && engine) compile();
  });

  onDestroy(() => {
    if (saveTimer) clearTimeout(saveTimer);
    if (view && view.state.doc.toString() !== saved) api.writeText(path, view.state.doc.toString()).catch(() => {});
    if (app.latexEditor === view) app.latexEditor = null;
    view?.destroy();
  });

  const errors = $derived(result?.issues.filter((i) => i.level === "error") ?? []);
  const warnings = $derived(result?.issues.filter((i) => i.level === "warning") ?? []);
</script>

<div class="lx" class:hidden={!visible}>
  <div class="bar">
    <Icon name="code" size={15} />
    <span class="name">{path.split("/").pop()}</span>
    {#if dirty}<span class="faint">{t("modifié")}</span>{/if}
    <span class="grow"></span>
    {#if isTex}
      {#if engine === null}
        <span class="warn">{t("Aucun moteur LaTeX")}</span>
        <button class="btn small primary" disabled={installing || !isTauri} onclick={installTectonic}>{installing ? t("Téléchargement…") : t("Installer Tectonic")}</button>
      {:else if engine}
        <span class="faint" title={engine.path}>{engine.kind}</span>
      {/if}
      <label class="check"><input type="checkbox" bind:checked={autoCompile} /> {t("Compiler à l'enregistrement")}</label>
      <button class="btn small ghost" onclick={() => (app.checkFor = { kind: "checklist", path })}>{t("Avant soumission")}</button>
      <button class="btn small ghost" onclick={() => (app.checkFor = { kind: "similarite", path })}>{t("Similarité")}</button>
      <button class="btn small ghost" title={t("Montrer dans le PDF la ligne du curseur (Ctrl+Alt+J) ; double-clic dans le PDF pour revenir au source")} onclick={syncForward}>{t("Voir dans le PDF")}</button>
      <button class="btn small ghost" title={t("Formules, tableaux, aperçu TikZ, renvois")} onclick={() => (app.latexToolsFor = { path, tab: "formules" })}>{t("Outils")}</button>
      <button class="btn small ghost" title={t("Tableau, équation, TikZ, formule, cohérence des citations (IA)")} onclick={() => app.openScience()}>{t("Assistant")}</button>
      <button class="btn small primary" disabled={compiling || !engine} onclick={compile} title={t("Compiler (Ctrl+Entrée)")}>{compiling ? t("Compilation…") : t("Compiler")}</button>
    {/if}
  </div>
  <div class="split" bind:this={split}>
    <div class="src" bind:this={host} style:flex={isTex ? `0 0 ${ratio * 100}%` : "1"}></div>
    {#if isTex}
      <Resizer
        value={ratio * splitW}
        min={splitW * 0.2}
        max={splitW * 0.8}
        defaultValue={splitW / 2}
        onchange={(v) => (ratio = v / splitW)}
        onend={saveLayout}
        label={t("Partage entre le source et le PDF")}
      />
      <div class="out">
        {#if result?.pdf && isTauri}
          <PdfView path={result.pdf} version={pdfVersion} onPick={syncInverse} target={pdfTarget} />
        {:else}
          <div class="placeholder">
            {#if engine === null}
              <p>{t("La compilation se fait sur ton ordinateur, sans Overleaf ni service en ligne. Installe Tectonic (un seul fichier, environ 30 Mo ; les paquets LaTeX sont téléchargés au premier usage puis gardés en cache), ou MiKTeX / TeX Live.")}</p>
            {:else}
              <p>{t("Compile (Ctrl+Entrée) pour voir le PDF ici.")}</p>
            {/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>
  {#if error}<p class="error">{error}</p>{/if}
  {#if result && !outHidden}
    {#if outOpen}
      <Resizer
        orientation="horizontal"
        value={outHeight}
        min={70}
        max={520}
        defaultValue={160}
        invert
        onchange={(v) => (outHeight = v)}
        oncollapse={() => {
          outOpen = false;
          saveLayout();
        }}
        collapseBelow={50}
        onend={saveLayout}
        label={t("Hauteur de la sortie de compilation")}
      />
    {/if}
    <div class="issues" class:closed={!outOpen} style:height={outOpen ? `${outHeight}px` : undefined}>
      <div class="ihead">
        <button class="fold" aria-expanded={outOpen} onclick={() => { outOpen = !outOpen; saveLayout(); }} title={outOpen ? t("Replier la sortie") : t("Déplier la sortie")} aria-label={outOpen ? t("Replier la sortie") : t("Déplier la sortie")}>
          {outOpen ? "▾" : "▸"}
        </button>
        <span class:ok={result.ok} class:ko={!result.ok}>{result.ok ? t("PDF produit en {s} s", { s: (result.millis / 1000).toFixed(1) }) : t("Échec de la compilation")}</span>
        <span class="faint">{tn(errors.length, "{n} erreur", "{n} erreurs")} · {tn(warnings.length, "{n} avertissement", "{n} avertissements")}</span>
        <span class="grow"></span>
        <button class="icon-btn" title={t("Masquer jusqu'à la prochaine compilation")} aria-label={t("Masquer la sortie")} onclick={() => (outHidden = true)}><Icon name="x" size={13} /></button>
      </div>
      {#if outOpen}
      <ul>
        {#each [...errors, ...warnings].slice(0, 60) as i, k (k)}
          <li class={i.level}>
            <button onclick={() => goto(i.line)} disabled={!i.line}>{i.file ? `${i.file}:` : ""}{i.line ? t("ligne {n}", { n: i.line }) : ""}</button>
            {i.message}
          </li>
        {/each}
      </ul>
      {#if !result.ok && !errors.length}<details><summary>{t("Journal")}</summary><pre>{result.log}</pre></details>{/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .lx {
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .hidden {
    display: none;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 40px;
    padding: 0 12px;
    border-bottom: 1px solid var(--line);
    font-size: 12.5px;
    color: var(--text-soft);
    flex-shrink: 0;
    overflow-x: auto;
    scrollbar-width: none;
  }
  /* A single line: the file name shortens, the commands never wrap. */
  .bar > :global(*) {
    flex-shrink: 0;
    white-space: nowrap;
  }
  .bar > .name {
    flex-shrink: 1;
    min-width: 40px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .bar > .grow {
    flex-shrink: 1;
  }
  .name {
    font-weight: 600;
    color: var(--text);
  }
  .grow {
    flex: 1;
  }
  .faint {
    color: var(--text-faint);
  }
  .warn {
    color: var(--danger);
  }
  .check {
    display: inline-flex;
    gap: 5px;
    align-items: center;
  }
  .split {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .src {
    flex: 1;
    min-width: 0;
    overflow: auto;
  }
  .ihead {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 26px;
  }
  .fold {
    border: 0;
    background: none;
    color: var(--text-soft);
    cursor: pointer;
    width: 18px;
    padding: 0;
  }
  .issues.closed {
    padding-top: 2px;
    padding-bottom: 2px;
  }
  .src :global(.cm-editor) {
    height: 100%;
    font: 14px/1.55 var(--font-mono);
  }
  .src :global(.cm-content) {
    font-family: var(--font-mono);
  }
  .out {
    flex: 1;
    min-width: 0;
    display: flex;
    border-left: 1px solid var(--line);
  }
  .placeholder {
    padding: 24px;
    color: var(--text-soft);
    font-size: 13.5px;
    max-width: 460px;
  }
  .issues {
    overflow: auto;
    border-top: 1px solid var(--line);
    padding: 8px 14px;
    font-size: 12.5px;
    flex-shrink: 0;
  }
  .ok {
    color: var(--accent-text);
    font-weight: 600;
    margin-right: 10px;
  }
  .ko {
    color: var(--danger);
    font-weight: 600;
    margin-right: 10px;
  }
  ul {
    margin: 6px 0 0;
    padding: 0;
    list-style: none;
  }
  li {
    padding: 2px 0;
  }
  li.error {
    color: var(--danger);
  }
  li.warning {
    color: var(--text-soft);
  }
  li button {
    border: 0;
    background: none;
    padding: 0 6px 0 0;
    color: var(--accent-text);
    font: inherit;
    cursor: pointer;
  }
  .error {
    color: var(--danger);
    padding: 0 14px;
  }
  pre {
    white-space: pre-wrap;
    font-size: 11.5px;
  }
</style>
