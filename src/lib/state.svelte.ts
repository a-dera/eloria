// Global state of the interface (Svelte 5, runes).

import { api, assetUrl, isTauri, onCaptured, onVaultChanged, onZoteroChanged, pickFile, pickFolder, setWindowTitle } from "./api";
import type { CustomTheme, ItemSummary, MirrorStatus, NoteMeta, Pane, Settings, Tab, TabKind, TreeNode, VaultInfo, Work } from "./types";
import { applyTheme, CUSTOM_PREFIX } from "./themes";
import { debounce, dirname, joinPath, stem, uid } from "./util";
import { editors } from "./editor/registry";
import { i18n, locale, t, tn, tk, translateError } from "./i18n.svelte";

export type LeftPanel = "files" | "search" | "tags" | "zotero" | "these" | "inbox";

export type RightPanel = "backlinks" | "outline" | "graph" | "properties" | "ask";

export interface Toast {
  id: string;
  text: string;
  kind: "info" | "success" | "error";
  action?: { label: string; run: () => void };
}

interface PromptState {
  title: string;
  value: string;
  placeholder?: string;
  resolve: (v: string | null) => void;
}

interface ConfirmState {
  title: string;
  message: string;
  danger?: boolean;
  confirmLabel?: string;
  resolve: (v: boolean) => void;
}

export interface MenuItem {
  label: string;
  icon?: string;
  danger?: boolean;
  disabled?: boolean;
  run?: () => void;
  separator?: boolean;
}


class AppState {
  vault = $state<VaultInfo | null>(null);
  settings = $state<Settings | null>(null);
  tree = $state<TreeNode | null>(null);
  notes = $state<NoteMeta[]>([]);
  tags = $state<[string, number][]>([]);

  panes = $state<Pane[]>([{ id: "p0", tabs: [], active: null }]);
  activePane = $state(0);

  leftPanel = $state<LeftPanel>("files");
  leftOpen = $state(true);
  rightOpen = $state(true);
  rightPanel = $state<RightPanel>("backlinks");
  leftWidth = $state(280);
  /** Window width (updated on resize). */
  winW = $state(typeof window !== "undefined" ? window.innerWidth : 1400);
  /** Left panel shown: never to the point of leaving less than 480 px to the note. */
  get leftShown() {
    return Math.max(200, Math.min(this.leftWidth, this.winW - 46 - 480));
  }
  /** Room for the right panel without squashing the note. */
  get roomForRight() {
    return this.winW - 46 - (this.leftOpen ? this.leftShown : 0) - this.rightWidth >= 480;
  }
  /** Shows or hides the right panel; when there is no room, the left one folds. */
  toggleRight() {
    this.rightOpen = !this.rightOpen;
    if (this.rightOpen && !this.roomForRight) this.leftOpen = false;
    this.saveLayout();
  }
  /** Share of the first pane when the space is split in two (0.2 to 0.8). */
  splitRatio = $state(0.5);
  rightWidth = $state(300);
  focusMode = $state(false);

  searchQuery = $state("");
  expanded = $state<Record<string, boolean>>({});

  zoteroStatus = $state<MirrorStatus | null>(null);
  zoteroItems = $state<ItemSummary[]>([]);
  fiches = $state<Record<string, string>>({});
  /** Custom themes of the vault (.eloria/themes/*.css) */
  customThemes = $state<CustomTheme[]>([]);

  palette = $state<null | "commands" | "files" | "templates">(null);
  /** Text pre-filled when opening the palette (e.g. "IA" for the Intelligence space). */
  paletteSeed = $state("");
  settingsOpen = $state(false);
  /** Section opened by default in the settings. */
  settingsSection = $state("general");

  openPalette(kind: "commands" | "files" | "templates", seed = "") {
    this.paletteSeed = seed;
    this.palette = kind;
  }

  openSettings(section = "general") {
    this.settingsSection = section;
    this.settingsOpen = true;
  }


  historyFor = $state<string | null>(null);
  exportFor = $state<string | null>(null);
  /** Window importing the Zotero library into notes */
  importOpen = $state(false);
  /** Fingerprint of the Rust binary (commit, build date), shown in "About". */
  buildInfo = $state<{ commit: string; built: string; commands: string[] } | null>(null);
  /** Writing assistant: chosen text and how to apply it to the editor. */
  rewriteFor = $state<{ text: string; task: import("./types").RewriteTask; apply: (text: string, insertAfter: boolean) => void } | null>(null);
  /** Focus session in progress: planned end (ms) and duration (min). */
  focus = $state<{ end: number; minutes: number; start: number } | null>(null);
  private focusTimer: ReturnType<typeof setTimeout> | null = null;

  startFocus() {
    const minutes = this.settings?.focus_minutes || 25;
    const start = Date.now();
    this.focus = { start, end: start + minutes * 60_000, minutes };
    if (this.focusTimer) clearTimeout(this.focusTimer);
    this.focusTimer = setTimeout(() => this.stopFocus(true), minutes * 60_000);
  }

  /** End (or stop) of the session: the minutes spent are counted. */
  async stopFocus(done = false) {
    if (!this.focus) return;
    const minutes = Math.round((Math.min(Date.now(), this.focus.end) - this.focus.start) / 60_000);
    this.focus = null;
    if (this.focusTimer) clearTimeout(this.focusTimer);
    if (minutes >= 1) {
      const total = await api.addFocus(minutes).catch(() => 0);
      this.toast(done ? t("Session terminée : {m} min. Aujourd'hui : {t} min de concentration.", { m: minutes, t: total }) : t("Session arrêtée : {m} min comptées.", { m: minutes }), "success");
      this.revision++;
    }
  }

  /** Schedules the review of a note (0: removes it). */
  async scheduleReview(path: string, days: number) {
    try {
      const d = await api.scheduleReview(path, days);
      await this.reloadOpen([path]);
      await this.refresh();
      this.toast(d ? t("À revoir le {date}", { date: new Date(`${d}T12:00:00`).toLocaleDateString(locale()) }) : t("Révision retirée"), "success");
    } catch (e) {
      this.toast(translateError(e), "error");
    }
  }

