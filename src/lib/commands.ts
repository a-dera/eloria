// Global commands: palette (Ctrl+P) and keyboard shortcuts.

import { api, pickSaveFile } from "./api";
import { app } from "./state.svelte";
import { editors } from "./editor/registry";
import { formatTable, insertBlock, setHeading, tableTemplate, toggleLinePrefix, toggleWrap } from "./editor/commands";
import { isAltGrText, isMac } from "./util";
import { t, tk, tn } from "./i18n.svelte";
import { THEMES } from "./themes";

export interface Command {
  id: string;
  label: string;
  /** Shortcut in the "Mod+Shift+K" format (Mod = Ctrl or Cmd) */
  keys?: string;
  group: string;
  run: () => void | Promise<void>;
  needsNote?: boolean;
  /** Label already in the user's language (custom commands) */
  raw?: boolean;
}

/** Displayed label of a command. */
export const labelOf = (c: Command) => (c.raw ? c.label : t(c.label));

const ed = () => editors.active()?.view;

function withEditor(f: (v: NonNullable<ReturnType<typeof ed>>) => void) {
  return () => {
    const v = ed();
    if (v) {
      f(v);
      v.focus();
    }
  };
}

export const commands: Command[] = [
  // Navigation
  { id: "palette", label: tk("Ouvrir la palette de commandes"), keys: "Mod+P", group: tk("Navigation"), run: () => void (app.palette = "commands") },
  { id: "quick-open", label: tk("Ouvrir une note…"), keys: "Mod+O", group: tk("Navigation"), run: () => void (app.palette = "files") },
  {
    id: "search",
    label: tk("Rechercher dans le coffre"),
    keys: "Mod+Shift+F",
    group: tk("Navigation"),
    run: () => {
      app.leftOpen = true;
      app.leftPanel = "search";
      setTimeout(() => document.querySelector<HTMLInputElement>(".search-input")?.focus(), 30);
    },
  },
  { id: "ask-eloria", label: tk("Ask Eloria : poser une question à mes notes (IA)…"), keys: "Mod+Shift+A", group: tk("IA"), run: () => app.toggleAsk(true) },
  { id: "ai-concepts", label: tk("Concepts de cette note (IA)…"), group: tk("IA"), needsNote: true, run: () => app.activeNotePath && (app.suggestFor = { path: app.activeNotePath, kind: "concepts" }) },
  { id: "ai-tags", label: tk("Tags suggérés pour cette note (IA)…"), group: tk("IA"), needsNote: true, run: () => app.activeNotePath && (app.suggestFor = { path: app.activeNotePath, kind: "tags" }) },
  { id: "ai-summarize-note", label: tk("Résumer la note (IA)…"), group: tk("IA"), run: () => app.openRewrite("summarize", true) },
  { id: "ai-rewrite", label: tk("Assistant de rédaction sur la sélection (IA)…"), keys: "Mod+Shift+J", group: tk("IA"), run: () => app.openRewrite("rephrase") },
  { id: "ai-fix", label: tk("Corriger la sélection (IA)…"), group: tk("IA"), run: () => app.openRewrite("fix") },
  { id: "ai-academic", label: tk("Style académique sur la sélection (IA)…"), group: tk("IA"), run: () => app.openRewrite("academic") },
  { id: "capture-clipboard", label: tk("Capturer le presse-papiers dans la boîte de réception"), keys: "Mod+Shift+V", group: tk("Fichiers"), run: () => app.captureClipboard() },
  { id: "capture-idea", label: tk("Noter une idée (boîte de réception)…"), group: tk("Fichiers"), run: () => app.captureIdea() },
  { id: "import-markdown", label: tk("Importer un dossier Markdown (Obsidian, Notion, Logseq)…"), group: tk("Fichiers"), run: () => app.importMarkdownDir() },
  { id: "git-save", label: tk("Enregistrer une version Git du coffre"), group: tk("Fichiers"), run: () => app.gitSave() },
  { id: "compare", label: tk("Comparer la note avec un autre document…"), group: tk("Fichiers"), needsNote: true, run: () => app.activeNotePath && (app.compareFor = app.activeNotePath) },
  { id: "citation-network", label: tk("Réseau de citations de la bibliothèque"), group: tk("Zotero"), run: () => app.openView("citations") },
  { id: "weekly-review", label: tk("Revue de la semaine"), group: tk("Navigation"), run: () => app.weeklyReview() },
  { id: "focus-session", label: tk("Démarrer ou arrêter une session de concentration"), group: tk("Navigation"), run: () => (app.focus ? app.stopFocus() : app.startFocus()) },
  { id: "assemble-thesis", label: tk("Assembler la thèse en LaTeX (chapitres → manuscrit)"), group: tk("Fichiers"), run: () => app.assembleThesis() },
  { id: "note-to-latex", label: tk("Convertir la note en document LaTeX"), group: tk("Fichiers"), needsNote: true, run: () => app.activeNotePath && app.noteToLatex(app.activeNotePath) },
  { id: "methods", label: tk("Méthodologies : méthodes rencontrées et articles qui les emploient"), group: tk("Zotero"), run: () => app.openView("methodes") },
  { id: "latex-tools", label: tk("Formules, tableau, aperçu TikZ, renvois (outils LaTeX)…"), group: tk("Fichiers"), run: () => (app.latexToolsFor = { path: app.activeTab?.path ?? "", tab: "formules" }) },
  { id: "science", label: tk("Assistant d'écriture scientifique : tableau, équation, TikZ, formule (IA)…"), group: tk("IA"), run: () => app.openScience() },
  { id: "support", label: tk("Cohérence des citations : chaque phrase est-elle soutenue par sa source ? (IA)"), group: tk("IA"), needsNote: true, run: () => app.openScience("citations") },
  { id: "exchange", label: tk("Import et export, espace disque du coffre"), group: tk("Fichiers"), run: () => app.openView("echanges") },
  { id: "bib-import", label: tk("Importer un fichier BibTeX (.bib)…"), group: tk("Zotero"), run: () => app.openView("echanges") },
  { id: "help", label: tk("Aide d'Eloria"), keys: "F1", group: tk("Fichiers"), run: () => app.openView("aide") },
  { id: "atlas", label: tk("Carte des connaissances (concepts, méthodes, questions, chapitres)"), group: tk("Fichiers"), run: () => app.openView("atlas") },
  { id: "taxonomy", label: tk("Taxonomie personnelle (classification hiérarchique)"), group: tk("Fichiers"), run: () => app.openView("taxonomie") },
  { id: "entities", label: tk("Entités : personnes, organisations, méthodes, technologies, lieux"), group: tk("Fichiers"), run: () => app.openView("entites") },
  { id: "topics", label: tk("Thèmes : regroupement automatique des notes"), group: tk("Fichiers"), run: () => app.openView("themes") },
  { id: "concepts", label: tk("Concepts : base de concepts, chronologie, fiches de connaissance"), group: tk("Fichiers"), run: () => app.openView("concepts") },
  { id: "research-stats", label: tk("Statistiques de recherche (citations, thèmes, couverture)"), group: tk("Fichiers"), run: () => app.openView("statistiques") },
  { id: "new-argument", label: tk("Nouvelle argumentation (affirmation, arguments, preuves, objections)…"), group: tk("Fichiers"), run: () => app.newArgument() },
  { id: "argument-map", label: tk("Carte de l'argumentation de la note"), group: tk("Fichiers"), needsNote: true, run: () => {
      if (app.activeNotePath) app.openView("argument", { path: app.activeNotePath, title: app.noteByPath(app.activeNotePath)?.title });
    },
  },
  { id: "outline", label: tk("Générer un plan à partir de notes et de sources…"), group: tk("Fichiers"), run: () => (app.outlineFor = { scope: "chapter", value: "" }) },
  { id: "thesis-space", label: tk("Espace thèse (ordre des chapitres, objectifs, questions)"), group: tk("Fichiers"), run: () => app.openView("these") },
  { id: "new-question", label: tk("Nouvelle question de recherche"), group: tk("Fichiers"), run: () => app.newQuestion() },
  { id: "new-document-flow", label: tk("Préparer le dernier document arrivé dans Zotero"), group: tk("Zotero"), run: () => app.prepareLatestDocument() },
  { id: "reliability", label: tk("Fiabilité des sources (rétractations, revues, citations)"), group: tk("Zotero"), run: () => (app.checkFor = { kind: "fiabilite" }) },
  { id: "add-pdf", label: tk("Ajouter un PDF à la bibliothèque…"), group: tk("Zotero"), run: () => app.pickPdf() },
  { id: "complete-sources", label: tk("Compléter les sources en ligne (résumé, mots-clés)"), group: tk("Zotero"), run: () => app.completeSources() },
  { id: "extract-pdf-text", label: tk("Extraire le texte des PDF (recherche, Ask Eloria)"), group: tk("Zotero"), run: () => app.extractPdfTexts() },
  { id: "dashboard", label: tk("Tableau de bord"), group: tk("Navigation"), run: () => app.openView("dashboard") },
  { id: "similarity-check", label: tk("Similarité avec la bibliothèque (passages repris mot pour mot)"), group: tk("Fichiers"), needsNote: true, run: () => app.activeNotePath && (app.checkFor = { kind: "similarite", path: app.activeNotePath }) },
  { id: "style-check", label: tk("Vérifier le style académique de la note"), group: tk("Fichiers"), needsNote: true, run: () => app.activeNotePath && (app.checkFor = { kind: "style", path: app.activeNotePath }) },
  { id: "check-citations", label: tk("Vérifier les citations de la note"), group: tk("Fichiers"), needsNote: true, run: () => app.activeNotePath && (app.checkFor = { kind: "citations", path: app.activeNotePath }) },
  { id: "bib-audit", label: tk("Audit de la bibliographie"), group: tk("Zotero"), run: () => (app.checkFor = { kind: "bib" }) },
  { id: "orphans", label: tk("Notes orphelines et doublons"), group: tk("Fichiers"), run: () => (app.checkFor = { kind: "notes" }) },
  { id: "check-integrity", label: tk("Vérifier le coffre (liens, fichiers, propriétés)…"), group: tk("Fichiers"), run: () => (app.integrityOpen = true) },
  { id: "backup-now", label: tk("Sauvegarder le coffre maintenant"), group: tk("Fichiers"), run: () => app.backupNow() },
  { id: "add-reference", label: tk("Ajouter une référence (DOI, titre)…"), group: tk("Zotero"), run: () => (app.addRefOpen = true) },
  { id: "graph", label: tk("Graphe du coffre"), keys: "Mod+G", group: tk("Navigation"), run: () => app.openView("graph") },
  { id: "kanban", label: tk("Tableau Kanban des fiches"), group: tk("Navigation"), run: () => app.openView("kanban") },
  { id: "table", label: tk("Base de notes (tableau filtrable)"), group: tk("Navigation"), run: () => app.openView("table") },
  { id: "dev-console", label: tk("Console développeur"), keys: "Mod+Alt+I", group: tk("Extensions"), run: () => app.openView("console") },
  { id: "extensions-reload", label: tk("Recharger les extensions"), group: tk("Extensions"), run: () => void import("./extensions.svelte").then((m) => m.loadExtensions()) },
  {
    id: "extensions-settings",
    label: tk("Gérer les extensions…"),
    group: tk("Extensions"),
    run: () => {
      app.settingsSection = "extensions";
      app.settingsOpen = true;
    },
  },
  { id: "new-latex", label: tk("Nouveau document LaTeX…"), group: tk("Fichiers"), run: () => app.newLatex() },
  { id: "new-review", label: tk("Nouvelle revue de littérature…"), group: tk("Fichiers"), run: () => app.newReview() },
  { id: "matrix", label: tk("Matrice de recherche (comparer les articles)"), group: tk("Navigation"), run: () => app.openView("matrix") },
  { id: "timeline", label: tk("Frise chronologique des lectures"), group: tk("Navigation"), run: () => app.openView("timeline") },
  { id: "daily", label: tk("Note du jour"), keys: "Mod+Shift+D", group: tk("Navigation"), run: () => app.openDaily() },
  { id: "next-tab", label: tk("Onglet suivant"), keys: "Mod+Tab", group: tk("Navigation"), run: () => app.cycleTab(1) },
  { id: "prev-tab", label: tk("Onglet précédent"), keys: "Mod+Shift+Tab", group: tk("Navigation"), run: () => app.cycleTab(-1) },
  { id: "close-tab", label: tk("Fermer l'onglet"), keys: "Mod+W", group: tk("Navigation"), run: () => app.closeActiveTab() },
  { id: "split", label: tk("Diviser / fusionner l'écran"), keys: "Mod+\\", group: tk("Affichage"), run: () => app.toggleSplit() },

  // Fichiers
  { id: "new-note", label: tk("Nouvelle note"), keys: "Mod+N", group: tk("Fichiers"), run: () => void app.newNote() },
  {
    id: "new-from-template",
    label: tk("Nouvelle note depuis un modèle…"),
    keys: "Mod+Shift+N",
    group: tk("Fichiers"),
    run: () => void (app.palette = "templates"),
  },
  { id: "new-folder", label: tk("Nouveau dossier"), group: tk("Fichiers"), run: () => app.newFolder("") },
  {
    id: "rename",
    label: tk("Renommer la note"),
    keys: "F2",
    group: tk("Fichiers"),
    needsNote: true,
    run: () => document.querySelector<HTMLInputElement>(".pane.active .note-title-input")?.select(),
  },
  {
    id: "delete",
    label: tk("Supprimer la note"),
    group: tk("Fichiers"),
    needsNote: true,
    run: () => {
      const p = app.activeNotePath;
      if (p) app.remove(p);
    },
  },
  {
    id: "history",
    label: tk("Historique des versions"),
    group: tk("Fichiers"),
    needsNote: true,
    run: () => void (app.historyFor = app.activeNotePath),
  },
  {
    id: "export",
    label: tk("Exporter la note (Word, PDF, HTML…)"),
    keys: "Mod+Shift+E",
    group: tk("Fichiers"),
    needsNote: true,
    run: () => void (app.exportFor = app.activeNotePath),
  },
  {
    id: "reveal",
    label: tk("Afficher dans l'explorateur"),
    group: tk("Fichiers"),
    needsNote: true,
    run: () => {
      const p = app.activeNotePath;
      if (p) api.revealPath(p);
    },
  },
  {
    id: "rebuild",
    label: tk("Reconstruire l'index"),
    group: tk("Fichiers"),
    run: async () => {
      const s = await api.rebuildIndex();
      await app.refresh();
      app.toast(tn(s.total, "Index reconstruit : {n} note", "Index reconstruit : {n} notes"), "success");
    },
  },
  { id: "close-vault", label: tk("Changer de coffre"), group: tk("Fichiers"), run: () => app.closeVault() },

  // Editing
  { id: "bold", label: tk("Gras"), keys: "Mod+B", group: tk("Édition"), needsNote: true, run: withEditor((v) => toggleWrap(v, "**")) },
  { id: "italic", label: tk("Italique"), keys: "Mod+I", group: tk("Édition"), needsNote: true, run: withEditor((v) => toggleWrap(v, "*")) },
  { id: "highlight", label: tk("Surligner"), keys: "Mod+Shift+H", group: tk("Édition"), needsNote: true, run: withEditor((v) => toggleWrap(v, "==")) },
  { id: "strike", label: tk("Barrer"), keys: "Mod+Shift+X", group: tk("Édition"), needsNote: true, run: withEditor((v) => toggleWrap(v, "~~")) },
  ...[1, 2, 3].map((l) => ({
    id: `h${l}`,
    label: ["", tk("Titre 1"), tk("Titre 2"), tk("Titre 3")][l],
    keys: `Mod+Alt+${l}`,
    group: tk("Édition"),
    needsNote: true,
    run: withEditor((v) => setHeading(v, l)),
  })),
  { id: "bullet", label: tk("Liste à puces"), keys: "Mod+Shift+8", group: tk("Édition"), needsNote: true, run: withEditor((v) => toggleLinePrefix(v, "bullet")) },
  { id: "task", label: tk("Case à cocher"), keys: "Mod+L", group: tk("Édition"), needsNote: true, run: withEditor((v) => toggleLinePrefix(v, "task")) },
  { id: "table-insert", label: tk("Insérer un tableau"), group: tk("Édition"), needsNote: true, run: withEditor((v) => insertBlock(v, tableTemplate(3, 2))) },
  { id: "table-format", label: tk("Aligner le tableau"), keys: "Mod+Alt+T", group: tk("Édition"), needsNote: true, run: withEditor((v) => void formatTable(v)) },
  {
    id: "mermaid",
    label: tk("Insérer un diagramme Mermaid"),
    group: tk("Édition"),
    needsNote: true,
    run: withEditor((v) => insertBlock(v, "```mermaid\nflowchart LR\n  A[§Début] --> B[Fin]\n```")),
  },
  { id: "find", label: tk("Rechercher / remplacer dans la note"), keys: "Mod+F", group: tk("Édition"), needsNote: true, run: withEditor(() => {}) },

  // Affichage
  {
    id: "toggle-preview",
    label: tk("Basculer aperçu en direct / Markdown brut"),
    keys: "Mod+Shift+P",
    group: tk("Affichage"),
    run: () => app.settings && app.updateSettings({ live_preview: !app.settings.live_preview }),
  },
  { id: "focus", label: tk("Mode concentration"), keys: "F11", group: tk("Affichage"), run: () => void (app.focusMode = !app.focusMode) },
  { id: "toggle-left", label: tk("Afficher / masquer le panneau gauche"), keys: "Mod+Shift+L", group: tk("Affichage"), run: () => void (app.leftOpen = !app.leftOpen) },
  { id: "toggle-right", label: tk("Afficher / masquer le panneau droit"), keys: "Mod+Shift+R", group: tk("Affichage"), run: () => app.toggleRight() },
  {
    id: "theme",
    label: tk("Thème suivant"),
    group: tk("Affichage"),
    run: () => {
      // Cycles through the gallery: light, dark, sepia, then the popular themes.
      const cur = document.documentElement.dataset.theme ?? "light";
      const i = THEMES.findIndex((th) => th.id === cur);
      const next = THEMES[(i + 1) % THEMES.length];
      app.updateSettings({ theme: next.id, accent: next.accent });
      app.toast(t("Thème : {name}", { name: t(next.name) }), "info");
    },
  },
  { id: "zoom-in", label: tk("Agrandir le texte"), keys: "Mod+=", group: tk("Affichage"), run: () => app.settings && app.updateSettings({ font_size: Math.min(30, app.settings.font_size + 1) }) },
  { id: "zoom-out", label: tk("Réduire le texte"), keys: "Mod+-", group: tk("Affichage"), run: () => app.settings && app.updateSettings({ font_size: Math.max(11, app.settings.font_size - 1) }) },
  { id: "settings", label: tk("Réglages"), keys: "Mod+,", group: tk("Affichage"), run: () => void (app.settingsOpen = true) },

  // Zotero
  { id: "zotero-sync", label: tk("Synchroniser Zotero"), group: tk("Zotero"), run: () => app.zoteroSync() },
  {
    id: "ai-fiche",
    label: tk("Méthode Eloria : rédiger cette fiche (IA)…"),
    group: tk("Intelligence artificielle"),
    needsNote: true,
    run: () => {
      const n = app.noteByPath(app.activeNotePath);
      const key = n?.frontmatter.zotero;
      if (typeof key === "string" && n?.frontmatter.type === "fiche") app.aiFicheFor = { key, title: n.title, path: n.path };
      else app.toast(t("Ouvre une fiche jointe à une source pour utiliser la méthode Eloria."), "info");
    },
  },
  { id: "zotero-import", label: tk("Importer la bibliothèque Zotero en notes…"), group: tk("Zotero"), run: () => void (app.importOpen = true) },
  {
    id: "zotero-panel",
    label: tk("Ouvrir le panneau Zotero"),
    group: tk("Zotero"),
    run: () => {
      app.leftOpen = true;
      app.leftPanel = "zotero";
    },
  },
  {
    id: "zotero-cite",
    label: tk("Insérer une citation Zotero"),
    keys: "Mod+Shift+C",
    group: tk("Zotero"),
    needsNote: true,
    run: withEditor((v) => {
      const r = v.state.selection.main;
      v.dispatch({ changes: { from: r.from, to: r.to, insert: "[@]" }, selection: { anchor: r.from + 2 } });
      setTimeout(() => import("@codemirror/autocomplete").then((m) => m.startCompletion(v)), 20);
    }),
  },
  {
    id: "fiche-sync",
    label: tk("Resynchroniser les annotations de cette fiche"),
    group: tk("Zotero"),
    needsNote: true,
    run: () => {
      const p = app.activeNotePath;
      if (p) app.syncFiche(p);
    },
  },
  {
    id: "bib-export",
    label: tk("Exporter la bibliographie (.bib)"),
    group: tk("Zotero"),
    run: async () => {
      const out = await pickSaveFile("bibliographie.bib", "bib", "BibTeX");
      if (!out) return;
      const n = await api.exportBibtex(out, false);
      app.toast(tn(n, "{n} référence exportée", "{n} références exportées"), "success");
    },
  },
];

