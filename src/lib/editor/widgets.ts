// Live preview widgets: checkboxes, bullets, links, images, formulas, Mermaid, callouts.

import { EditorView, WidgetType } from "@codemirror/view";
import { Facet } from "@codemirror/state";
import katex from "katex";
import { renderMermaid } from "./mermaid";
import { mount, unmount } from "svelte";
import ZoteroItemView from "../../components/ZoteroItemView.svelte";
import { t, tk, tn } from "../i18n.svelte";

export interface LinkHandlers {
  openWiki(target: string, e: MouseEvent): void;
  openUrl(url: string, e: MouseEvent): void;
  openTag(tag: string): void;
  openCitation(key: string, e: MouseEvent): void;
  /** Displayable URL of an image (vault path or remote URL) */
  imageSrc(target: string): Promise<string | null>;
  /** Does the target note exist? */
  exists(target: string): boolean;
  /** Path of the edited note. */
  notePath(): string;
  /** Content of a `![[Note#Section]]` embed (safe HTML), or null if not found. */
  embed(target: string): Promise<{ path: string; title: string; html: string } | null>;
}

export const linkHandlers = Facet.define<LinkHandlers, LinkHandlers | null>({
  combine: (v) => v[0] ?? null,
});

export class CheckboxWidget extends WidgetType {
  constructor(readonly checked: boolean, readonly pos: number) {
    super();
  }
  eq(o: CheckboxWidget) {
    return o.checked === this.checked && o.pos === this.pos;
  }
  toDOM(view: EditorView) {
    const box = document.createElement("input");
    box.type = "checkbox";
    box.className = "cm-task-box";
    box.checked = this.checked;
    box.addEventListener("mousedown", (e) => e.preventDefault());
    box.addEventListener("click", (e) => {
      e.preventDefault();
      const text = view.state.sliceDoc(this.pos, this.pos + 3);
      const next = /\[[xX]\]/.test(text) ? "[ ]" : "[x]";
      view.dispatch({ changes: { from: this.pos, to: this.pos + 3, insert: next } });
    });
    return box;
  }
  ignoreEvent() {
    return true;
  }
}

export class BulletWidget extends WidgetType {
  constructor(readonly depth: number) {
    super();
  }
  eq(o: BulletWidget) {
    return o.depth === this.depth;
  }
  toDOM() {
    const s = document.createElement("span");
    s.className = "cm-bullet";
    s.textContent = ["•", "◦", "▪"][this.depth % 3];
    return s;
  }
}

export class HrWidget extends WidgetType {
  eq() {
    return true;
  }
  toDOM() {
    const s = document.createElement("span");
    s.className = "cm-hr";
    return s;
  }
}

export class WikiLinkWidget extends WidgetType {
  constructor(readonly target: string, readonly label: string, readonly resolved: boolean) {
    super();
  }
  eq(o: WikiLinkWidget) {
    return o.target === this.target && o.label === this.label && o.resolved === this.resolved;
  }
  toDOM(view: EditorView) {
    const a = document.createElement("span");
    a.className = "cm-wikilink" + (this.resolved ? "" : " cm-wikilink-missing");
    a.textContent = this.label;
    a.title = this.resolved ? this.target : t("{target} (n'existe pas encore : clic pour créer)", { target: this.target });
    a.addEventListener("mousedown", (e) => {
      if (e.button !== 0) return;
      e.preventDefault();
      view.state.facet(linkHandlers)?.openWiki(this.target, e);
    });
    return a;
  }
  ignoreEvent() {
    return true;
  }
}

export class LinkTextWidget extends WidgetType {
  constructor(readonly text: string, readonly url: string) {
    super();
  }
  eq(o: LinkTextWidget) {
    return o.text === this.text && o.url === this.url;
  }
  toDOM(view: EditorView) {
    const a = document.createElement("span");
    a.className = "cm-ext-link";
    a.textContent = this.text;
    a.title = this.url;
    a.addEventListener("mousedown", (e) => {
      if (e.button !== 0) return;
      e.preventDefault();
      view.state.facet(linkHandlers)?.openUrl(this.url, e);
    });
    return a;
  }
  ignoreEvent() {
    return true;
  }
}

