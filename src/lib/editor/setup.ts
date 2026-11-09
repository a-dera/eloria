// CodeMirror 6 editor assembly.

import { closeBrackets, closeBracketsKeymap, completionKeymap } from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { HighlightStyle, indentUnit, syntaxHighlighting } from "@codemirror/language";
import { languages } from "@codemirror/language-data";
import { highlightSelectionMatches, search, searchKeymap } from "@codemirror/search";
import { Compartment, EditorState, type Extension } from "@codemirror/state";
import {
  drawSelection,
  dropCursor,
  EditorView,
  highlightActiveLine,
  keymap,
  lineNumbers,
  placeholder,
  rectangularSelection,
  type KeyBinding,
} from "@codemirror/view";
import { tags } from "@lezer/highlight";
import type { Settings } from "../types";
import { eloriaAutocomplete } from "./autocomplete";
import { formatTable, insertLink, setHeading, tableAddRow, toggleLinePrefix, toggleTask, toggleWrap } from "./commands";
import { livePreview, sourceMode } from "./livePreview";
import { eloriaMarkdown } from "./markdownExt";
import { formatBar } from "./formatBar";
import { linkHandlers, type LinkHandlers } from "./widgets";
import { isAltGrText } from "../util";
import { t } from "../i18n.svelte";

/** Shared highlighting (notes, code blocks, LaTeX), based on the theme variables. */
export const highlight = HighlightStyle.define([
  { tag: tags.heading1, class: "tok-heading tok-h1" },
  { tag: tags.heading2, class: "tok-heading tok-h2" },
  { tag: tags.heading3, class: "tok-heading tok-h3" },
  { tag: [tags.heading4, tags.heading5, tags.heading6], class: "tok-heading" },
  { tag: tags.strong, class: "tok-strong" },
  { tag: tags.emphasis, class: "tok-em" },
  { tag: tags.strikethrough, class: "tok-strike" },
  { tag: tags.link, class: "tok-link" },
  { tag: tags.url, class: "tok-url" },
  { tag: tags.monospace, class: "tok-mono" },
  { tag: tags.quote, class: "tok-quote" },
  { tag: tags.processingInstruction, class: "tok-mark" },
  { tag: tags.meta, class: "tok-meta" },
  { tag: tags.special(tags.content), class: "tok-highlight" },
  { tag: tags.special(tags.string), class: "tok-math" },
  { tag: tags.list, class: "tok-list" },
  { tag: tags.contentSeparator, class: "tok-hr" },
  // Code blocks
  { tag: tags.keyword, class: "tok-keyword" },
  { tag: [tags.string, tags.regexp], class: "tok-string" },
  { tag: [tags.number, tags.bool, tags.null], class: "tok-number" },
  { tag: tags.comment, class: "tok-comment" },
  { tag: [tags.function(tags.variableName), tags.function(tags.propertyName)], class: "tok-fn" },
  { tag: [tags.typeName, tags.className], class: "tok-type" },
  { tag: tags.propertyName, class: "tok-prop" },
  { tag: tags.operator, class: "tok-op" },
  // LaTeX mode (StreamLanguage stex): commands, environments, arguments
  { tag: tags.tagName, class: "tok-keyword" },
  { tag: [tags.atom, tags.special(tags.variableName)], class: "tok-type" },
  { tag: tags.bracket, class: "tok-op" },
  { tag: tags.invalid, class: "tok-invalid" },
]);

export interface EditorCallbacks {
  onChange(content: string): void;
  onSave(): void;
  onCursor(line: number, words: number, chars: number, selection: number): void;
  onPasteFile(file: File): Promise<string | null>;
  onFocus(): void;
  handlers: LinkHandlers;
}

export class EditorConfig {
  readonly preview = new Compartment();
  readonly prefs = new Compartment();
  readonly editable = new Compartment();

  constructor(private cb: EditorCallbacks) {}

  previewExt(s: Settings): Extension {
    return s.live_preview ? livePreview : sourceMode;
  }

  prefsExt(s: Settings): Extension {
    return [
      // In the preferences compartment: follows language changes.
      placeholder(t("Commence à écrire. Tape / pour insérer un bloc, [[ pour un lien, [@ pour citer.")),
      s.show_line_numbers ? lineNumbers() : [],
      EditorView.contentAttributes.of({
        spellcheck: s.spellcheck ? "true" : "false",
        lang: s.spellcheck_lang || "fr",
        autocorrect: "off",
        autocapitalize: "sentences",
      }),
    ];
  }