// ---------------------------------------------------------------- raccourcis

/** Effective shortcut of a command: custom (Settings › Shortcuts) or default. */
export function keysOf(c: Command): string | undefined {
  const custom = app.settings?.keys?.[c.id];
  if (custom === undefined) return c.keys;
  return custom || undefined;
}

const MODIFIERS = new Set(["Control", "Shift", "Alt", "Meta", "AltGraph", "OS"]);

/**
 * Combination in the command format ("Mod+Shift+K") from a keystroke, or null
 * (modifier key alone, or an ordinary key without a modifier).
 * Read from the physical key for letters and digits: independent of AZERTY / QWERTY.
 */
export function comboFromEvent(e: KeyboardEvent): string | null {
  if (MODIFIERS.has(e.key) || isAltGrText(e)) return null;
  let key: string;
  if (/^Key[A-Z]$/.test(e.code)) key = e.code.slice(3);
  else if (/^Digit\d$/.test(e.code)) key = e.code.slice(5);
  else if (/^F\d{1,2}$/.test(e.key)) key = e.key;
  else if (e.key === "Tab" || e.key === "Enter") key = e.key;
  else if (e.code === "Backslash" || e.code === "IntlBackslash") key = "\\";
  else if (e.key.length === 1) key = e.key.toUpperCase();
  else return null;
  const mod = isMac() ? e.metaKey : e.ctrlKey;
  if (!mod && !e.altKey && !/^F\d/.test(key)) return null;
  return [mod && "Mod", e.altKey && "Alt", e.shiftKey && "Shift", key].filter(Boolean).join("+");
}