const imgCache = new Map<string, string>();

export class ImageWidget extends WidgetType {
  constructor(readonly target: string, readonly alt: string, readonly width: number | null, readonly from: number) {
    super();
  }
  eq(o: ImageWidget) {
    return o.target === this.target && o.alt === this.alt && o.width === this.width;
  }
  get estimatedHeight() {
    return 240;
  }
  toDOM(view: EditorView) {
    const wrap = document.createElement("span");
    wrap.className = "cm-image";
    const img = document.createElement("img");
    img.alt = this.alt;
    if (this.width) img.style.width = `${this.width}px`;
    img.addEventListener("mousedown", (e) => {
      e.preventDefault();
      view.dispatch({ selection: { anchor: this.from } });
      view.focus();
    });
    const cached = imgCache.get(this.target);
    const set = (src: string | null) => {
      if (!src) {
        wrap.classList.add("cm-image-missing");
        wrap.textContent = t("Image introuvable : {target}", { target: this.target });
        return;
      }
      img.src = src;
      img.onload = () => view.requestMeasure();
    };
    if (cached) set(cached);
    else {
      view.state
        .facet(linkHandlers)
        ?.imageSrc(this.target)
        .then((src) => {
          if (src) imgCache.set(this.target, src);
          set(src);
        });
    }
    wrap.appendChild(img);
    return wrap;
  }
  ignoreEvent() {
    return true;
  }
}

export class MathWidget extends WidgetType {
  constructor(readonly tex: string, readonly display: boolean, readonly from: number) {
    super();
  }
  eq(o: MathWidget) {
    return o.tex === this.tex && o.display === this.display;
  }
  toDOM(view: EditorView) {
    const el = document.createElement(this.display ? "div" : "span");
    el.className = this.display ? "cm-math-block" : "cm-math-inline";
    try {
      katex.render(this.tex, el, { displayMode: this.display, throwOnError: false, output: "html" });
    } catch (e) {
      el.textContent = this.tex;
      el.classList.add("cm-math-error");
    }
    el.addEventListener("mousedown", (e) => {
      e.preventDefault();
      view.dispatch({ selection: { anchor: this.from + (this.display ? 3 : 1) } });
      view.focus();
    });
    return el;
  }
  ignoreEvent() {
    return true;
  }
}

export class MermaidWidget extends WidgetType {
  constructor(readonly code: string, readonly from: number, readonly theme: string) {
    super();
  }
  eq(o: MermaidWidget) {
    return o.code === this.code && o.theme === this.theme;
  }
  get estimatedHeight() {
    return 260;
  }
  toDOM(view: EditorView) {
    const el = document.createElement("div");
    el.className = "cm-mermaid";
    el.innerHTML = `<div class="cm-mermaid-loading">${t("Rendu du diagramme…")}</div>`;
    renderMermaid(this.code, this.theme).then(
      (svg) => {
        el.innerHTML = svg;
        view.requestMeasure();
      },
      (err) => {
        el.innerHTML = "";
        const pre = document.createElement("pre");
        pre.className = "cm-mermaid-error";
        pre.textContent = `${t("Diagramme Mermaid invalide")}\n${String(err?.message ?? err).slice(0, 400)}`;
        el.appendChild(pre);
        view.requestMeasure();
      },
    );
    el.addEventListener("mousedown", (e) => {
      e.preventDefault();
      view.dispatch({ selection: { anchor: this.from + 4 } });
      view.focus();
    });
    return el;
  }
  ignoreEvent() {
    return true;
  }
}

const CALLOUT_ICONS: Record<string, string> = {
  note: "✎",
  info: "ℹ",
  astuce: "💡",
  tip: "💡",
  important: "❗",
  attention: "⚠",
  warning: "⚠",
  danger: "⛔",
  question: "?",
  citation: "❝",
  quote: "❝",
  abstract: "≡",
  resume: "≡",
  résumé: "≡",
  exemple: "◆",
  example: "◆",
  todo: "☐",
  succes: "✓",
  success: "✓",
  definition: "§",
  définition: "§",
};