  async weeklyReview() {
    try {
      const p = await api.weeklyReview();
      await this.refresh();
      await this.openNote(p, { newTab: true });
    } catch (e) {
      this.toast(translateError(e), "error");
    }
  }

  /** Captures the copied text into the inbox. */
  async captureClipboard() {
    let text = "";
    try {
      text = await navigator.clipboard.readText();
    } catch {
      /* reading refused: ask */
    }
    if (!text.trim()) text = (await this.ask(t("Texte à capturer"), "")) ?? "";
    if (!text.trim()) return;
    await this.captureText(text);
  }

  async captureIdea() {
    const text = await this.ask(t("Une idée, une citation, un lien…"), "");
    if (text?.trim()) await this.captureText(text);
  }

  private async captureText(text: string) {
    try {
      const p = await api.capture(text);
      await this.refresh();
      this.leftPanel = "inbox";
      this.leftOpen = true;
      this.toast(t("Capturé dans la boîte de réception"), "success", { label: t("Ouvrir"), run: () => this.openNote(p, { newTab: true }) });
    } catch (e) {
      this.toast(translateError(e), "error");
    }
  }

  /** Imports a Markdown folder from Obsidian, Notion, Logseq… */
  async importMarkdownDir() {
    const dir = await pickFolder(t("Dossier Markdown à importer (Obsidian, Notion, Logseq…)"));
    if (!dir) return;
    try {
      const r = await api.importMarkdown(dir);
      await this.refresh();
      this.toast(t("{n} notes et {f} fichiers importés dans « {folder} » ({l} liens convertis)", { n: r.notes, f: r.files, folder: r.folder, l: r.links_rewritten }), "success");
    } catch (e) {
      this.toast(translateError(e), "error");
    }
  }

  /** Saves a Git version of the vault (the repository is created on first use). */
  async gitSave(message = "") {
    try {
      let st = await api.gitStatus();
      if (!st.available) {
        this.toast(t("Git n'est pas installé : winget install Git.Git"), "info");
        return;
      }
      if (!st.repo) st = await api.gitInit();
      await this.saveAll();
      st = await api.gitCommit(message);
      this.toast(st.log[0] ? t("Version enregistrée : {msg}", { msg: st.log[0][2] }) : t("Rien de nouveau à enregistrer"), "success");
      return st;
    } catch (e) {
      this.toast(translateError(e), "error");
    }
  }

  /** Document comparison: the first document (the second one is chosen). */
  compareFor = $state<string | null>(null);
  /** Local checks: style, citations, bibliography, orphan notes and duplicates. */
  /** Scientific writing assistant: target document and starting text (selection). */
  scienceFor = $state<{ path: string; text: string; tab?: "generer" | "formule" | "ameliorer" | "citations" } | null>(null);
  /** LaTeX tools (formulas, tables, TikZ, cross-references): target document. */
  latexToolsFor = $state<{ path: string; tab: "formules" | "tableau" | "tikz" | "renvois" } | null>(null);
  /** Active LaTeX editor (the assistant inserts its answer there). */
  latexEditor: import("@codemirror/view").EditorView | null = null;

  /** Opens the scientific assistant on the active document, with the current selection. */
  openScience(tab?: "generer" | "formule" | "ameliorer" | "citations") {
    const tb = this.activeTab;
    const path = tb?.path ?? "";
    const view = tb?.kind === "latex" ? this.latexEditor : editors.activeView();
    const sel = view ? view.state.sliceDoc(view.state.selection.main.from, view.state.selection.main.to) : "";
    this.scienceFor = { path, text: sel, tab };
  }

  /** Inserts text into the active editor (note or LaTeX), replacing the selection. */
  insertInEditor(text: string): boolean {
    const view = this.activeTab?.kind === "latex" ? this.latexEditor : editors.activeView();
    if (!view) return false;
    const { from, to } = view.state.selection.main;
    view.dispatch({ changes: { from, to, insert: text }, selection: { anchor: from + text.length } });
    view.focus();
    return true;
  }

  /** Outline generator: scope suggested on opening. */
  outlineFor = $state<{ scope: "chapter" | "tag" | "folder"; value: string } | null>(null);

  /** "New document" flow: a Zotero item (key) or a vault source (path). */
  flowFor = $state<{ key?: string; path?: string; title: string } | null>(null);

  checkFor = $state<{ kind: "style" | "citations" | "bib" | "notes" | "checklist" | "fiabilite" | "similarite"; path?: string } | null>(null);

  /** Converts the note into a LaTeX document and opens it (compiled right away). */
  async noteToLatex(path: string) {
    try {
      const tex = await api.noteToLatex(path);
      await this.refresh();
      this.openView("latex", { path: tex, title: tex.split("/").pop() });
    } catch (e) {
      this.toast(translateError(e), "error");
    }
  }

  /** Assembles the chapters into a LaTeX manuscript and opens main.tex. */
  async assembleThesis() {
    try {
      const main = await api.assembleThesis();
      await this.refresh();
      this.openView("latex", { path: main, title: "main.tex" });
      this.toast(t("Manuscrit assemblé à partir des chapitres"), "success");
    } catch (e) {
      this.toast(translateError(e), "error");
    }
  }
  /** AI suggestions (concepts or tags) for a note, to approve. */
  suggestFor = $state<{ path: string; kind: "concepts" | "tags" } | null>(null);
  /** Question to send to Ask Eloria (right panel), from the palette for example. */
  askSeed = $state("");

  /** Opens (or closes) the Ask Eloria conversation in the right panel. */
  toggleAsk(force?: boolean) {
    const open = force ?? !(this.rightOpen && this.rightPanel === "ask");
    if (open) {
      this.rightPanel = "ask";
      this.rightOpen = true;
      if (!this.roomForRight) this.leftOpen = false;
    } else this.rightOpen = false;
    this.saveLayout();
  }