/** Commands whose effective shortcut is shared with another one: keys -> identifiers. */
export function keyConflicts(): Map<string, string[]> {
  const by = new Map<string, string[]>();
  for (const c of commands) {
    const k = keysOf(c)?.toLowerCase();
    if (k) by.set(k, [...(by.get(k) ?? []), c.id]);
  }
  return new Map([...by].filter(([, ids]) => ids.length > 1));
}

function matches(e: KeyboardEvent, keys: string): boolean {
  if (isAltGrText(e)) return false;
  const parts = keys.split("+");
  const key = parts.pop()!.toLowerCase();
  const need = new Set(parts.map((p) => p.toLowerCase()));
  const mod = isMac() ? e.metaKey : e.ctrlKey;
  if (need.has("mod") !== mod) return false;
  if (need.has("shift") !== e.shiftKey) return false;
  if (need.has("alt") !== e.altKey) return false;
  const k = e.key.toLowerCase();
  if (key === "\\") return k === "\\" || e.code === "Backslash" || e.code === "IntlBackslash";
  if (key === "=") return k === "=" || k === "+";
  if (/^\d$/.test(key)) return e.code === `Digit${key}`;
  if (key === "tab") return k === "tab";
  return k === key || e.code === `Key${key.toUpperCase()}`;
}