export const CALLOUT_LABELS: Record<string, string> = {
  note: tk("Note"),
  info: tk("Info"),
  astuce: tk("Astuce"),
  tip: tk("Astuce"),
  important: tk("Important"),
  attention: tk("Attention"),
  warning: tk("Attention"),
  danger: tk("Danger"),
  question: tk("Question"),
  citation: tk("Citation"),
  quote: tk("Citation"),
  abstract: tk("Résumé"),
  resume: tk("Résumé"),
  exemple: tk("Exemple"),
  example: tk("Exemple"),
  todo: tk("À faire"),
  succes: tk("Succès"),
  success: tk("Succès"),
  definition: tk("Définition"),
  souligné: tk("Souligné"),
  image: tk("Image"),
  dessin: tk("Dessin"),
};

export class CalloutHeaderWidget extends WidgetType {
  constructor(readonly kind: string, readonly fold: string, readonly hasTitle: boolean, readonly lineEnd: number) {
    super();
  }
  eq(o: CalloutHeaderWidget) {
    return o.kind === this.kind && o.fold === this.fold && o.hasTitle === this.hasTitle && o.lineEnd === this.lineEnd;
  }
  toDOM(view: EditorView) {
    const s = document.createElement("span");
    s.className = "cm-callout-label";
    const k = this.kind.toLowerCase();
    const icon = CALLOUT_ICONS[k] ?? "✎";
    const label = CALLOUT_LABELS[k] ? t(CALLOUT_LABELS[k]) : this.kind.charAt(0).toUpperCase() + this.kind.slice(1);
    s.textContent = `${this.fold === "-" ? "▸ " : this.fold === "+" ? "▾ " : ""}${icon} ${this.hasTitle ? "" : label}`;
    s.addEventListener("mousedown", (e) => {
      e.preventDefault();
      view.dispatch({ selection: { anchor: this.lineEnd } });
      view.focus();
    });
    return s;
  }
  ignoreEvent() {
    return true;
  }
}

export class FoldedWidget extends WidgetType {
  constructor(readonly lines: number) {
    super();
  }
  eq(o: FoldedWidget) {
    return o.lines === this.lines;
  }
  toDOM() {
    const d = document.createElement("div");
    d.className = "cm-folded";
    d.textContent = tn(this.lines, "… {n} ligne repliée", "… {n} lignes repliées");
    return d;
  }
}