  /** Opens the assistant on the active editor's selection, or on the whole note. */
  openRewrite(task: import("./types").RewriteTask, whole = false) {
    const view = editors.activeView();
    if (!view) return this.toast(t("Ouvre une note d'abord"), "info");
    const doc = view.state.doc.toString();
    let { from, to } = view.state.selection.main;
    if (whole || from === to) {
      // The whole note, without the properties.
      const fm = /^---\n[\s\S]*?\n---\n?/.exec(doc);
      from = fm ? fm[0].length : 0;
      to = doc.length;
    }
    const text = doc.slice(from, to);
    if (!text.trim()) return this.toast(t("Rien à traiter : sélectionne un passage"), "info");
    this.rewriteFor = {
      text,
      task,
      apply: (out, insertAfter) => {
        const cur = view.state.doc.sliceString(from, to);
        // The text changed meanwhile: the passage is found again, otherwise inserted at the cursor.
        let f = from;
        let tt = to;
        if (cur !== text) {
          const i = view.state.doc.toString().indexOf(text);
          if (i < 0) {
            f = tt = view.state.selection.main.head;
            insertAfter = true;
          } else {
            f = i;
            tt = i + text.length;
          }
        }
        if (insertAfter) {
          // Summary of the whole note: after the first heading; of a selection: right after it.
          let at = tt;
          if (whole) {
            const body = view.state.doc.sliceString(f, tt);
            const h = /^# .*\n/m.exec(body);
            at = h ? f + h.index + h[0].length : f;
          }
          view.dispatch({ changes: { from: at, insert: `\n${out}\n` } });
        } else view.dispatch({ changes: { from: f, to: tt, insert: out } });
        view.focus();
      },
    };
  }

  /** "Check the vault" window. */
  integrityOpen = $state(false);
  private backupTimer: ReturnType<typeof setInterval> | null = null;

  async backupNow() {
    if (!this.settings?.backup_dir) {
      this.openSettings("fichiers");
      return this.toast(t("Choisis d'abord un dossier de sauvegarde."), "info");
    }
    const waiting = this.toast(t("Sauvegarde du coffre…"), "info");
    try {
      const b = await api.backupNow(this.settings?.backup_dir);
      this.dismiss(waiting);
      this.toast(t("Coffre sauvegardé ({size})", { size: `${(b.bytes / 1_048_576).toFixed(1)} Mo` }), "success");
      return b;
    } catch (e) {
      this.dismiss(waiting);
      this.toast(translateError(e), "error");
    }
  }

  /** Automatic backup: checked on opening, then every 30 minutes. */
  startAutoBackup() {
    if (this.backupTimer) clearInterval(this.backupTimer);
    const tick = async () => {
      const s = this.settings;
      if (!s?.backup_dir || !s.backup_interval_hours || !this.vault) return;
      try {
        const list = await api.backupList();
        const last = list[0] ? new Date(list[0].created.replace(" ", "T")).getTime() : 0;
        if (Date.now() - last >= s.backup_interval_hours * 3600_000) {
          const b = await api.backupNow(this.settings?.backup_dir);
          this.toast(t("Coffre sauvegardé ({size})", { size: `${(b.bytes / 1_048_576).toFixed(1)} Mo` }), "info");
        }
      } catch (e) {
        this.toast(t("Sauvegarde impossible : {err}", { err: translateError(e) }), "error");
      }
    };
    tick();
    this.backupTimer = setInterval(tick, 30 * 60_000);
  }

  /** "Add a reference" window (DOI, Crossref). */
  addRefOpen = $state(false);
  /** Collection to import (null = the whole library) */
  importCollection = $state<{ key: string; name: string } | null>(null);
  /** "Eloria method" AI analysis being generated or reviewed */
  aiFicheFor = $state<{ key: string; title: string; path?: string } | null>(null);
  prompt = $state<PromptState | null>(null);
  confirmBox = $state<ConfirmState | null>(null);
  menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  toasts = $state<Toast[]>([]);
  /** Incremented on every index change (panels refresh). */
  revision = $state(0);
  /** Cursor position in the active note (line), for the outline. */
  cursorLine = $state(0);
  /** Statistics of the active note */
  activeStats = $state({ words: 0, chars: 0, selection: 0 });

  private unlisten: (() => void) | null = null;

  // ------------------------------------------------------------ derived

  get pane(): Pane {
    return this.panes[Math.min(this.activePane, this.panes.length - 1)];
  }

  get activeTab(): Tab | null {
    const p = this.pane;
    return p.tabs.find((tb) => tb.id === p.active) ?? null;
  }

  get activeNotePath(): string | null {
    const tb = this.activeTab;
    return tb?.kind === "note" ? (tb.path ?? null) : null;
  }

  /**
   * Note the right panel relates to: the one of the active pane, otherwise the one
   * shown in the other pane. This way, switching panes neither shows
   * nor hides the right panel (which used to shift everything during the click).
   */
  get contextNotePath(): string | null {
    if (this.activeNotePath) return this.activeNotePath;
    for (const p of this.panes) {
      const tb = p.tabs.find((x) => x.id === p.active);
      if (tb?.kind === "note" && tb.path) return tb.path;
    }
    return null;
  }

  noteByPath(path: string | null | undefined): NoteMeta | undefined {
    if (!path) return undefined;
    return this.notes.find((n) => n.path === path);
  }

  // -------------------------------------------------------- startup