/** Shortcuts handled by the editor itself when it has the focus. */
const EDITOR_OWNED = new Set(["bold", "italic", "highlight", "strike", "h1", "h2", "h3", "bullet", "task", "table-format", "find"]);

export function handleGlobalKey(e: KeyboardEvent): boolean {
  // A field recording a combination (Settings › Shortcuts) receives every key.
  if ((e.target as HTMLElement)?.closest?.("[data-capture-keys]")) return false;
  const inEditor = (e.target as HTMLElement)?.closest?.(".cm-editor");
  for (const c of commands) {
    const keys = keysOf(c);
    if (!keys || !matches(e, keys)) continue;
    // The editor handles its own default shortcuts; a custom shortcut goes through here.
    if (inEditor && EDITOR_OWNED.has(c.id) && keys === c.keys) return false;
    if (c.needsNote && !app.activeNotePath) return false;
    e.preventDefault();
    e.stopPropagation();
    c.run();
    return true;
  }
  return false;
}

export function prettyKeys(keys?: string): string {
  if (!keys) return "";
  return keys
    .replace("Mod", isMac() ? "⌘" : "Ctrl")
    .replace("Shift", isMac() ? "⇧" : t("Maj"))
    .replace("Alt", isMac() ? "⌥" : "Alt");
}