  extensions(s: Settings): Extension[] {
    const cb = this.cb;
    const mod = (f: (v: EditorView) => boolean): KeyBinding["run"] => f;
    const keys: KeyBinding[] = [
      { key: "Mod-s", run: () => (cb.onSave(), true), preventDefault: true },
      { key: "Mod-b", run: mod((v) => toggleWrap(v, "**")) },
      { key: "Mod-i", run: mod((v) => toggleWrap(v, "*")) },
      { key: "Mod-Shift-x", run: mod((v) => toggleWrap(v, "~~")) },
      { key: "Mod-Shift-h", run: mod((v) => toggleWrap(v, "==")) },
      { key: "Mod-e", run: mod((v) => toggleWrap(v, "`")) },
      { key: "Mod-k", run: insertLink },
      { key: "Mod-l", run: toggleTask },
      { key: "Mod-Shift-8", run: mod((v) => toggleLinePrefix(v, "bullet")) },
      { key: "Mod-Shift-7", run: mod((v) => toggleLinePrefix(v, "number")) },
      { key: "Mod-Shift-.", run: mod((v) => toggleLinePrefix(v, "quote")) },
      { key: "Mod-Alt-t", run: formatTable },
      { key: "Mod-Alt-Enter", run: tableAddRow },
    ];

    return [
      markdown({ base: markdownLanguage, codeLanguages: languages, extensions: [eloriaMarkdown], addKeymap: true }),
      history(),
      drawSelection(),
      dropCursor(),
      rectangularSelection(),
      closeBrackets(),
      highlightActiveLine(),
      highlightSelectionMatches({ minSelectionLength: 3 }),
      search({ top: true }),
      indentUnit.of("    "),
      EditorState.tabSize.of(4),
      EditorView.lineWrapping,
      syntaxHighlighting(highlight),
      eloriaAutocomplete,
      formatBar,
      keymap.of([...keys, ...closeBracketsKeymap, ...completionKeymap, ...searchKeymap, ...historyKeymap, ...defaultKeymap, indentWithTab]),
      linkHandlers.of(cb.handlers),
      this.preview.of(this.previewExt(s)),
      this.prefs.of(this.prefsExt(s)),
      this.editable.of(EditorView.editable.of(true)),
      EditorView.updateListener.of((u) => {
        if (u.docChanged) cb.onChange(u.state.doc.toString());
        if (u.docChanged || u.selectionSet) {
          const st = u.state;
          const sel = st.selection.main;
          const text = st.doc.toString();
          const words = (text.replace(/^---\n[\s\S]*?\n---\n/, "").match(/[\p{L}\p{N}][\p{L}\p{N}'’-]*/gu) ?? []).length;
          const selWords = sel.empty ? 0 : (st.sliceDoc(sel.from, sel.to).match(/[\p{L}\p{N}][\p{L}\p{N}'’-]*/gu) ?? []).length;
          cb.onCursor(st.doc.lineAt(sel.head).number - 1, words, text.length, selWords);
        }
        if (u.focusChanged && u.view.hasFocus) cb.onFocus();
      }),
      EditorView.domEventHandlers({
        // Ctrl+Alt+0…6: headings. Read from the physical key (e.code), because on AZERTY
        // digits need Shift; AltGr (typing # [ { @…) is let through.
        keydown: (e, view) => {
          const m = /^(?:Digit|Numpad)([0-6])$/.exec(e.code);
          if (!m || !(e.ctrlKey || e.metaKey) || !e.altKey || e.shiftKey || isAltGrText(e)) return false;
          e.preventDefault();
          return setHeading(view, Number(m[1]));
        },
        paste: (e, view) => {
          const files = [...(e.clipboardData?.files ?? [])].filter((f) => f.type.startsWith("image/"));
          if (!files.length) return false;
          e.preventDefault();
          for (const f of files) {
            cb.onPasteFile(f).then((link) => {
              if (link) {
                const r = view.state.selection.main;
                view.dispatch({ changes: { from: r.from, to: r.to, insert: link }, selection: { anchor: r.from + link.length } });
              }
            });
          }
          return true;
        },
        drop: (e, view) => {
          const files = [...(e.dataTransfer?.files ?? [])].filter((f) => f.type.startsWith("image/"));
          if (!files.length) return false;
          e.preventDefault();
          const pos = view.posAtCoords({ x: e.clientX, y: e.clientY }) ?? view.state.selection.main.head;
          for (const f of files) {
            cb.onPasteFile(f).then((link) => link && view.dispatch({ changes: { from: pos, insert: link } }));
          }
          return true;
        },
        mousedown: (e, view) => {
          const el = e.target as HTMLElement;
          const tag = el.closest<HTMLElement>(".cm-tag");
          const cite = el.closest<HTMLElement>(".cm-citation");
          const url = el.closest<HTMLElement>(".cm-url");
          const raw = el.closest<HTMLElement>(".cm-wikilink-raw, .cm-ext-link-raw");
          const modDown = e.ctrlKey || e.metaKey;
          // On a line being edited, Ctrl/Cmd is needed to follow a link.
          const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
          const editing = pos !== null && view.hasFocus && view.state.doc.lineAt(pos).number === view.state.doc.lineAt(view.state.selection.main.head).number;
          if (!modDown && editing) return false;
          if (tag) {
            e.preventDefault();
            cb.handlers.openTag(tag.dataset.tag!);
            return true;
          }
          if (cite) {
            e.preventDefault();
            cb.handlers.openCitation(cite.dataset.cite!, e);
            return true;
          }
          if (url && modDown) {
            e.preventDefault();
            cb.handlers.openUrl(url.dataset.url!, e);
            return true;
          }
          if (raw && modDown && pos !== null) {
            const line = view.state.doc.lineAt(pos);
            const off = pos - line.from;
            const wl = [...line.text.matchAll(/\[\[([^\]]+)\]\]/g)].find((m) => m.index! <= off && off <= m.index! + m[0].length);
            if (wl) {
              e.preventDefault();
              const target = wl[1].split("|")[0];
              cb.handlers.openWiki(target, e);
              return true;
            }
            const ml = [...line.text.matchAll(/\[[^\]]*\]\(([^)\s]+)\)/g)].find((m) => m.index! <= off && off <= m.index! + m[0].length);
            if (ml) {
              e.preventDefault();
              cb.handlers.openUrl(ml[1], e);
              return true;
            }
          }
          return false;
        },
      }),
    ];
  }

  reconfigure(view: EditorView, s: Settings) {
    view.dispatch({ effects: [this.preview.reconfigure(this.previewExt(s)), this.prefs.reconfigure(this.prefsExt(s))] });
  }

  setEditable(view: EditorView, on: boolean) {
    view.dispatch({ effects: this.editable.reconfigure(EditorView.editable.of(on)) });
  }
}