  async init() {
    this.settings = await api.getSettings();
    applySettings(this.settings);
    window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => this.settings && applySettings(this.settings));
    try {
      const saved = JSON.parse(localStorage.getItem("eloria:layout") ?? "{}");
      if (saved.leftWidth) this.leftWidth = saved.leftWidth;
      if (saved.rightWidth) this.rightWidth = saved.rightWidth;
      if (saved.splitRatio) this.splitRatio = saved.splitRatio;
      if (typeof saved.rightOpen === "boolean") this.rightOpen = saved.rightOpen;
      if (saved.expanded) this.expanded = saved.expanded;
    } catch {}
    const info = await api.vaultInfo();
    if (info) await this.afterOpen(info);
    else if (this.settings.last_vault) await this.openVault(this.settings.last_vault).catch(() => {});
  }

  saveLayout = debounce(() => {
    try {
      localStorage.setItem(
        "eloria:layout",
        JSON.stringify({ leftWidth: this.leftWidth, rightWidth: this.rightWidth, splitRatio: this.splitRatio, rightOpen: this.rightOpen, expanded: this.expanded }),
      );
    } catch {}
  }, 400);

  async openVault(path: string) {
    const info = await api.openVault(path);
    await this.afterOpen(info);
    if (info.created) this.toast(t("Nouveau coffre prêt : dossiers, modèles et note d'accueil créés."), "success");
  }

  private async afterOpen(info: VaultInfo) {
    this.vault = info;
    this.settings = await api.getSettings();
    setWindowTitle(`${info.name} · Eloria`);
    this.panes = [{ id: "p0", tabs: [], active: null }];
    this.activePane = 0;
    await this.refresh();
    await this.loadCustomThemes();
    this.unlisten?.();
    this.unlisten = await onVaultChanged((e) => this.onExternalChange(e.paths, e.structural));
    this.unlistenZotero?.();
    this.unlistenZotero = await onZoteroChanged(() => this.zoteroAutoSync());
    await onCaptured((p) => {
      this.refresh();
      this.toast(t("Page reçue du navigateur"), "success", { label: t("Ouvrir"), run: () => this.openNote(p, { newTab: true }) });
    });
    this.loadZotero();
    this.startAutoBackup();
    try {
      const last = JSON.parse(localStorage.getItem(`eloria:tabs:${info.root}`) ?? "[]") as string[];
      for (const p of last.filter((p) => this.noteByPath(p))) await this.openNote(p, { newTab: true, background: true });
      if (this.pane.tabs.length) this.pane.active = this.pane.tabs[this.pane.tabs.length - 1].id;
    } catch {}
    if (!this.pane.tabs.length) {
      const welcome = this.notes.find((n) => n.stem.toLowerCase() === "bienvenue") ?? this.notes[0];
      if (welcome) await this.openNote(welcome.path);
    }
  }

  async closeVault() {
    await this.saveAll();
    await api.closeVault();
    this.unlisten?.();
    this.vault = null;
    this.tree = null;
    this.notes = [];
    this.panes = [{ id: "p0", tabs: [], active: null }];
    setWindowTitle("Eloria");
  }

  rememberTabs = debounce(() => {
    if (!this.vault) return;
    const paths = this.panes.flatMap((p) => p.tabs.filter((tb) => tb.kind === "note" && tb.path).map((tb) => tb.path!));
    try {
      localStorage.setItem(`eloria:tabs:${this.vault.root}`, JSON.stringify([...new Set(paths)].slice(-12)));
    } catch {}
  }, 500);

  refresh = async () => {
    if (!this.vault) return;
    const [tree, notes, tags] = await Promise.all([api.tree(), api.allNotes(), api.tags()]);
    this.tree = tree;
    this.notes = notes;
    this.tags = tags;
    this.revision++;
  };

  refreshSoon = debounce(() => this.refresh(), 250);

  async loadCustomThemes() {
    try {
      this.customThemes = await api.customThemes();
    } catch {
      this.customThemes = [];
    }
    if (this.settings) applySettings(this.settings);
  }

  async loadZotero() {
    try {
      this.zoteroStatus = await api.zoteroStatus();
      if (this.zoteroStatus.items > 0) {
        this.zoteroItems = await api.zoteroItems(null, null, 20000);
      }
      this.fiches = await api.fichesMap();
    } catch {}
  }

  private async onExternalChange(paths: string[], structural: boolean) {
    this.refreshSoon();
    for (const p of paths) {
      const eds = editors.allByPath(p);
      if (!eds.length) continue;
      try {
        const file = await api.readNote(p);
        for (const ed of eds) ed.externalUpdate(file.content, file.mtime);
      } catch {
        // Deleted elsewhere
      }
    }
    void structural;
  }

  // ------------------------------------------------------------ onglets

  /** Changes a tab through the global state (a component does not modify the tab it received as a prop). */
  updateTab(id: string, patch: Partial<Tab>) {
    const found = this.findTab((tb) => tb.id === id);
    if (found) Object.assign(found[1], patch);
  }

  private findTab(pred: (tb: Tab) => boolean): [number, Tab] | null {
    for (let i = 0; i < this.panes.length; i++) {
      const tb = this.panes[i].tabs.find(pred);
      if (tb) return [i, tb];
    }
    return null;
  }

  private addTab(tab: Tab, opts: { newTab?: boolean; otherPane?: boolean; background?: boolean } = {}) {
    if (opts.otherPane) {
      if (this.panes.length < 2) this.panes.push({ id: uid("p"), tabs: [], active: null });
      this.activePane = this.activePane === 0 ? 1 : 0;
    }
    const pane = this.pane;
    const current = pane.tabs.find((tb) => tb.id === pane.active);
    // Like a browser: without an explicit request, the current tab is replaced if it has no changes.
    if (!opts.newTab && current && !current.dirty && !opts.otherPane && current.kind === tab.kind && tab.kind === "note") {
      const i = pane.tabs.indexOf(current);
      editors.dispose(current.id);
      pane.tabs[i] = tab;
    } else {
      const i = current ? pane.tabs.indexOf(current) + 1 : pane.tabs.length;
      pane.tabs.splice(i, 0, tab);
    }
    if (!opts.background) pane.active = tab.id;
    this.rememberTabs();
  }

  activate(paneIndex: number, tabId: string) {
    this.activePane = paneIndex;
    this.panes[paneIndex].active = tabId;
  }

  async openNote(
    path: string,
    opts: { newTab?: boolean; otherPane?: boolean; line?: number; heading?: string; background?: boolean } = {},
  ) {
    if (/\.(png|jpe?g|gif|svg|webp)$/i.test(path)) return this.openView("image", { path, title: stem(path) });
    if (/\.pdf$/i.test(path)) return this.openPdfFile(this.vaultAbs(path), stem(path), { newTab: true });
    if (/\.(tex|bib|sty|cls)$/i.test(path)) return this.openView("latex", { path, title: path.split("/").pop() });
    const existing = this.findTab((tb) => tb.kind === "note" && tb.path === path);
    if (existing && !opts.otherPane) {
      if (!opts.background) this.activate(existing[0], existing[1].id);
    } else {
      const meta = this.noteByPath(path);
      // Each open file has its tab (an already open tab is simply reactivated).
      this.addTab({ id: uid(), kind: "note", path, title: meta?.title ?? stem(path) }, { ...opts, newTab: opts.newTab ?? true });
    }
    if (opts.line !== undefined || opts.heading) {
      // The editor may not be mounted right away.
      setTimeout(() => editors.active()?.reveal(opts.line, opts.heading), 60);
    }
  }

  /** Absolute path of a vault file. */
  vaultAbs(rel: string): string {
    const root = this.vault?.root ?? "";
    const sep = root.includes("\\") ? "\\" : "/";
    return root + sep + rel.split("/").join(sep);
  }

  /** Opens a PDF in the built-in reader (by default in the other pane). */
  openPdfFile(abs: string, title: string, opts: { page?: number; zoteroKey?: string; newTab?: boolean } = {}) {
    const existing = this.findTab((tb) => tb.kind === "pdf" && tb.path === abs);
    if (existing) {
      existing[1].page = opts.page;
      return this.activate(existing[0], existing[1].id);
    }
    this.addTab(
      { id: uid(), kind: "pdf", title, path: abs, key: opts.zoteroKey, page: opts.page },
      opts.newTab ? { newTab: true } : { newTab: true, otherPane: true },
    );
  }

  /** PDF of a Zotero item or attachment, next to the note, at the right page. */
  async openZoteroPdf(key: string, title: string, page?: number) {
    try {
      const p = await api.zoteroPdfPath(key);
      if (!p) return this.toast(t("PDF introuvable sur le disque : il n'est peut-être pas téléchargé dans Zotero."), "info");
      this.openPdfFile(p, title, { page, zoteroKey: key });
    } catch (e) {
      this.toast(translateError(e), "error");
    }
  }

  openView(kind: TabKind, opts: { title?: string; path?: string; key?: string; otherPane?: boolean } = {}) {
    const existing = this.findTab((tb) => tb.kind === kind && tb.key === opts.key && tb.path === opts.path);
    if (existing) return this.activate(existing[0], existing[1].id);
    const titles: Record<string, string> = {
      graph: tk("Graphe"),
      kanban: tk("Tableau Kanban"),
      table: tk("Base de notes"),
      timeline: tk("Frise des lectures"),
      matrix: tk("Matrice de recherche"),
      dashboard: tk("Tableau de bord"),
      these: tk("Espace thèse"),
      methodes: tk("Méthodologies"),
      argument: tk("Argumentation"),
      statistiques: tk("Statistiques de recherche"),
      concepts: tk("Concepts"),
      themes: tk("Thèmes"),
      entites: tk("Entités"),
      taxonomie: tk("Taxonomie"),
      atlas: tk("Carte des connaissances"),
      aide: tk("Aide"),
      console: tk("Console développeur"),
      echanges: tk("Import et export"),
      citations: tk("Réseau de citations"),
      review: tk("Revue de littérature"),
    };
    this.addTab(
      { id: uid(), kind, title: opts.title ?? (titles[kind] ? t(titles[kind]) : kind), path: opts.path, key: opts.key },
      { newTab: true, otherPane: opts.otherPane },
    );
  }

  async closeTab(paneIndex: number, tabId: string) {
    const pane = this.panes[paneIndex];
    const i = pane.tabs.findIndex((tb) => tb.id === tabId);
    if (i < 0) return;
    await editors.get(tabId)?.saveNow();
    editors.dispose(tabId);
    pane.tabs.splice(i, 1);
    if (pane.active === tabId) pane.active = pane.tabs[Math.min(i, pane.tabs.length - 1)]?.id ?? null;
    if (!pane.tabs.length && this.panes.length > 1) {
      this.panes.splice(paneIndex, 1);
      this.activePane = 0;
    }
    this.rememberTabs();
  }

  closeActiveTab() {
    const tb = this.activeTab;
    if (tb) this.closeTab(this.activePane, tb.id);
  }

  cycleTab(dir: 1 | -1) {
    const p = this.pane;
    if (!p.tabs.length) return;
    const i = p.tabs.findIndex((tb) => tb.id === p.active);
    p.active = p.tabs[(i + dir + p.tabs.length) % p.tabs.length].id;
  }

  toggleSplit() {
    if (this.panes.length > 1) {
      const [a, b] = this.panes;
      a.tabs.push(...b.tabs.filter((tb) => !a.tabs.some((x) => x.kind === "note" && x.path === tb.path && tb.kind === "note")));
      this.panes = [a];
      this.activePane = 0;
    } else {
      const tb = this.activeTab;
      this.panes.push({ id: uid("p"), tabs: [], active: null });
      if (tb?.kind === "note" && tb.path) {
        const nt = { ...tb, id: uid() };
        this.panes[1].tabs.push(nt);
        this.panes[1].active = nt.id;
      }
      this.activePane = 1;
    }
  }

  moveTab(fromPane: number, tabId: string, toPane: number, index: number) {
    const src = this.panes[fromPane];
    const i = src.tabs.findIndex((tb) => tb.id === tabId);
    if (i < 0) return;
    const [tab] = src.tabs.splice(i, 1);
    const dst = this.panes[toPane];
    dst.tabs.splice(Math.min(index, dst.tabs.length), 0, tab);
    dst.active = tab.id;
    if (src.active === tabId) src.active = src.tabs[Math.max(0, i - 1)]?.id ?? null;
    this.activePane = toPane;
  }

  /** Updates the title/path of tabs after a rename. */
  private retab(oldPath: string, newPath: string) {
    for (const p of this.panes) {
      for (const tb of p.tabs) {
        if (tb.path === oldPath) {
          tb.path = newPath;
          tb.title = stem(newPath);
          editors.get(tb.id)?.setPath(newPath);
        } else if (tb.path?.startsWith(oldPath + "/")) {
          const np = newPath + tb.path.slice(oldPath.length);
          tb.path = np;
          editors.get(tb.id)?.setPath(np);
        }
      }
    }
  }

  async saveAll() {
    await Promise.all(editors.all().map((e) => e.saveNow()));
  }

  // ----------------------------------------------------------- fichiers

  async newNote(folder?: string, name = "Sans titre", content?: string, opts: { otherPane?: boolean } = {}) {
    const f = folder ?? (this.activeNotePath ? dirname(this.activeNotePath) : "");
    const path = await api.createNote(f, name, content ?? "");
    await this.refresh();
    await this.openNote(path, { newTab: true, ...opts });
    if (name === "Sans titre") setTimeout(() => document.querySelector<HTMLInputElement>(".note-title-input")?.select(), 80);
    return path;
  }

  async newNoteFromTemplate(templatePath: string, folder?: string) {
    const title = await this.ask(t("Titre de la nouvelle note"), "");
    if (!title) return;
    const content = await api.renderTemplate(templatePath, title);
    return this.newNote(folder, title, content);
  }

  async newFolder(parent = "") {
    const name = await this.ask(t("Nom du dossier"), t("Nouveau dossier"));
    if (!name) return;
    const path = joinPath(parent, name);
    await api.createFolder(path);
    this.expanded[parent] = true;
    await this.refresh();
  }

  async rename(path: string, newName: string) {
    const isNote = path.endsWith(".md");
    const clean = newName.trim().replace(/[\\/:*?"<>|]/g, "-");
    if (!clean) return;
    const dir = dirname(path);
    const to = joinPath(dir, isNote ? `${clean}.md` : clean);
    if (to === path) return;
    await this.saveAll();
    const r = await api.rename(path, to, true);
    this.retab(path, to);
    await this.reloadOpen(r.updated);
    await this.refresh();
    if (r.updated.length) this.toast(tn(r.updated.length, "{n} note mise à jour avec le nouveau nom", "{n} notes mises à jour avec le nouveau nom"), "success");
    return to;
  }

  async move(path: string, folder: string) {
    const name = path.split("/").pop()!;
    const to = joinPath(folder, name);
    if (to === path || to.startsWith(path + "/")) return;
    await this.saveAll();
    const r = await api.rename(path, to, true);
    this.retab(path, to);
    await this.reloadOpen(r.updated);
    await this.refresh();
  }

  async remove(path: string) {
    const ok = await this.confirm(
      t("Supprimer ?"),
      t("« {path} » sera déplacé dans la corbeille du coffre (.eloria/trash). Tu pourras le récupérer depuis l'explorateur de fichiers.", { path }),
      true,
    );
    if (!ok) return;
    for (const [pi, p] of this.panes.entries()) {
      for (const tb of [...p.tabs]) {
        if (tb.path === path || tb.path?.startsWith(path + "/")) {
          editors.get(tb.id)?.markDeleted();
          await this.closeTab(pi, tb.id);
        }
      }
    }
    await api.deletePath(path);
    await this.refresh();
    this.toast(t("Déplacé dans la corbeille"), "info");
  }

  async reloadOpen(paths: string[]) {
    for (const p of paths) {
      const eds = editors.allByPath(p);
      if (eds.length) {
        const f = await api.readNote(p);
        for (const ed of eds) ed.externalUpdate(f.content, f.mtime, true);
      }
    }
  }

  async followLink(target: string, from: string, opts: { otherPane?: boolean; newTab?: boolean } = {}) {
    const [name, heading] = target.split("#");
    if (!name.trim() && heading) return editors.active()?.reveal(undefined, heading);
    let path = await api.resolveLink(name, from);
    if (!path) {
      path = await api.createFromLink(name, from);
      await this.refresh();
      this.toast(t("Note « {name} » créée", { name: stem(path) }), "success");
    }
    await this.openNote(path, { ...opts, heading: heading || undefined });
  }

  async openDaily() {
    const p = await api.dailyNote();
    await this.refresh();
    await this.openNote(p, { newTab: true });
  }

  // ------------------------------------------------------------- Zotero

  unlistenZotero: (() => void) | null = null;
  /** Zotero sync in progress (status bar). */
  zoteroSyncing = $state(false);

  /** Zotero changed: quiet resync, reported only if there is something new. */
  async zoteroAutoSync() {
    if (this.zoteroSyncing || !this.settings?.zotero_auto_sync) return;
    this.zoteroSyncing = true;
    const before = this.zoteroStatus;
    try {
      this.zoteroStatus = await api.zoteroSync();
      this.zoteroItems = await api.zoteroItems(null, null, 20000);
      this.fiches = await api.fichesMap();
      this.revision++;
      const docs = this.zoteroStatus.items - (before?.items ?? 0);
      const anns = this.zoteroStatus.annotations - (before?.annotations ?? 0);
      if (docs > 0 || anns > 0) {
        const parts = [docs > 0 ? tn(docs, "{n} nouveau document", "{n} nouveaux documents") : "", anns > 0 ? tn(anns, "{n} nouvelle annotation", "{n} nouvelles annotations") : ""].filter(Boolean);
        this.toast(t("Zotero mis à jour : {what}", { what: parts.join(", ") }), "success");
      }
    } catch {
      /* Zotero busy: the next change will restart the sync. */
    } finally {
      this.zoteroSyncing = false;
    }
  }

  async zoteroSync(source?: "auto" | "sqlite" | "api") {
    this.zoteroSyncing = true;
    const waiting = this.toast(t("Synchronisation Zotero…"), "info");
    try {
      this.zoteroStatus = await api.zoteroSync(source);
      this.zoteroItems = await api.zoteroItems(null, null, 20000);
      this.fiches = await api.fichesMap();
      this.dismiss(waiting);
      this.toast(
        t("Zotero : {docs}, {anns} ({source})", {
          docs: tn(this.zoteroStatus.items, "{n} document", "{n} documents"),
          anns: tn(this.zoteroStatus.annotations, "{n} annotation", "{n} annotations"),
          source: this.zoteroStatus.source ?? "",
        }),
        "success",
      );
      this.revision++;
    } catch (e) {
      this.dismiss(waiting);
      this.toast(translateError(e), "error");
    } finally {
      this.zoteroSyncing = false;
    }
  }

  /** New reading note attached to the item's source: title and "Eloria method" outline. */
  async createFiche(key: string, opts: { otherPane?: boolean } = {}) {
    try {
      const r = await api.createFiche(key);
      await this.refresh();
      this.fiches = await api.fichesMap();
      await this.openNote(r.path, { newTab: true, ...opts });
      this.toast(t("Fiche créée et jointe à la source"), "success");
    } catch (e) {
      this.toast(translateError(e), "error");
    }
  }

  /** New LaTeX document (article, chapter or presentation template). */
  async newLatex() {
    const name = await this.ask(t("Nom du document LaTeX"), t("Chapitre"));
    if (!name) return;
    const choice = await this.ask(t("Modèle : article, chapitre ou beamer"), "chapitre");
    if (!choice) return;
    try {
      const folder = this.settings?.language === "en" ? "Thesis" : "Thèse";
      const path = await api.latexNew(folder, name, choice.trim().toLowerCase());
      await this.refresh();
      this.openView("latex", { path, title: path.split("/").pop() });
    } catch (e) {
      this.toast(translateError(e), "error");
    }
  }

  /** New literature review: a `type: revue` note, opened in its view. */
  async newReview() {
    const title = await this.ask(t("Titre de la revue de littérature"), t("Revue de littérature"));
    if (!title) return;
    const question = (await this.ask(t("Question de recherche (facultatif)"), "")) ?? "";
    try {
      const path = await api.createReview(title, question);
      await this.refresh();
      this.openView("review", { path, title });
    } catch (e) {
      this.toast(translateError(e), "error");
    }
  }

  /** "Create reading note" from a vault source (Zotero or not). */
  async createFicheFromSource(path: string) {
    try {
      const r = await api.createFicheFromSource(path);
      await this.refresh();
      await this.openNote(r.path, { newTab: true });
      this.toast(t("Fiche créée et jointe à la source"), "success");
    } catch (e) {
      this.toast(translateError(e), "error");
    }
  }

  /** Adds an article to Eloria's library (source in the vault) and opens it. */
  async addReference(work: Work) {
    try {
      const r = await api.addReference(work);
      await this.refresh();
      this.fiches = await api.fichesMap();
      await this.openNote(r.path, { newTab: true });
      this.toast(r.created ? t("Source ajoutée à ta bibliothèque") : t("Cet article est déjà dans ta bibliothèque"), r.created ? "success" : "info");
      return r;
    } catch (e) {
      this.toast(translateError(e), "error");
    }
  }

  /** Adds PDFs from disk (picked or dropped): one source each, DOI read from the file. */
  async addPdfs(paths: string[]) {
    for (const p of paths) {
      try {
        this.toast(t("Lecture de {name}…", { name: p.split(/[\\/]/).pop() ?? p }), "info");
        const r = await api.addPdf(p);
        await this.refresh();
        this.fiches = await api.fichesMap();
        if (paths.length === 1) await this.openNote(r.path, { newTab: true });
        this.toast(
          !r.created
            ? t("Ce document est déjà dans ta bibliothèque")
            : r.doi
              ? t("Source ajoutée d'après le DOI {doi}", { doi: r.doi })
              : t("Source ajoutée : aucun DOI dans le PDF, titre tiré du nom du fichier"),
          r.created ? "success" : "info",
        );
        void this.indexPdfText(r.pdf);
      } catch (e) {
        this.toast(translateError(e), "error");
      }
    }
  }

  async pickPdf() {
    const p = await pickFile(["pdf"], "PDF");
    if (p) await this.addPdfs([p]);
  }

  /** PDFs dropped on the window. False if there was no PDF. */
  async dropPdfs(files: File[]): Promise<boolean> {
    const pdfs = files.filter((f) => /\.pdf$/i.test(f.name));
    if (!pdfs.length) return false;
    const paths: string[] = [];
    for (const f of pdfs) {
      try {
        paths.push(await api.stagePdf(f));
      } catch (e) {
        this.toast(translateError(e), "error");
      }
    }
    await this.addPdfs(paths);
    return true;
  }

  /** Extracts and keeps the text of a vault PDF (search, Ask Eloria, missing abstract). */
  async indexPdfText(rel: string, quiet = false) {
    try {
      if (!rel || (await api.hasPdfText(rel))) return;
      const { urlText } = await import("./pdftext");
      const url = isTauri ? await assetUrl(`${this.vault?.root ?? ""}/${rel}`) : "/demo/exemple.pdf";
      const done = await api.storePdfText(rel, await urlText(url));
      if (done.length) {
        await this.refresh();
        if (!quiet) this.toast(t("Résumé lu dans le PDF et ajouté à la source"), "success");
      }
      return done.length;
    } catch {
      // Unreadable PDF (scanned, protected): the source stays without full text.
      return 0;
    }
  }

  /** Extracts the text of every vault PDF that does not have it yet. */
  async extractPdfTexts() {
    const list = await api.pdfsWithoutText();
    if (!list.length) {
      this.toast(t("Le texte de tous les PDF est déjà extrait"), "info");
      return;
    }
    this.toast(tn(list.length, "Extraction du texte de {n} PDF…", "Extraction du texte de {n} PDF…"), "info");
    let abstracts = 0;
    for (const rel of list) abstracts += (await this.indexPdfText(rel, true)) ?? 0;
    this.toast(
      abstracts
        ? t("Texte de {n} PDF extrait ; {m}", { n: list.length, m: tn(abstracts, "{n} résumé lu dans les PDF", "{n} résumés lus dans les PDF") })
        : t("Texte de {n} PDF extrait : la recherche et Ask Eloria le lisent", { n: list.length }),
      "success",
    );
  }

  /** New argument: the note is created, then its map opens. */
  async newArgument() {
    const title = await this.ask(t("Ce que tu veux démontrer (l'affirmation, en une phrase courte)"), "");
    if (!title?.trim()) return;
    try {
      const p = await api.argumentNew(title.trim());
      await this.refresh();
      this.openView("argument", { path: p, title: title.trim() });
    } catch (e) {
      this.toast(translateError(e), "error");
    }
  }

  /** New research question (Thèse/Questions folder), attached to a chapter if given. */
  async newQuestion(chapter = "") {
    const q = await this.ask(t("Ta question de recherche"), "");
    if (!q?.trim()) return;
    const name = q.trim().replace(/[?!.:;]+$/, "").slice(0, 80);
    const content = `---\ntype: question\nstatut: ouverte\nchapitres: [${chapter}]\ntags: [thèse/question]\n---\n# ${q.trim()}\n\n## ${t("Pourquoi c'est important")}\n\n\n## ${t("Sources")}\n\n\n## ${t("Réponse provisoire")}\n\n`;
    await this.newNote(`${t("Thèse")}/${t("Questions")}`, name, content);
  }

  /** "New document" flow for the latest Zotero item without a source. */
  async prepareLatestDocument() {
    const [d] = await api.newZoteroDocuments(1).catch(() => []);
    if (d) this.flowFor = { key: d.key, title: d.title };
    else this.toast(t("Tous les documents Zotero ont déjà leur source dans le coffre"), "info");
  }

  /** Completes a source online (abstract, keywords, journal, year). */
  async completeSource(path: string) {
    try {
      const changed = await api.completeSource(path);
      await this.refresh();
      this.toast(changed ? t("Source complétée : résumé et mots-clés ajoutés") : t("Rien de plus à ajouter : les bases en ligne n'en savent pas davantage"), changed ? "success" : "info");
    } catch (e) {
      this.toast(translateError(e), "error");
    }
  }

  /** Completes every source that has a DOI but no abstract or no keywords. */
  async completeSources() {
    try {
      let shown = false;
      const n = await api.completeSources(([done, total]) => {
        if (!shown && total > 0) {
          shown = true;
          this.toast(tn(total, "Recherche en ligne pour {n} source…", "Recherche en ligne pour {n} sources…"), "info");
        }
        void done;
      });
      await this.refresh();
      this.toast(n ? tn(n, "{n} source complétée", "{n} sources complétées") : t("Aucune source à compléter"), n ? "success" : "info");
    } catch (e) {
      this.toast(translateError(e), "error");
    }
  }

  /** Imports an item's source into the vault (if needed) and opens it. */
  async importSource(key: string, opts: { otherPane?: boolean } = {}) {
    try {
      const r = await api.importSource(key);
      await this.refresh();
      this.fiches = await api.fichesMap();
      await this.openNote(r.path, { newTab: true, ...opts });
      if (r.created) this.toast(t("Source importée dans le coffre"), "success");
    } catch (e) {
      this.toast(translateError(e), "error");
    }
  }

  async syncFiche(path: string) {
    try {
      await editors.savePath(path);
      const content = await api.syncFiche(path);
      editors.pushContent(path, content);
      await this.refresh();
      this.toast(t("Annotations synchronisées"), "success");
    } catch (e) {
      this.toast(translateError(e), "error");
    }
  }

  // ------------------------------------------------------------- settings

  async updateSettings(patch: Partial<Settings>) {
    if (!this.settings) return;
    this.settings = { ...this.settings, ...patch };
    applySettings(this.settings);
    editors.all().forEach((e) => e.reconfigure());
    this.persistSettings();
  }

  persistSettings = debounce(() => this.settings && api.saveSettings($state.snapshot(this.settings) as Settings), 400);

  /** Saves right away (closing the settings): the core sees the new settings. */
  async saveSettingsNow() {
    if (this.settings) await api.saveSettings($state.snapshot(this.settings) as Settings);
  }

  // ------------------------------------------------------------- dialogues

  toast(text: string, kind: Toast["kind"] = "info", action?: Toast["action"]): string {
    const id = uid("toast");
    this.toasts.push({ id, text, kind, action });
    setTimeout(() => this.dismiss(id), kind === "error" ? 9000 : action ? 8000 : 3500);
    return id;
  }

  dismiss(id: string) {
    this.toasts = this.toasts.filter((tb) => tb.id !== id);
  }

  ask(title: string, value = "", placeholder?: string): Promise<string | null> {
    return new Promise((resolve) => (this.prompt = { title, value, placeholder, resolve }));
  }

  confirm(title: string, message: string, danger = false, confirmLabel?: string): Promise<boolean> {
    return new Promise((resolve) => (this.confirmBox = { title, message, danger, confirmLabel, resolve }));
  }

  openMenu(e: MouseEvent, items: MenuItem[]) {
    e.preventDefault();
    e.stopPropagation();
    this.menu = { x: e.clientX, y: e.clientY, items };
  }
}

export const app = new AppState();

/** Applies theme, fonts and sizes through CSS variables. */
export function applySettings(s: Settings) {
  const root = document.documentElement;
  i18n.lang = s.language === "en" ? "en" : "fr";
  root.lang = i18n.lang;
  const dark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const custom = s.theme.startsWith(CUSTOM_PREFIX) ? app.customThemes.find((c) => CUSTOM_PREFIX + c.id === s.theme) : undefined;
  applyTheme(s.theme, custom ? { css: custom.css, scheme: custom.scheme } : null, dark);
  const set = (k: string, v: string) => root.style.setProperty(k, v);
  set("--accent", s.accent);
  set("--font-ui", s.font_ui);
  set("--font-editor", s.font_editor);
  set("--font-mono", s.font_mono);
  set("--editor-size", `${s.font_size}px`);
  set("--editor-lh", String(s.line_height));
  set("--editor-width", s.readable_line_length && s.editor_width > 0 ? `${s.editor_width}px` : "100%");
  set("--h-scale", String(s.heading_scale));
}