export class PropertiesWidget extends WidgetType {
  constructor(readonly yaml: string, readonly to: number) {
    super();
  }
  eq(o: PropertiesWidget) {
    return o.yaml === this.yaml;
  }
  toDOM(view: EditorView) {
    const d = document.createElement("div");
    d.className = "cm-props";
    const rows = this.yaml
      .split("\n")
      .slice(1, -1)
      .filter((l) => /^[^\s#][^:]*:/.test(l));
    if (!rows.length) {
      d.textContent = t("Propriétés");
    }
    for (const line of rows.slice(0, 12)) {
      const i = line.indexOf(":");
      const row = document.createElement("div");
      row.className = "cm-prop";
      const k = document.createElement("span");
      k.className = "cm-prop-key";
      k.textContent = line.slice(0, i);
      const v = document.createElement("span");
      v.className = "cm-prop-val";
      v.textContent = line.slice(i + 1).trim().replace(/^\[|\]$/g, "").replace(/^"|"$/g, "") || "—";
      row.append(k, v);
      d.appendChild(row);
    }
    if (rows.length > 12) {
      const more = document.createElement("div");
      more.className = "cm-prop-more";
      more.textContent = tn(rows.length - 12, "+ {n} propriété", "+ {n} propriétés");
      d.appendChild(more);
    }
    d.addEventListener("mousedown", (e) => {
      e.preventDefault();
      view.dispatch({ selection: { anchor: 4 } });
      view.focus();
    });
    return d;
  }
  ignoreEvent() {
    return true;
  }
}

/**
 * At the top of a reading note linked to an item (`zotero:` property): the item card,
 * as in the Zotero panel (abstract, tags, annotations, PDF, record), instead of the
 * raw property table. The note follows below.
 */
/** `![[Note]]` / `![[Note#Section]]` embed: the content of the note, read-only. */
export class EmbedWidget extends WidgetType {
  constructor(readonly target: string) {
    super();
  }
  eq(o: EmbedWidget) {
    return o.target === this.target;
  }
  get estimatedHeight() {
    return 120;
  }
  toDOM(view: EditorView) {
    const box = document.createElement("div");
    box.className = "cm-embed";
    const head = document.createElement("div");
    head.className = "cm-embed-head";
    const title = document.createElement("span");
    title.textContent = `↳ ${this.target}`;
    const open = document.createElement("button");
    open.className = "cm-embed-open";
    open.textContent = t("Ouvrir");
    head.append(title, open);
    const body = document.createElement("div");
    body.className = "cm-embed-body";
    body.textContent = t("Chargement…");
    box.append(head, body);
    const h = view.state.facet(linkHandlers);
    open.addEventListener("mousedown", (e) => e.preventDefault());
    open.addEventListener("click", (e) => h?.openWiki(this.target, e));
    h?.embed(this.target)
      .then((r) => {
        if (!r) {
          box.classList.add("missing");
          body.textContent = t("Note ou section introuvable : {target}", { target: this.target });
          return;
        }
        const [, heading] = this.target.split("#");
        title.textContent = `↳ ${r.title}${heading ? ` › ${heading}` : ""}`;
        body.innerHTML = r.html; // HTML produced by the core: the note's raw HTML neutralised
        view.requestMeasure();
      })
      .catch(() => (body.textContent = t("Note ou section introuvable : {target}", { target: this.target })));
    return box;
  }
  ignoreEvent() {
    return true;
  }
}

export class DocumentWidget extends WidgetType {
  private app: ReturnType<typeof mount> | null = null;
  constructor(readonly yaml: string) {
    super();
  }
  eq(o: DocumentWidget) {
    return o.yaml === this.yaml;
  }
  get estimatedHeight() {
    return 520;
  }
  toDOM(view: EditorView) {
    const d = document.createElement("div");
    d.className = "cm-document";
    this.app = mount(ZoteroItemView, {
      target: d,
      props: {
        sourcePath: view.state.facet(linkHandlers)?.notePath() ?? "",
        inNote: true,
        onEditProps: () => {
          view.dispatch({ selection: { anchor: 4 } });
          view.focus();
        },
      },
    });
    // The height changes once the card has loaded the item.
    new ResizeObserver(() => view.requestMeasure()).observe(d);
    return d;
  }
  destroy() {
    if (this.app) unmount(this.app);
    this.app = null;
  }
  ignoreEvent() {
    return true;
  }
}

// ------------------------------------------------------------------ tableaux

function inlineMd(src: string): string {
  let s = src.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  s = s.replace(/`([^`]+)`/g, "<code>$1</code>");
  s = s.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
  s = s.replace(/(^|[^*])\*([^*]+)\*/g, "$1<em>$2</em>");
  s = s.replace(/==([^=]+)==/g, "<mark>$1</mark>");
  s = s.replace(/~~([^~]+)~~/g, "<del>$1</del>");
  s = s.replace(/\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g, (_m, t, a) => `<span class="cm-wikilink">${a ?? t}</span>`);
  s = s.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<span class="cm-ext-link">$1</span>');
  s = s.replace(/\[(@[^\]]+)\]/g, '<span class="cm-citation">[$1]</span>');
  return s;
}

function splitRow(line: string): string[] {
  const t = line.trim().replace(/^\|/, "").replace(/\|$/, "");
  return t.split(/(?<!\\)\|/).map((c) => c.trim());
}

/** Markdown table rebuilt from the cells (alignments kept). */
export function serializeTable(head: string[][], body: string[][], aligns: string[]): string {
  const n = Math.max(...head.map((r) => r.length), ...body.map((r) => r.length), aligns.length, 1);
  const cell = (c: string | undefined) => (c ?? "").replace(/\n/g, " ").replace(/(?<!\\)\|/g, "\\|").trim();
  const row = (r: string[]) => `| ${Array.from({ length: n }, (_, i) => cell(r[i])).join(" | ")} |`;
  const delim = Array.from({ length: n }, (_, i) => {
    const a = aligns[i] ?? "left";
    return a === "center" ? ":---:" : a === "right" ? "---:" : "---";
  });
  return [...head.map(row), `| ${delim.join(" | ")} |`, ...body.map(row)].join("\n");
}

/** Cell to reopen after the table is rewritten (Tab, new row…). */
let pendingCell: { from: number; row: number; col: number } | null = null;

export class TableWidget extends WidgetType {
  constructor(readonly source: string, readonly from: number) {
    super();
  }
  eq(o: TableWidget) {
    return o.source === this.source && o.from === this.from;
  }
  toDOM(view: EditorView) {
    const lines = this.source.split("\n").filter((l) => l.trim());
    const delim = lines.findIndex((l) => /^\s*\|?\s*:?-{2,}/.test(l));
    const aligns = delim >= 0 ? splitRow(lines[delim]).map((c) => (c.startsWith(":") && c.endsWith(":") ? "center" : c.endsWith(":") ? "right" : "left")) : [];
    const head = (delim >= 0 ? lines.slice(0, delim) : lines.slice(0, 1)).map(splitRow);
    const body = (delim >= 0 ? lines.slice(delim + 1) : lines.slice(1)).map(splitRow);
    const cols = Math.max(...head.map((r) => r.length), ...body.map((r) => r.length), 1);
    // Full grid: row r (header included), column c.
    const grid = [...head, ...body].map((r) => Array.from({ length: cols }, (_, i) => r[i] ?? ""));
    const nHead = head.length;

    const wrap = document.createElement("div");
    wrap.className = "cm-table-widget";
    const table = document.createElement("table");
    const bar = document.createElement("div");
    const cellsEl: HTMLElement[][] = [];
    const makeRow = (ri: number) => {
      const tr = document.createElement("tr");
      cellsEl[ri] = [];
      grid[ri].forEach((cell, ci) => {
        const td = document.createElement(ri < nHead ? "th" : "td");
        td.innerHTML = inlineMd(cell);
        td.dataset.row = String(ri);
        td.dataset.col = String(ci);
        if (aligns[ci]) td.style.textAlign = aligns[ci];
        tr.appendChild(td);
        cellsEl[ri][ci] = td;
      });
      (ri < nHead ? (table.tHead ?? table.createTHead()) : (table.tBodies[0] ?? table.createTBody())).appendChild(tr);
    };
    grid.forEach((_, ri) => makeRow(ri));
    wrap.appendChild(table);

    // The Markdown is only rewritten when leaving the table (or through the toolbar):
    // navigation between cells stays in this widget, no keystroke is lost.
    const write = (g: string[][], next?: { row: number; col: number }) => {
      const text = serializeTable(g.slice(0, nHead), g.slice(nHead), aligns);
      if (text === this.source) {
        if (next) open(next.row, next.col);
        return;
      }
      if (next) pendingCell = { from: this.from, ...next };
      view.dispatch({ changes: { from: this.from, to: this.from + this.source.length, insert: text } });
    };

    let editing: { row: number; col: number; el: HTMLElement } | null = null;
    /** Keeps the text of the open cell in the grid (without touching the document). */
    const commit = () => {
      if (!editing) return;
      const { row, col, el } = editing;
      editing = null;
      grid[row][col] = (el.textContent ?? "").replace(/\n/g, " ");
      el.contentEditable = "false";
      el.innerHTML = inlineMd(grid[row][col]);
    };
    const cancel = () => {
      if (!editing) return;
      editing.el.contentEditable = "false";
      editing.el.innerHTML = inlineMd(grid[editing.row][editing.col]);
      editing = null;
    };
    const open = (row: number, col: number) => {
      const el = cellsEl[row]?.[col];
      if (!el) return;
      editing = { row, col, el };
      el.textContent = grid[row][col];
      el.contentEditable = "true";
      el.focus();
      const sel = window.getSelection();
      const range = document.createRange();
      // Content selected, as in a spreadsheet: typing replaces, arrows to adjust.
      range.selectNodeContents(el);
      sel?.removeAllRanges();
      sel?.addRange(range);
    };
    const move = (row: number, col: number, dc: number) => {
      let r = row;
      let c = col + dc;
      if (c >= cols) {
        c = 0;
        r++;
      } else if (c < 0) {
        c = cols - 1;
        r--;
      }
      return { row: r, col: c };
    };

    table.addEventListener("mousedown", (e) => {
      const td = (e.target as HTMLElement).closest("td,th") as HTMLElement | null;
      if (!td || td.isContentEditable) return;
      e.preventDefault();
      commit();
      open(Number(td.dataset.row), Number(td.dataset.col));
    });
    table.addEventListener("keydown", (e) => {
      if (!editing) return;
      const { row, col } = editing;
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "a") {
        // Select all: the cell only, not the whole note.
        e.preventDefault();
        const r = document.createRange();
        r.selectNodeContents(editing.el);
        window.getSelection()?.removeAllRanges();
        window.getSelection()?.addRange(r);
      } else if (e.key === "Tab") {
        e.preventDefault();
        commit();
        const n = move(row, col, e.shiftKey ? -1 : 1);
        if (n.row >= grid.length) {
          // Tab on the last cell: new row, added in place.
          grid.push(Array(cols).fill(""));
          makeRow(grid.length - 1);
        }
        if (n.row >= 0) open(n.row, n.col);
      } else if (e.key === "Enter") {
        e.preventDefault();
        commit();
        if (row + 1 < grid.length) open(row + 1, col);
        else {
          write(grid);
          view.focus();
        }
      } else if (e.key === "Escape") {
        e.preventDefault();
        cancel();
        write(grid);
        view.focus();
      }
      e.stopPropagation();
    });
    // Leaving the table writes the Markdown. Checked after the focus change: moving
    // from one cell to another (Tab, click) also makes the first one lose the focus.
    table.addEventListener("focusout", () => {
      setTimeout(() => {
        const a = document.activeElement;
        if (!wrap.isConnected || (a && wrap.contains(a))) return;
        commit();
        write(grid);
      }, 0);
    });

    // Toolbar: rows, columns, Markdown source.
    bar.className = "cm-table-bar";
    const btn = (label: string, run: () => void) => {
      const b = document.createElement("button");
      b.textContent = label;
      b.addEventListener("mousedown", (e) => e.preventDefault());
      b.addEventListener("click", (e) => {
        e.preventDefault();
        run();
      });
      bar.appendChild(b);
    };
    const target = () => editing ?? { row: grid.length - 1, col: cols - 1 };
    btn(t("+ ligne"), () => {
      const { row } = target();
      commit();
      const g = grid.map((r) => [...r]);
      const at = Math.max(row + 1, nHead);
      g.splice(at, 0, Array(cols).fill(""));
      write(g, { row: at, col: 0 });
    });
    btn(t("+ colonne"), () => {
      const { col } = target();
      commit();
      const g = grid.map((r) => {
        const c = [...r];
        c.splice(col + 1, 0, "");
        return c;
      });
      aligns.splice(col + 1, 0, "left");
      write(g, { row: 0, col: col + 1 });
    });
    btn(t("− ligne"), () => {
      const { row } = target();
      if (row < nHead || grid.length - nHead <= 1) return;
      editing = null;
      const g = grid.map((r) => [...r]);
      g.splice(row, 1);
      write(g);
    });
    btn(t("− colonne"), () => {
      const { col } = target();
      if (cols <= 1) return;
      editing = null;
      const g = grid.map((r) => r.filter((_, i) => i !== col));
      aligns.splice(col, 1);
      write(g);
    });
    btn(t("Source"), () => {
      editing = null;
      view.dispatch({ selection: { anchor: this.from + 2 } });
      view.focus();
    });
    wrap.appendChild(bar);

    if (pendingCell && pendingCell.from === this.from) {
      const p = pendingCell;
      pendingCell = null;
      requestAnimationFrame(() => open(p.row, p.col));
    }
    return wrap;
  }
  ignoreEvent() {
    return true;
  }
}
