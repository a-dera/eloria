// Floating formatting bar: shows above a text selection.

import { StateField, type EditorState } from "@codemirror/state";
import { EditorView, ViewPlugin, showTooltip, type Tooltip, type TooltipView } from "@codemirror/view";
import { startCompletion } from "@codemirror/autocomplete";
import { t, tk } from "../i18n.svelte";
import { insertLink, setHeading, toggleLinePrefix, toggleWrap } from "./commands";

interface Button {
  id: string;
  /** Tooltip text (French key) */
  title: string;
  keys?: string;
  /** Button content: short text or SVG */
  face: string;
  cls?: string;
  run(view: EditorView): void;
  active?(state: EditorState): boolean;
}

const svg = (d: string) =>
  `<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="${d}"/></svg>`;

/** Is the selection already wrapped in `mark` (or does it contain it at both ends)? */
function wrapped(state: EditorState, mark: string): boolean {
  const r = state.selection.main;
  const inner = state.sliceDoc(r.from, r.to);
  if (inner.length >= mark.length * 2 && inner.startsWith(mark) && inner.endsWith(mark)) return true;
  return state.sliceDoc(r.from - mark.length, r.from) === mark && state.sliceDoc(r.to, r.to + mark.length) === mark;
}

function headingLevel(state: EditorState): number {
  const line = state.doc.lineAt(state.selection.main.from);
  return /^(#{1,6})\s/.exec(line.text)?.[1].length ?? 0;
}

function linePrefixed(state: EditorState, re: RegExp): boolean {
  return re.test(state.doc.lineAt(state.selection.main.from).text.trimStart());
}

const BUTTONS: (Button | "|")[] = [
  { id: "bold", title: tk("Gras"), keys: "Ctrl+B", face: "<b>B</b>", run: (v) => toggleWrap(v, "**"), active: (s) => wrapped(s, "**") },
  {
    id: "italic",
    title: tk("Italique"),
    keys: "Ctrl+I",
    face: "<i>I</i>",
    cls: "serif",
    run: (v) => toggleWrap(v, "*"),
    active: (s) => wrapped(s, "*") && !wrapped(s, "**"),
  },
  { id: "strike", title: tk("Barré"), keys: "Ctrl+Maj+X", face: "<s>S</s>", run: (v) => toggleWrap(v, "~~"), active: (s) => wrapped(s, "~~") },
  {
    id: "highlight",
    title: tk("Surligner"),
    keys: "Ctrl+Maj+H",
    face: '<span class="cm-fb-mark">A</span>',
    run: (v) => toggleWrap(v, "=="),
    active: (s) => wrapped(s, "=="),
  },
  { id: "code", title: tk("Code"), keys: "Ctrl+E", face: svg("M8 7l-5 5 5 5M16 7l5 5-5 5"), run: (v) => toggleWrap(v, "`"), active: (s) => wrapped(s, "`") && !wrapped(s, "```") },
  "|",
  {
    id: "link",
    title: tk("Lien"),
    keys: "Ctrl+K",
    face: svg("M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"),
    run: (v) => insertLink(v),
  },
  {
    id: "wikilink",
    title: tk("Lien vers une note"),
    face: '<span class="cm-fb-mono">[[ ]]</span>',
    run: (v) => {
      const r = v.state.selection.main;
      const text = v.state.sliceDoc(r.from, r.to);
      v.dispatch({ changes: { from: r.from, to: r.to, insert: `[[${text}]]` }, selection: { anchor: r.from + 2, head: r.from + 2 + text.length } });
      startCompletion(v);
    },
    active: (s) => wrapped(s, "[[") || (s.sliceDoc(s.selection.main.from - 2, s.selection.main.from) === "[[" && s.sliceDoc(s.selection.main.to, s.selection.main.to + 2) === "]]"),
  },
  "|",
  { id: "h1", title: tk("Titre 1"), keys: "Ctrl+Alt+1", face: "H1", cls: "small", run: (v) => setHeading(v, 1), active: (s) => headingLevel(s) === 1 },
  { id: "h2", title: tk("Titre 2"), keys: "Ctrl+Alt+2", face: "H2", cls: "small", run: (v) => setHeading(v, 2), active: (s) => headingLevel(s) === 2 },
  { id: "h3", title: tk("Titre 3"), keys: "Ctrl+Alt+3", face: "H3", cls: "small", run: (v) => setHeading(v, 3), active: (s) => headingLevel(s) === 3 },
  {
    id: "quote",
    title: tk("Citation"),
    keys: "Ctrl+Maj+.",
    face: svg("M7 7h4v4c0 3-2 5-4 5M15 7h4v4c0 3-2 5-4 5"),
    run: (v) => toggleLinePrefix(v, "quote"),
    active: (s) => linePrefixed(s, /^>/),
  },
  {
    id: "bullet",
    title: tk("Liste à puces"),
    keys: "Ctrl+Maj+8",
    face: svg("M9 6h11M9 12h11M9 18h11M4.5 6h.01M4.5 12h.01M4.5 18h.01"),
    run: (v) => toggleLinePrefix(v, "bullet"),
    active: (s) => linePrefixed(s, /^[-*+] (?!\[)/),
  },
  {
    id: "task",
    title: tk("Case à cocher"),
    keys: "Ctrl+L",
    face: svg("M4 4h16v16H4zM8 12l3 3 5-6"),
    run: (v) => toggleLinePrefix(v, "task"),
    active: (s) => linePrefixed(s, /^[-*+] \[[ xX]\]/),
  },
  "|",
  {
    id: "assist",
    title: tk("Assistant de rédaction (IA) : reformuler, corriger, résumer…"),
    keys: "Ctrl+Maj+J",
    face: svg("M13 2 4 14h7l-1 8 9-12h-7z"),
    cls: "assist",
    // The application opens the assistant on the selection (event caught by the global state).
    run: () => window.dispatchEvent(new CustomEvent("eloria:assist")),
  },
];

function barView(view: EditorView): TooltipView {
  const dom = document.createElement("div");
  dom.className = "cm-format-bar";
  dom.setAttribute("role", "toolbar");
  dom.setAttribute("aria-label", t("Mise en forme"));
  const els = new Map<string, HTMLButtonElement>();
  for (const b of BUTTONS) {
    if (b === "|") {
      const sep = document.createElement("span");
      sep.className = "cm-fb-sep";
      dom.appendChild(sep);
      continue;
    }
    const el = document.createElement("button");
    el.type = "button";
    el.className = `cm-fb-btn${b.cls ? ` ${b.cls}` : ""}`;
    el.dataset.cmd = b.id;
    el.innerHTML = b.face;
    const label = t(b.title);
    el.title = b.keys ? `${label} (${b.keys.replace("Maj", t("Maj"))})` : label;
    el.setAttribute("aria-label", label);
    // Keep the selection and the focus in the editor.
    el.addEventListener("mousedown", (e) => e.preventDefault());
    el.addEventListener("click", (e) => {
      e.preventDefault();
      b.run(view);
      view.focus();
    });
    els.set(b.id, el);
    dom.appendChild(el);
  }
  const refresh = (state: EditorState) => {
    for (const b of BUTTONS) {
      if (b === "|" || !b.active) continue;
      els.get(b.id)?.classList.toggle("active", b.active(state));
    }
  };
  refresh(view.state);
  return { dom, update: (u) => refresh(u.state) };
}

function barTooltip(state: EditorState): Tooltip | null {
  const r = state.selection.main;
  if (r.empty || state.selection.ranges.length > 1) return null;
  // Not on a selection made only of whitespace.
  if (!state.sliceDoc(r.from, r.to).trim()) return null;
  return { pos: r.from, end: r.to, above: true, strictSide: false, arrow: false, create: barView };
}

const barField = StateField.define<Tooltip | null>({
  create: barTooltip,
  update(value, tr) {
    if (!tr.docChanged && !tr.selection) return value;
    const next = barTooltip(tr.state);
    // Same range: keep the bar (no flicker), it updates itself.
    if (value && next && value.pos === next.pos && value.end === next.end) return value;
    return next;
  },
  provide: (f) => showTooltip.from(f),
});

/** Hides the bar while the selection is being stretched with the mouse. */
const dragTracker = ViewPlugin.fromClass(
  class {
    private up = () => this.view.dom.classList.remove("cm-selecting");
    constructor(readonly view: EditorView) {
      view.dom.addEventListener("mousedown", this.down);
      document.addEventListener("mouseup", this.up);
    }
    private down = (e: MouseEvent) => {
      if (e.button === 0 && !(e.target as HTMLElement).closest(".cm-format-bar")) this.view.dom.classList.add("cm-selecting");
    };
    destroy() {
      this.view.dom.removeEventListener("mousedown", this.down);
      document.removeEventListener("mouseup", this.up);
    }
  },
);

export const formatBar = [barField, dragTracker];
