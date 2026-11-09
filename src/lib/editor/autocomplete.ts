// Autocompletion: [[links]], #tags, [@Zotero citations] and Notion-style "/" commands.

import { autocompletion, startCompletion, type Completion, type CompletionContext, type CompletionResult } from "@codemirror/autocomplete";
import type { EditorView } from "@codemirror/view";
import { app } from "../state.svelte";
import { t, tk } from "../i18n.svelte";
import { fuzzyScore, today } from "../util";
import { tableTemplate } from "./commands";

function wikiSource(ctx: CompletionContext): CompletionResult | null {
  const m = ctx.matchBefore(/!?\[\[[^\]\n|]*$/);
  if (!m) return null;
  const start = m.from + (m.text.startsWith("!") ? 3 : 2);
  const q = m.text.slice(start - m.from);
  // [[Note#  -> headings of the note
  const hash = q.indexOf("#");
  if (hash >= 0) return null;
  const options: Completion[] = [];
  for (const n of app.notes) {
    const s = fuzzyScore(q, `${n.title} ${n.stem}`);
    if (s < 0) continue;
    const detail = n.path.includes("/") ? n.path.slice(0, n.path.lastIndexOf("/")) : "";
    options.push({ label: n.stem, displayLabel: n.title !== n.stem ? `${n.title}` : n.stem, detail, boost: s / 100, type: "note", apply: applyWiki(n.stem) });
    for (const a of n.aliases) {
      if (fuzzyScore(q, a) >= 0) options.push({ label: a, detail: t("alias de {name}", { name: n.stem }), type: "alias", apply: applyWiki(`${n.stem}|${a}`) });
    }
  }
  if (q.trim() && !app.notes.some((n) => n.stem.toLowerCase() === q.trim().toLowerCase())) {
    options.push({ label: q.trim(), detail: t("nouvelle note"), type: "new", boost: -50, apply: applyWiki(q.trim()) });
  }
  return { from: start, options: options.slice(0, 200), filter: false };
}

function applyWiki(text: string) {
  return (view: EditorView, _c: Completion, from: number, to: number) => {
    const after = view.state.sliceDoc(to, to + 2);
    const close = after === "]]" ? "" : after.startsWith("]") ? "]" : "]]";
    const insert = text + close;
    view.dispatch({
      changes: { from, to, insert },
      selection: { anchor: from + insert.length + (after === "]]" ? 2 : after.startsWith("]") ? 1 : 0) },
    });
  };
}

function tagSource(ctx: CompletionContext): CompletionResult | null {
  const m = ctx.matchBefore(/(?:^|[\s(])#[\p{L}\p{N}_/-]+$/u);
  if (!m) return null;
  const hashAt = m.text.indexOf("#");
  const from = m.from + hashAt + 1;
  return {
    from,
    options: app.tags.map(([t, n]) => ({ label: t, detail: `${n}`, type: "tag" })),
    validFor: /^[\p{L}\p{N}_/-]*$/u,
  };
}

function citeSource(ctx: CompletionContext): CompletionResult | null {
  const m = ctx.matchBefore(/@[\p{L}\p{N}_:.\-]*$/u);
  if (!m) return null;
  const line = ctx.state.doc.lineAt(ctx.pos);
  const before = line.text.slice(0, ctx.pos - line.from);
  const open = before.lastIndexOf("[");
  const close = before.lastIndexOf("]");
  const inBrackets = open > close;
  if (!inBrackets && !ctx.explicit) return null;
  const q = m.text.slice(1);
  if (!app.zoteroItems.length) {
    return {
      from: m.from + 1,
      options: [{ label: t("Bibliothèque Zotero vide"), detail: t("synchronise depuis le panneau Zotero"), apply: () => {} }],
      filter: false,
    };
  }
  const opts: Completion[] = [];
  for (const it of app.zoteroItems) {
    const s = fuzzyScore(q, `${it.citekey} ${it.authors} ${it.year ?? ""} ${it.title}`);
    if (s < 0) continue;
    opts.push({
      label: it.citekey,
      displayLabel: `${it.authors || "?"} (${it.year ?? t("s.d.")})`,
      detail: it.citekey,
      info: it.title,
      type: "citation",
      boost: s / 100,
    });
    if (opts.length > 300) break;
  }
  return { from: m.from + 1, options: opts, filter: false };
}

interface Slash {
  label: string;
  keywords: string;
  icon: string;
  run: (view: EditorView, from: number, to: number) => void;
}

/** Inserts a text (translated at insertion time); "§" marks the cursor position. */
function replaceWith(source: string) {
  return (view: EditorView, from: number, to: number) => {
    const text = t(source);
    const idx = text.indexOf("§");
    const clean = text.replace("§", "");
    view.dispatch({ changes: { from, to, insert: clean }, selection: { anchor: from + (idx >= 0 ? idx : clean.length) } });
  };
}

function linePrefix(prefix: string) {
  return (view: EditorView, from: number, to: number) => {
    const line = view.state.doc.lineAt(from);
    const rest = view.state.sliceDoc(to, line.to);
    view.dispatch({
      changes: { from: line.from, to: line.to, insert: prefix + view.state.sliceDoc(line.from, from).trimStart() + rest },
      selection: { anchor: line.from + prefix.length + (from - line.from) },
    });
  };
}

const SLASH: Slash[] = [
  { label: tk("Titre 1"), keywords: "h1 titre heading", icon: "H1", run: linePrefix("# ") },
  { label: tk("Titre 2"), keywords: "h2 titre heading", icon: "H2", run: linePrefix("## ") },
  { label: tk("Titre 3"), keywords: "h3 titre heading", icon: "H3", run: linePrefix("### ") },
  { label: tk("Liste à puces"), keywords: "liste puce bullet", icon: "•", run: linePrefix("- ") },
  { label: tk("Liste numérotée"), keywords: "liste numero ordered", icon: "1.", run: linePrefix("1. ") },
  { label: tk("Liste de tâches"), keywords: "tache todo checkbox case", icon: "☐", run: linePrefix("- [ ] ") },
  { label: tk("Citation"), keywords: "quote citation", icon: "❝", run: linePrefix("> ") },
  { label: tk("Callout note"), keywords: "callout encadre note", icon: "✎", run: replaceWith("> [!note] §\n> ") },
  { label: tk("Callout astuce"), keywords: "callout astuce tip", icon: "💡", run: replaceWith("> [!astuce] §\n> ") },
  { label: tk("Callout attention"), keywords: "callout warning attention", icon: "⚠", run: replaceWith("> [!attention] §\n> ") },
  { label: tk("Callout important"), keywords: "callout important", icon: "❗", run: replaceWith("> [!important] §\n> ") },
  { label: tk("Callout définition"), keywords: "definition callout", icon: "§", run: replaceWith("> [!definition] §\n> ") },
  { label: tk("Bloc repliable"), keywords: "toggle replier details fold", icon: "▸", run: replaceWith(tk("> [!note]- §Titre\n> Contenu replié")) },
  { label: tk("Tableau"), keywords: "table tableau grille", icon: "▦", run: replaceWith(tableTemplate(3, 2) + "\n") },
  { label: tk("Bloc de code"), keywords: "code", icon: "{}", run: replaceWith("```§\n\n```") },
  { label: tk("Diagramme : flux"), keywords: "mermaid flowchart diagramme schema", icon: "⇉", run: replaceWith(tk("```mermaid\nflowchart LR\n  A[§Début] --> B{Choix}\n  B -->|oui| C[Résultat]\n  B -->|non| D[Autre]\n```\n")) },
  { label: tk("Diagramme : séquence"), keywords: "mermaid sequence diagramme", icon: "⇆", run: replaceWith(tk("```mermaid\nsequenceDiagram\n  participant U as Utilisateur\n  participant S as Système\n  U->>S: §Requête\n  S-->>U: Réponse\n```\n")) },
  { label: tk("Diagramme : Gantt"), keywords: "mermaid gantt planning calendrier", icon: "▤", run: replaceWith(tk("```mermaid\ngantt\n  title §Planning\n  dateFormat YYYY-MM-DD\n  section Phase 1\n  Tâche A :a1, 2026-01-01, 30d\n  Tâche B :after a1, 20d\n```\n")) },
  { label: tk("Diagramme : carte mentale"), keywords: "mermaid mindmap carte mentale", icon: "✺", run: replaceWith(tk("```mermaid\nmindmap\n  root((§Sujet))\n    Idée 1\n    Idée 2\n      Détail\n```\n")) },
  { label: tk("Diagramme : classes"), keywords: "mermaid class uml", icon: "⊞", run: replaceWith(tk("```mermaid\nclassDiagram\n  class §Transaction {\n    +montant\n    +date\n  }\n```\n")) },
  { label: tk("Formule (bloc)"), keywords: "math latex formule equation", icon: "∑", run: replaceWith("$$\n§\n$$\n") },
  { label: tk("Formule (en ligne)"), keywords: "math latex inline", icon: "x²", run: replaceWith("$§$") },
  { label: tk("Séparateur"), keywords: "hr separateur ligne", icon: "—", run: replaceWith("---\n") },
  { label: tk("Date du jour"), keywords: "date aujourd'hui", icon: "📅", run: (v, f, to) => replaceWith(today())(v, f, to) },
  { label: tk("Lien interne"), keywords: "lien wiki note", icon: "[[", run: (v, f, to) => { replaceWith("[[§]]")(v, f, to); startCompletion(v); } },
  { label: tk("Citation Zotero"), keywords: "zotero cite citation reference biblio", icon: "@", run: (v, f, to) => { replaceWith("[@§]")(v, f, to); setTimeout(() => startCompletion(v), 10); } },
  { label: tk("Image"), keywords: "image photo", icon: "🖼", run: replaceWith(tk("![§](chemin/vers/image.png)")) },
  { label: tk("Insérer un modèle…"), keywords: "modele template", icon: "⎘", run: (v, f, to) => { v.dispatch({ changes: { from: f, to } }); app.palette = "templates"; } },
];

function slashSource(ctx: CompletionContext): CompletionResult | null {
  const m = ctx.matchBefore(/(?:^|\s)\/[\p{L}\p{N} ]{0,24}$/u);
  if (!m) return null;
  const slashAt = m.from + m.text.indexOf("/");
  // Not inside a code block / path
  const q = ctx.state.sliceDoc(slashAt + 1, ctx.pos);
  if (q.includes("  ")) return null;
  const options: Completion[] = SLASH.map((s) => ({
    label: t(s.label),
    detail: s.icon,
    type: "slash",
    boost: fuzzyScore(q, `${t(s.label)} ${s.label} ${s.keywords}`) / 100,
    apply: (view: EditorView, _c: Completion, _from: number, to: number) => s.run(view, slashAt, to),
  })).filter((o) => (o.boost ?? 0) >= 0 || !q);
  if (!options.length) return null;
  return { from: slashAt + 1, options, filter: false };
}

export const eloriaAutocomplete = autocompletion({
  override: [slashSource, wikiSource, citeSource, tagSource],
  icons: false,
  closeOnBlur: true,
  activateOnTyping: true,
  maxRenderedOptions: 60,
  addToOptions: [
    {
      render: (c) => {
        const s = document.createElement("span");
        s.className = `cm-ac-kind cm-ac-${c.type ?? "x"}`;
        return s;
      },
      position: 20,
    },
  ],
});
