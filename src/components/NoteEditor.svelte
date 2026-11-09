<script lang="ts">
  import { t, translateError } from "../lib/i18n.svelte";
  import { onDestroy, onMount } from "svelte";
  import { EditorView } from "@codemirror/view";
  import { EditorState } from "@codemirror/state";
  import { app } from "../lib/state.svelte";
  import { api, assetUrl } from "../lib/api";
  import { editors, type EditorHandle } from "../lib/editor/registry";
  import { EditorConfig } from "../lib/editor/setup";
  import type { Tab } from "../lib/types";
  import { debounce, dirname, stem } from "../lib/util";
  import Icon from "./Icon.svelte";

  let { tab, paneIndex, visible }: { tab: Tab; paneIndex: number; visible: boolean } = $props();

  let host: HTMLDivElement;
  let view: EditorView | null = null;
  let config: EditorConfig;
  let saved = "";
  let deleted = false;
  let saving = false;
  let loadError = $state("");
  let titleValue = $state(stem(tab.path ?? ""));
  let isFiche = $state(false);
  let ready = $state(false);

  // Quick index of note names to colour missing links
  let known = new Set<string>();
  $effect(() => {
    const s = new Set<string>();
    for (const n of app.notes) {
      s.add(n.stem.toLowerCase());
      s.add(n.path.toLowerCase().replace(/\.md$/, ""));
      for (const a of n.aliases) s.add(a.toLowerCase());
    }
    known = s;
  });

  $effect(() => {
    titleValue = stem(tab.path ?? "");
  });

  const folder = $derived(dirname(tab.path ?? ""));

  function exists(target: string): boolean {
    const key = target.toLowerCase().replace(/\.md$/, "").replace(/^\//, "");
    if (known.has(key)) return true;
    if (key.includes("/")) return known.has(key.split("/").pop()!);
    return false;
  }

  const autosave = debounce(() => save(), app.settings?.autosave_ms ?? 800);

  async function save() {
    if (!view || deleted || !tab.path || saving) return;
    const content = view.state.doc.toString();
    if (content === saved) {
      app.updateTab(tab.id, { dirty: false });
      return;
    }
    saving = true;
    try {
      const mtime = await api.saveNote(tab.path, content);
      saved = content;
      app.updateTab(tab.id, { dirty: view.state.doc.toString() !== saved });
      // Reading chain: the status of a reading note (or of the notes cited by a chapter)
      // moves forward on its own. The open note is only reloaded if nothing was typed meanwhile.
      const kind = app.noteByPath(tab.path)?.frontmatter.type;
      if (kind === "fiche" || kind === "chapitre") {
        const p = tab.path;
        api.readingChain(p).then((up) => {
          if (!up.length) return;
          for (const u of up) for (const h of editors.allByPath(u)) api.readNote(u).then((f) => h.externalUpdate(f.content, f.mtime, false));
          app.refreshSoon();
        }).catch(() => {});
      }
      // Same note open in the other pane: it receives the saved text.
      for (const h of editors.allByPath(tab.path)) if (h !== handle) h.externalUpdate(content, mtime);
      app.refreshSoon();
    } catch (e) {
      app.toast(t("Échec de l'enregistrement : {err}", { err: translateError(e) }), "error");
    } finally {
      saving = false;
    }
    if (tab.dirty) autosave();
  }

  function detectFiche(content: string) {
    const fm = /^---\n([\s\S]*?)\n---/.exec(content)?.[1] ?? "";
    isFiche = /^(citekey|zotero):/m.test(fm);
  }

  function replaceDoc(content: string) {
    if (!view) return;
    const sel = view.state.selection.main;
    const pos = Math.min(sel.head, content.length);
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: content },
      selection: { anchor: pos },
      annotations: [],
    });
  }

  const handle: EditorHandle = {
    tabId: tab.id,
    get path() {
      return tab.path ?? "";
    },
    get view() {
      return view!;
    },
    get dirty() {
      return !!tab.dirty;
    },
    saveNow: async () => {
      autosave.cancel();
      await save();
    },
    externalUpdate(content, _mtime, force) {
      if (!view) return;
      const current = view.state.doc.toString();
      if (content === current) {
        saved = content;
        app.updateTab(tab.id, { dirty: false });
        return;
      }
      if (content === saved && !force) return; // our own write
      if (!tab.dirty || force) {
        saved = content;
        replaceDoc(content);
        app.updateTab(tab.id, { dirty: false });
        detectFiche(content);
      } else {
        app.toast(t("« {title} » a été modifiée ailleurs.", { title: tab.title }), "info", {
          label: t("Recharger"),
          run: () => {
            saved = content;
            replaceDoc(content);
            app.updateTab(tab.id, { dirty: false });
          },
        });
      }
    },
    setPath(p) {
      app.updateTab(tab.id, { path: p });
    },
    markDeleted() {
      deleted = true;
      autosave.cancel();
    },
    reconfigure() {
      if (view && app.settings) config.reconfigure(view, app.settings);
    },
    reveal(line, heading) {
      if (!view) return;
      const doc = view.state.doc;
      let pos = 0;
      if (heading) {
        const h = heading.toLowerCase().trim();
        for (let i = 1; i <= doc.lines; i++) {
          const m = /^#{1,6}\s+(.*)$/.exec(doc.line(i).text);
          if (m && m[1].trim().toLowerCase() === h) {
            pos = doc.line(i).from;
            break;
          }
        }
      } else if (line !== undefined) {
        pos = doc.line(Math.min(doc.lines, Math.max(1, line + 1))).from;
      }
      view.dispatch({ selection: { anchor: pos }, effects: EditorView.scrollIntoView(pos, { y: "start", yMargin: 80 }) });
      view.focus();
    },
    insert(text) {
      if (!view) return;
      const r = view.state.selection.main;
      view.dispatch({ changes: { from: r.from, to: r.to, insert: text }, selection: { anchor: r.from + text.length }, scrollIntoView: true });
      view.focus();
    },
    focus() {
      view?.focus();
    },
  };

  async function imageSrc(target: string): Promise<string | null> {
    if (/^(https?:|data:)/.test(target)) return target;
    const decoded = decodeURIComponent(target);
    const rel = await api.resolveLink(decoded, tab.path);
    if (!rel || !app.vault) return null;
    const sep = app.vault.root.includes("\\") ? "\\" : "/";
    return assetUrl(app.vault.root + sep + rel.split("/").join(sep));
  }

  async function onPasteFile(file: File): Promise<string | null> {
    const buf = new Uint8Array(await file.arrayBuffer());
    let bin = "";
    for (let i = 0; i < buf.length; i += 0x8000) bin += String.fromCharCode(...buf.subarray(i, i + 0x8000));
    const ext = (file.type.split("/")[1] || "png").replace("jpeg", "jpg").replace("svg+xml", "svg");
    const stamp = new Date().toISOString().replace(/[-:T]/g, "").slice(0, 14);
    const name = file.name && file.name !== "image.png" ? file.name : `Image ${stamp}.${ext}`;
    try {
      const rel = await api.saveAttachment(app.settings?.attachments_folder ?? "Pièces jointes", name, btoa(bin));
      app.refreshSoon();
      return `![[${rel.split("/").pop()}]]`;
    } catch (e) {
      app.toast(translateError(e), "error");
      return null;
    }
  }

  onMount(async () => {
    config = new EditorConfig({
      onChange: (content) => {
        app.updateTab(tab.id, { dirty: content !== saved });
        if (tab.dirty) autosave();
      },
      onSave: () => handle.saveNow(),
      onCursor: (line, words, chars, selection) => {
        if (editors.activeId === tab.id) {
          app.cursorLine = line;
          app.activeStats = { words, chars, selection };
        }
      },
      onFocus: () => {
        editors.activeId = tab.id;
        app.activePane = paneIndex;
        // The other copy of the note has unsaved keystrokes: they are taken back
        // before writing here, otherwise the two versions diverge.
        for (const h of editors.allByPath(tab.path ?? "")) if (h !== handle && h.dirty) void h.saveNow();
      },
      onPasteFile,
      handlers: {
        notePath: () => tab.path ?? "",
        embed: (target) => api.embed(target),
        openWiki: (target, e) => app.followLink(target, tab.path ?? "", { otherPane: e.altKey, newTab: true }),
        openUrl: (url, e) => {
          // Links from reading notes to the PDF: opened in the built-in reader, at the right page.
          const pdfLink = /^zotero:\/\/open-pdf\/library\/items\/([A-Z0-9]+)(?:\?(.*))?$/i.exec(url);
          // Annotations made in Eloria: vault PDF, at the annotation's page.
          const own = /^eloria-pdf:([^#]+)(?:#page=(\d+))?$/.exec(url);
          if (own) {
            const rel = decodeURIComponent(own[1]);
            app.openPdfFile(app.vaultAbs(rel), stem(rel), { page: own[2] ? Number(own[2]) : undefined });
            return;
          }
          if (pdfLink && !e.shiftKey) {
            const page = Number.parseInt(new URLSearchParams(pdfLink[2] ?? "").get("page") ?? "") || undefined;
            app.openZoteroPdf(pdfLink[1], stem(tab.path ?? ""), page);
          } else if (/^(https?|zotero|mailto):/i.test(url)) api.openUrl(url);
          else app.followLink(decodeURIComponent(url).replace(/\.md$/, ""), tab.path ?? "", { otherPane: e.altKey });
        },
        openTag: (key) => {
          app.searchQuery = `#${key}`;
          app.leftPanel = "search";
          app.leftOpen = true;
        },
        openCitation: async (key, e) => {
          const fiche = app.fiches[key.toLowerCase()];
          if (fiche && !(e.altKey || e.shiftKey)) return app.openNote(fiche, { otherPane: e.ctrlKey || e.metaKey });
          const d = await api.zoteroItemByCitekey(key);
          if (d) app.openView("zotero-item", { key: d.item.key, title: `@${key}`, otherPane: true });
          else app.toast(t("@{key} est absent du miroir Zotero (synchronise d'abord)", { key }), "info");
        },
        imageSrc,
        exists,
      },
    });
    try {
      const file = await api.readNote(tab.path!);
      saved = file.content;
      detectFiche(file.content);
      // Cursor placed after the properties so that they show as a panel.
      const fm = /^---\n[\s\S]*?\n---\n?/.exec(file.content);
      const start = fm ? Math.min(file.content.length, fm[0].length) : 0;
      view = new EditorView({
        parent: host,
        state: EditorState.create({ doc: file.content, selection: { anchor: start }, extensions: config.extensions(app.settings!) }),
      });
      editors.register(handle);
      ready = true;
      if (visible && app.activePane === paneIndex) {
        editors.activeId = tab.id;
        view.focus();
      }
    } catch (e) {
      loadError = translateError(e);
    }
  });

  $effect(() => {
    if (visible && ready && view) {
      view.requestMeasure();
      if (app.activePane === paneIndex) {
        editors.activeId = tab.id;
        const st = view.state;
        const text = st.doc.toString();
        app.cursorLine = st.doc.lineAt(st.selection.main.head).number - 1;
        app.activeStats = {
          words: (text.replace(/^---\n[\s\S]*?\n---\n/, "").match(/[\p{L}\p{N}][\p{L}\p{N}'’-]*/gu) ?? []).length,
          chars: text.length,
          selection: 0,
        };
      }
    }
  });

  onDestroy(() => {
    if (view && tab.dirty && !deleted && tab.path) {
      api.saveNote(tab.path, view.state.doc.toString()).catch(() => {});
    }
    editors.dispose(tab.id);
    view = null;
  });

  async function commitTitle() {
    const v = titleValue.trim();
    if (!tab.path || !v || v === stem(tab.path)) {
      titleValue = stem(tab.path ?? "");
      return;
    }
    try {
      await app.rename(tab.path, v);
    } catch (e) {
      app.toast(translateError(e), "error");
      titleValue = stem(tab.path ?? "");
    }
  }

  function moreMenu(e: MouseEvent) {
    if (!tab.path) return;
    const p = tab.path;
    app.openMenu(e, [
      { label: t("Ouvrir à côté"), icon: "split", run: () => app.openNote(p, { otherPane: true }) },
      { label: t("Historique des versions"), icon: "history", run: () => (app.historyFor = p) },
      { label: t("Comparer avec un autre document…"), icon: "split", run: () => (app.compareFor = p) },
      { label: t("Exporter…"), icon: "download", run: () => (app.exportFor = p) },
      { label: t("Convertir en document LaTeX"), icon: "code", run: () => app.noteToLatex(p) },
      { separator: true, label: "" },
      { label: t("Résumer (IA)…"), icon: "zap", run: () => app.openRewrite("summarize", true) },
      { label: t("Concepts de cette note (IA)…"), icon: "zap", run: () => (app.suggestFor = { path: p, kind: "concepts" }) },
      { label: t("Tags suggérés (IA)…"), icon: "tag", run: () => (app.suggestFor = { path: p, kind: "tags" }) },
      { separator: true, label: "" },
      { label: t("Vérifier le style"), icon: "check", run: () => (app.checkFor = { kind: "style", path: p }) },
      { label: t("Similarité avec la bibliothèque"), icon: "search", run: () => (app.checkFor = { kind: "similarite", path: p }) },
      { label: t("Insérer une formule ou un tableau…"), icon: "table", run: () => (app.latexToolsFor = { path: p, tab: "formules" }) },
      { label: t("Assistant d'écriture scientifique (IA)"), icon: "zap", run: () => app.openScience() },
      { label: t("Cohérence des citations (IA)"), icon: "at", run: () => app.openScience("citations") },
      ...(app.noteByPath(p)?.frontmatter.type === "argument" ? [{ label: t("Carte de l'argumentation"), icon: "graph", run: () => app.openView("argument", { path: p, title: app.noteByPath(p)?.title }) }] : []),
      { separator: true, label: "" },
      { label: t("Revoir demain"), icon: "calendar", run: () => app.scheduleReview(p, 1) },
      { label: t("Revoir dans une semaine"), icon: "calendar", run: () => app.scheduleReview(p, 7) },
      { label: t("Revoir dans un mois"), icon: "calendar", run: () => app.scheduleReview(p, 30) },
      ...(app.noteByPath(p)?.frontmatter.revoir_le ? [{ label: t("Retirer la révision"), icon: "x", run: () => app.scheduleReview(p, 0) }] : []),
      { label: t("Vérifier les citations"), icon: "at", run: () => (app.checkFor = { kind: "citations", path: p }) },
      { separator: true, label: "" },
      { label: t("Afficher dans l'explorateur"), icon: "external", run: () => api.revealPath(p) },
      { label: t("Copier le lien [[…]]"), icon: "link", run: () => navigator.clipboard.writeText(`[[${stem(p)}]]`) },
      { separator: true, label: "" },
      { label: t("Supprimer"), icon: "trash", danger: true, run: () => app.remove(p) },
    ]);
  }
</script>

<div class="note" class:focus={app.focusMode}>
  <div class="bar">
    <div class="crumbs">
      {#if folder}
        {#each folder.split("/") as part, i (i)}
          <span class="crumb">{part}</span><span class="sep">/</span>
        {/each}
      {/if}
      <input
        class="note-title-input"
        bind:value={titleValue}
        spellcheck="false"
        aria-label={t("Nom de la note")}
        onkeydown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            (e.target as HTMLInputElement).blur();
            view?.focus();
          }
          if (e.key === "Escape") {
            titleValue = stem(tab.path ?? "");
            view?.focus();
          }
        }}
        onblur={commitTitle}
        style:width="{Math.max(6, titleValue.length + 1)}ch"
      />
    </div>
    <div class="actions">
      {#if app.noteByPath(tab.path)?.frontmatter.type === "revue"}
        <button class="btn small primary" onclick={() => app.openView("review", { path: tab.path, title: app.noteByPath(tab.path)?.title })}>
          <Icon name="list" size={14} /> {t("Ouvrir la revue")}
        </button>
      {/if}
      {#if isFiche}
        {@const fm = app.noteByPath(tab.path)?.frontmatter}
        {@const zkey = fm?.zotero}
        {#if fm?.type === "fiche" && typeof zkey === "string"}
          <!-- Reading note: the "Eloria method" outline is filled by hand or by the AI. -->
          <button class="btn small primary" title={t("Rédige la fiche complète selon la méthode Eloria (IA)")} onclick={() => (app.aiFicheFor = { key: zkey, title: app.noteByPath(tab.path)?.title ?? "", path: tab.path })}>
            <Icon name="zap" size={14} /> {t("Méthode Eloria")}
          </button>
        {:else if fm?.type === "source"}
          <button class="btn small ghost" title={t("Régénère le bloc d'annotations depuis Zotero")} onclick={() => tab.path && app.syncFiche(tab.path)}>
            <Icon name="sync" size={14} /> {t("Annotations")}
          </button>
        {/if}
      {/if}
      <button
        class="icon-btn"
        class:active={app.settings?.live_preview}
        title={app.settings?.live_preview ? t("Aperçu en direct (Ctrl+Maj+P pour le Markdown brut)") : t("Markdown brut (Ctrl+Maj+P pour l'aperçu)")}
        aria-label={t("Basculer l'aperçu")}
        onclick={() => app.updateSettings({ live_preview: !app.settings?.live_preview })}
      >
        <Icon name={app.settings?.live_preview ? "eye" : "code"} />
      </button>
      <button class="icon-btn" title={t("Mode concentration (F11)")} aria-label={t("Mode concentration")} onclick={() => (app.focusMode = !app.focusMode)}><Icon name="focus" /></button>
      <button class="icon-btn" title={t("Plus d'actions")} aria-label={t("Plus d'actions")} onclick={moreMenu}><Icon name="more" /></button>
    </div>
  </div>
  {#if loadError}
    <p class="empty">{t("Impossible d'ouvrir cette note : {err}", { err: loadError })}</p>
  {/if}
  <div class="host" bind:this={host}></div>
</div>

<style>
  .note {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 38px;
    padding: 0 10px 0 16px;
    flex-shrink: 0;
  }
  .note.focus .bar {
    opacity: 0;
    transition: opacity 0.2s;
  }
  .note.focus .bar:hover {
    opacity: 1;
  }
  .crumbs {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    color: var(--text-faint);
    font-size: 12.5px;
    overflow: hidden;
  }
  .crumb {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 1ch;
    flex: 0 1 auto;
  }
  .sep {
    flex-shrink: 0;
  }
  .sep {
    opacity: 0.6;
  }
  .note-title-input {
    border: 0;
    background: transparent;
    color: var(--text);
    font-weight: 600;
    font-size: 12.5px;
    padding: 3px 5px;
    border-radius: 4px;
    min-width: 6ch;
    max-width: 100%;
    flex: 0 0 auto;
    outline: none;
    user-select: text;
  }
  .note-title-input:hover {
    background: var(--bg-sunk);
  }
  .note-title-input:focus {
    background: var(--bg-raised);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
  }
  .host {
    flex: 1;
    min-height: 0;
  }
</style>
