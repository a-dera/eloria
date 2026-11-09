// Live preview: Markdown syntax disappears on lines where the cursor
// is not, and the rendering (headings, links, checkboxes, images, formulas, diagrams) shows.
//
// Two sources of decorations:
// - a ViewPlugin for everything that fits in a line (fast, limited to the visible area);
// - a StateField for block replacements (front matter, Mermaid, $$, folded callouts),
//   because CodeMirror forbids block decorations coming from a plugin.

import { syntaxTree } from "@codemirror/language";
import { EditorState, StateField, type Range } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, type DecorationSet, type ViewUpdate } from "@codemirror/view";
import type { SyntaxNode } from "@lezer/common";
import {
  BulletWidget,
  CalloutHeaderWidget,
  CheckboxWidget,
  DocumentWidget,
  EmbedWidget,
  HrWidget,
  ImageWidget,
  LinkTextWidget,
  MathWidget,
  MermaidWidget,
  PropertiesWidget,
  TableWidget,
  WikiLinkWidget,
  linkHandlers,
} from "./widgets";

const hide = Decoration.replace({});
const IMAGE_EXT = /\.(png|jpe?g|gif|svg|webp|bmp)$/i;
export const CALLOUT_RE = /^>\s*\[!([\wÀ-ſ-]+)(?:\|([\w-]+))?\]([+-]?)\s?(.*)$/;
const TAG_RE = /(^|[\s(\[,;])#([\p{L}\p{N}_][\p{L}\p{N}_/\-]*)/gu;
const CITE_RE = /\[[^\[\]\n]*@[^\[\]\n]*\]/g;
const KEY_RE = /@([\p{L}\p{N}_][\p{L}\p{N}_:.#$%&+?<>~/\-]*[\p{L}\p{N}_])/gu;

/** Adds a mark only if the range is not empty (CodeMirror refuses empty marks). */
function pushMark(decos: Range<Decoration>[], spec: Parameters<typeof Decoration.mark>[0], from: number, to: number) {
  if (from < to) decos.push(Decoration.mark(spec).range(from, to));
}

function lineClass(cls: string) {
  return Decoration.line({ class: cls });
}

/** Numbers of the lines touched by the selection (if the editor has the focus). */
function activeLines(state: EditorState, focused: boolean): Set<number> {
  const s = new Set<number>();
  if (!focused) return s;
  for (const r of state.selection.ranges) {
    const a = state.doc.lineAt(r.from).number;
    const b = state.doc.lineAt(r.to).number;
    for (let i = a; i <= b; i++) s.add(i);
  }
  return s;
}

function selectionTouches(state: EditorState, from: number, to: number): boolean {
  return state.selection.ranges.some((r) => r.from <= to && r.to >= from);
}

function child(node: SyntaxNode, name: string): SyntaxNode | null {
  for (let c = node.firstChild; c; c = c.nextSibling) if (c.name === name) return c;
  return null;
}

function children(node: SyntaxNode, name: string): SyntaxNode[] {
  const out: SyntaxNode[] = [];
  for (let c = node.firstChild; c; c = c.nextSibling) if (c.name === name) out.push(c);
  return out;
}

function listDepth(node: SyntaxNode): number {
  let d = -1;
  for (let p: SyntaxNode | null = node; p; p = p.parent) if (p.name === "BulletList" || p.name === "OrderedList") d++;
  return Math.max(0, d);
}

export function parseWikiTarget(raw: string): { target: string; heading: string; alias: string } {
  let target = raw;
  let alias = "";
  const pipe = target.indexOf("|");
  if (pipe >= 0) {
    alias = target.slice(pipe + 1).trim();
    target = target.slice(0, pipe);
  }
  let heading = "";
  const hash = target.indexOf("#");
  if (hash >= 0) {
    heading = target.slice(hash + 1).trim();
    target = target.slice(0, hash);
  }
  return { target: target.trim(), heading, alias };
}

// ------------------------------------------------------------------ inline

function buildInline(view: EditorView): DecorationSet {
  const { state } = view;
  const doc = state.doc;
  const active = activeLines(state, view.hasFocus);
  const isActive = (from: number, to: number) => {
    const a = doc.lineAt(from).number;
    const b = doc.lineAt(to).number;
    for (let i = a; i <= b; i++) if (active.has(i)) return true;
    return false;
  };
  const handlers = state.facet(linkHandlers);
  const decos: Range<Decoration>[] = [];
  const skip: [number, number][] = [];
  const tree = syntaxTree(state);

  for (const { from, to } of view.visibleRanges) {
    tree.iterate({
      from,
      to,
      enter: (ref) => {
        const node = ref.node;
        const name = ref.name;
        switch (name) {
          case "Frontmatter":
            skip.push([ref.from, ref.to]);
            for (let pos = ref.from; pos <= ref.to; ) {
              const line = doc.lineAt(pos);
              decos.push(lineClass("cm-frontmatter").range(line.from));
              pos = line.to + 1;
            }
            return false;
          case "ATXHeading1":
          case "ATXHeading2":
          case "ATXHeading3":
          case "ATXHeading4":
          case "ATXHeading5":
          case "ATXHeading6": {
            const level = Number(name.slice(-1));
            decos.push(lineClass(`cm-h cm-h${level}`).range(doc.lineAt(ref.from).from));
            if (!isActive(ref.from, ref.to)) {
              const mark = child(node, "HeaderMark");
              if (mark) {
                const end = doc.sliceString(mark.to, mark.to + 1) === " " ? mark.to + 1 : mark.to;
                decos.push(hide.range(mark.from, end));
              }
            }
            return;
          }
          case "SetextHeading1":
          case "SetextHeading2":
            decos.push(lineClass(`cm-h cm-h${name.slice(-1)}`).range(doc.lineAt(ref.from).from));
            return;
          case "EmphasisMark":
          case "StrikethroughMark":
          case "HighlightMark":
          case "InlineMathMark":
            if (!isActive(ref.from, ref.to)) decos.push(hide.range(ref.from, ref.to));
            return;
          case "CodeMark":
            if (node.parent?.name === "InlineCode" && !isActive(ref.from, ref.to)) decos.push(hide.range(ref.from, ref.to));
            return;
          case "InlineCode":
            skip.push([ref.from, ref.to]);
            return;
          case "Highlight":
            pushMark(decos, { class: "cm-highlight" }, ref.from, ref.to);
            return;
          case "Link": {
            const marks = children(node, "LinkMark");
            const url = child(node, "URL");
            // [text] without a URL (e.g. citation [@key]): not a real link
            if (!url || marks.length < 2) return;
            skip.push([ref.from, ref.to]);
            if (isActive(ref.from, ref.to)) {
              pushMark(decos, { class: "cm-ext-link-raw" }, marks[0].to, marks[1].from);
              return false;
            }
            const text = doc.sliceString(marks[0].to, marks[1].from);
            decos.push(
              Decoration.replace({ widget: new LinkTextWidget(text || doc.sliceString(url.from, url.to), doc.sliceString(url.from, url.to)) }).range(
                ref.from,
                ref.to,
              ),
            );
            return false;
          }
          case "Image": {
            skip.push([ref.from, ref.to]);
            const url = child(node, "URL");
            const marks = children(node, "LinkMark");
            if (!url || isActive(ref.from, ref.to)) return false;
            const altRaw = marks.length >= 2 ? doc.sliceString(marks[0].to, marks[1].from) : "";
            const [alt, w] = altRaw.split("|");
            decos.push(
              Decoration.replace({
                widget: new ImageWidget(doc.sliceString(url.from, url.to), alt ?? "", w && /^\d+$/.test(w) ? Number(w) : null, ref.from),
              }).range(ref.from, ref.to),
            );
            return false;
          }
          case "WikiLink":
          case "WikiEmbed": {
            skip.push([ref.from, ref.to]);
            const tgt = child(node, "WikiLinkTarget");
            if (!tgt) return false;
            const raw = doc.sliceString(tgt.from, tgt.to);
            const { target, heading, alias } = parseWikiTarget(raw);
            if (isActive(ref.from, ref.to)) {
              pushMark(decos, { class: "cm-wikilink-raw" }, tgt.from, tgt.to);
              return false;
            }
            // Embed alone on its line: rendered as a block by the StateField (the note's content).
            if (name === "WikiEmbed" && !IMAGE_EXT.test(target) && doc.lineAt(ref.from).text.trim() === doc.sliceString(ref.from, ref.to)) return false;
            if (name === "WikiEmbed" && IMAGE_EXT.test(target)) {
              const width = /^\d+$/.test(alias) ? Number(alias) : null;
              decos.push(Decoration.replace({ widget: new ImageWidget(target, target, width, ref.from) }).range(ref.from, ref.to));
              return false;
            }
            const full = heading ? `${target}#${heading}` : target;
            const label = alias || (target ? (heading ? `${target} › ${heading}` : target) : `› ${heading}`);
            const exists = !target || (handlers?.exists(target) ?? true);
            decos.push(
              Decoration.replace({ widget: new WikiLinkWidget(full, (name === "WikiEmbed" ? "↳ " : "") + label, exists) }).range(ref.from, ref.to),
            );
            return false;
          }
          case "InlineMath": {
            skip.push([ref.from, ref.to]);
            if (isActive(ref.from, ref.to)) {
              pushMark(decos, { class: "cm-math-src" }, ref.from, ref.to);
              return false;
            }
            const tex = doc.sliceString(ref.from + 1, ref.to - 1);
            decos.push(Decoration.replace({ widget: new MathWidget(tex, false, ref.from) }).range(ref.from, ref.to));
            return false;
          }
          case "BlockMath":
            skip.push([ref.from, ref.to]);
            for (let pos = ref.from; pos <= ref.to; ) {
              const line = doc.lineAt(pos);
              decos.push(lineClass("cm-math-src-line").range(line.from));
              pos = line.to + 1;
            }
            return false;
          case "FencedCode": {
            skip.push([ref.from, ref.to]);
            const first = doc.lineAt(ref.from);
            const last = doc.lineAt(ref.to);
            for (let n = first.number; n <= last.number; n++) {
              const line = doc.line(n);
              const cls =
                "cm-codeblock" + (n === first.number ? " cm-codeblock-begin" : "") + (n === last.number ? " cm-codeblock-end" : "");
              decos.push(lineClass(cls).range(line.from));
            }
            return false;
          }
          case "Blockquote": {
            const first = doc.lineAt(ref.from);
            const last = doc.lineAt(ref.to);
            const m = CALLOUT_RE.exec(first.text);
            const kind = m?.[1]?.toLowerCase();
            const color = m?.[2]?.toLowerCase();
            for (let n = first.number; n <= last.number; n++) {
              const line = doc.line(n);
              let cls = m ? `cm-callout cm-callout-${kind}${color ? ` cm-callout-c-${color}` : ""}` : "cm-quote";
              if (n === first.number) cls += m ? " cm-callout-head" : " cm-quote-first";
              if (n === last.number) cls += m ? " cm-callout-last" : " cm-quote-last";
              decos.push(lineClass(cls).range(line.from));
            }
            if (m && !isActive(first.from, first.to)) {
              const markerEnd = first.from + first.text.indexOf("]") + 1 + m[3].length;
              const end = doc.sliceString(markerEnd, markerEnd + 1) === " " ? markerEnd + 1 : markerEnd;
              decos.push(
                Decoration.replace({ widget: new CalloutHeaderWidget(m[1], m[3], m[4].trim().length > 0, first.to) }).range(first.from, end),
              );
              // The ">" marker of the first line is already replaced
              return;
            }
            return;
          }
          case "QuoteMark": {
            const line = doc.lineAt(ref.from);
            if (isActive(line.from, line.to)) return;
            // First line of a callout already replaced
            const bq = node.parent;
            if (bq && doc.lineAt(bq.from).number === line.number && CALLOUT_RE.test(line.text)) return;
            const end = doc.sliceString(ref.to, ref.to + 1) === " " ? ref.to + 1 : ref.to;
            decos.push(hide.range(ref.from, end));
            return;
          }
          case "ListMark": {
            const line = doc.lineAt(ref.from);
            const task = node.nextSibling?.name === "Task" ? node.nextSibling : null;
            const marker = task ? child(task, "TaskMarker") : null;
            if (marker) {
              const checked = /\[[xX]\]/.test(doc.sliceString(marker.from, marker.to));
              if (checked) decos.push(lineClass("cm-task-done").range(line.from));
              if (!isActive(line.from, line.to)) {
                const end = doc.sliceString(marker.to, marker.to + 1) === " " ? marker.to + 1 : marker.to;
                decos.push(Decoration.replace({ widget: new CheckboxWidget(checked, marker.from) }).range(ref.from, end));
              }
              return;
            }
            if (node.parent?.parent?.name === "BulletList" && !isActive(line.from, line.to)) {
              decos.push(Decoration.replace({ widget: new BulletWidget(listDepth(node)) }).range(ref.from, ref.to));
            }
            return;
          }
          case "HorizontalRule":
            if (!isActive(ref.from, ref.to)) decos.push(Decoration.replace({ widget: new HrWidget() }).range(ref.from, ref.to));
            return;
          case "Table": {
            const first = doc.lineAt(ref.from);
            const last = doc.lineAt(ref.to);
            for (let n = first.number; n <= last.number; n++) {
              const line = doc.line(n);
              let cls = "cm-table";
              if (n === first.number) cls += " cm-table-head";
              if (/^\s*\|?\s*:?-{2,}/.test(line.text)) cls += " cm-table-delim";
              decos.push(lineClass(cls).range(line.from));
            }
            return;
          }
          case "CommentBlock":
          case "Comment":
            if (!isActive(ref.from, ref.to)) {
              if (name === "CommentBlock") {
                for (let pos = ref.from; pos <= ref.to; ) {
                  const line = doc.lineAt(pos);
                  decos.push(lineClass("cm-comment-line").range(line.from));
                  pos = line.to + 1;
                }
              }
              decos.push(hide.range(ref.from, ref.to));
            }
            return false;
          case "URL":
            if (node.parent?.name !== "Link" && node.parent?.name !== "Image") {
              pushMark(decos, { class: "cm-url", attributes: { "data-url": doc.sliceString(ref.from, ref.to) } }, ref.from, ref.to);
            }
            return;
        }
      },
    });

    // Tags and citations (outside code, links, front matter)
    const inSkip = (pos: number) => skip.some(([a, b]) => pos >= a && pos < b);
    const text = doc.sliceString(from, to);
    for (const m of text.matchAll(TAG_RE)) {
      const start = from + m.index! + m[1].length;
      const tag = m[2].replace(/[/-]+$/, "");
      if (/^\d+$/.test(tag) || inSkip(start)) continue;
      // No tag in a "# Heading" (the # is followed by a space, hence already excluded)
      decos.push(
        Decoration.mark({ class: "cm-tag", attributes: { "data-tag": tag } }).range(start, start + 1 + tag.length),
      );
    }
    for (const g of text.matchAll(CITE_RE)) {
      const gStart = from + g.index!;
      if (inSkip(gStart)) continue;
      for (const k of g[0].matchAll(KEY_RE)) {
        const s = gStart + k.index!;
        pushMark(decos, { class: "cm-citation", attributes: { "data-cite": k[1] } }, s, s + k[0].length);
      }
    }
  }
  return Decoration.set(decos, true);
}

const inlinePlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = buildInline(view);
    }
    update(u: ViewUpdate) {
      if (
        u.docChanged ||
        u.viewportChanged ||
        u.selectionSet ||
        u.focusChanged ||
        syntaxTree(u.state) !== syntaxTree(u.startState)
      ) {
        this.decorations = buildInline(u.view);
      }
    }
  },
  { decorations: (v) => v.decorations },
);

// ------------------------------------------------------------------- blocs

function buildBlocks(state: EditorState): DecorationSet {
  const decos: Range<Decoration>[] = [];
  const doc = state.doc;
  const root = document.documentElement;
  const theme = root.dataset.scheme === "dark" ? "dark" : root.dataset.theme === "sepia" ? "sepia" : "light";
  syntaxTree(state).iterate({
    enter: (ref) => {
      const name = ref.name;
      if (name === "Frontmatter") {
        if (!selectionTouches(state, ref.from, ref.to)) {
          const yaml = doc.sliceString(ref.from, ref.to);
          // Source note (from Zotero or from Eloria's library): the item card,
          // as in the Zotero panel. Reading notes keep their plain properties.
          const isSource = /^type:\s*["']?source["']?\s*$/m.test(yaml);
          const widget = isSource ? new DocumentWidget(yaml) : new PropertiesWidget(yaml, ref.to);
          decos.push(Decoration.replace({ widget, block: true }).range(ref.from, ref.to));
        }
        return false;
      }
      if (name === "FencedCode") {
        const info = ref.node.getChild("CodeInfo");
        const lang = info ? doc.sliceString(info.from, info.to).trim().toLowerCase() : "";
        if (lang === "mermaid" && !selectionTouches(state, ref.from, ref.to)) {
          const text = ref.node.getChild("CodeText");
          const code = text ? doc.sliceString(text.from, text.to) : "";
          const end = doc.lineAt(ref.to).to;
          decos.push(Decoration.replace({ widget: new MermaidWidget(code, ref.from, theme), block: true }).range(doc.lineAt(ref.from).from, end));
        }
        return false;
      }
      if (name === "BlockMath") {
        if (!selectionTouches(state, ref.from, ref.to)) {
          const src = doc.sliceString(ref.from, ref.to).trim();
          const tex = src.replace(/^\$\$/, "").replace(/\$\$$/, "").trim();
          decos.push(
            Decoration.replace({ widget: new MathWidget(tex, true, ref.from), block: true }).range(doc.lineAt(ref.from).from, doc.lineAt(ref.to).to),
          );
        }
        return false;
      }
      if (name === "WikiEmbed") {
        const line = doc.lineAt(ref.from);
        const raw = doc.sliceString(ref.from, ref.to);
        const target = raw.replace(/^!\[\[/, "").replace(/\]\]$/, "").split("|")[0].trim();
        if (line.text.trim() === raw && target && !IMAGE_EXT.test(target.split("#")[0]) && !selectionTouches(state, line.from, line.to)) {
          decos.push(Decoration.replace({ widget: new EmbedWidget(target), block: true }).range(line.from, line.to));
        }
        return false;
      }
      if (name === "Table") {
        if (!selectionTouches(state, ref.from, ref.to)) {
          const from = doc.lineAt(ref.from).from;
          const to = doc.lineAt(ref.to).to;
          decos.push(Decoration.replace({ widget: new TableWidget(doc.sliceString(from, to), from), block: true }).range(from, to));
        }
        return false;
      }
      if (name === "Blockquote") {
        const first = doc.lineAt(ref.from);
        const m = CALLOUT_RE.exec(first.text);
        if (m && m[3] === "-" && !selectionTouches(state, ref.from, ref.to)) {
          const last = doc.lineAt(ref.to);
          if (last.number > first.number) {
            decos.push(Decoration.replace({ block: true }).range(doc.line(first.number + 1).from, last.to));
          }
        }
        return false;
      }
      return;
    },
  });
  return Decoration.set(decos, true);
}

const blockField = StateField.define<DecorationSet>({
  create: (state) => buildBlocks(state),
  update(value, tr) {
    if (tr.docChanged || tr.selection || syntaxTree(tr.state) !== syntaxTree(tr.startState)) return buildBlocks(tr.state);
    return value;
  },
  provide: (f) => EditorView.decorations.from(f),
});

export const livePreview = [inlinePlugin, blockField];

/** Minimal decorations in source mode: only heading sizes and code. */
export const sourceMode = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = this.build(view);
    }
    update(u: ViewUpdate) {
      if (u.docChanged || u.viewportChanged || syntaxTree(u.state) !== syntaxTree(u.startState)) this.decorations = this.build(u.view);
    }
    build(view: EditorView) {
      const decos: Range<Decoration>[] = [];
      const doc = view.state.doc;
      for (const { from, to } of view.visibleRanges) {
        syntaxTree(view.state).iterate({
          from,
          to,
          enter: (ref) => {
            if (/^ATXHeading\d$/.test(ref.name)) decos.push(lineClass(`cm-h cm-h${ref.name.slice(-1)} cm-h-source`).range(doc.lineAt(ref.from).from));
            if (ref.name === "FencedCode") {
              for (let pos = ref.from; pos <= ref.to; ) {
                const line = doc.lineAt(pos);
                decos.push(lineClass("cm-codeblock").range(line.from));
                pos = line.to + 1;
              }
              return false;
            }
            return;
          },
        });
      }
      return Decoration.set(decos, true);
    }
  },
  { decorations: (v) => v.decorations },
);
