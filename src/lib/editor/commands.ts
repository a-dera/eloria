// Formatting commands (keyboard shortcuts, palette, toolbar).

import { EditorSelection, type ChangeSpec } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";

/** Wraps (or unwraps) the selection with a marker: **bold**, *italics*, ==highlight==... */
export function toggleWrap(view: EditorView, mark: string): boolean {
  const { state } = view;
  const tr = state.changeByRange((range) => {
    const before = state.sliceDoc(range.from - mark.length, range.from);
    const after = state.sliceDoc(range.to, range.to + mark.length);
    if (before === mark && after === mark) {
      return {
        changes: [
          { from: range.from - mark.length, to: range.from },
          { from: range.to, to: range.to + mark.length },
        ],
        range: EditorSelection.range(range.from - mark.length, range.to - mark.length),
      };
    }
    const text = state.sliceDoc(range.from, range.to);
    if (text.startsWith(mark) && text.endsWith(mark) && text.length >= mark.length * 2) {
      const inner = text.slice(mark.length, text.length - mark.length);
      return { changes: { from: range.from, to: range.to, insert: inner }, range: EditorSelection.range(range.from, range.from + inner.length) };
    }
    if (range.empty) {
      // Word under the cursor
      const word = state.wordAt(range.head);
      if (word) {
        return {
          changes: [
            { from: word.from, insert: mark },
            { from: word.to, insert: mark },
          ],
          range: EditorSelection.cursor(range.head + mark.length),
        };
      }
      return { changes: { from: range.from, insert: mark + mark }, range: EditorSelection.cursor(range.from + mark.length) };
    }
    return {
      changes: [
        { from: range.from, insert: mark },
        { from: range.to, insert: mark },
      ],
      range: EditorSelection.range(range.from + mark.length, range.to + mark.length),
    };
  });
  view.dispatch(state.update(tr, { scrollIntoView: true, userEvent: "input.format" }));
  return true;
}

/** Changes the heading level of the selected lines (0 = paragraph). */
export function setHeading(view: EditorView, level: number): boolean {
  const { state } = view;
  const changes: ChangeSpec[] = [];
  const seen = new Set<number>();
  for (const r of state.selection.ranges) {
    for (let pos = r.from; pos <= r.to; ) {
      const line = state.doc.lineAt(pos);
      if (!seen.has(line.number)) {
        seen.add(line.number);
        const m = /^(#{1,6})\s+/.exec(line.text);
        const current = m ? m[1].length : 0;
        const prefix = level === 0 || current === level ? "" : "#".repeat(level) + " ";
        changes.push({ from: line.from, to: line.from + (m ? m[0].length : 0), insert: prefix });
      }
      pos = line.to + 1;
    }
  }
  view.dispatch({ changes, userEvent: "input.format" });
  return true;
}

/** Line prefix: list, numbered list, task, quote. */
export function toggleLinePrefix(view: EditorView, kind: "bullet" | "number" | "task" | "quote"): boolean {
  const { state } = view;
  const changes: ChangeSpec[] = [];
  const seen = new Set<number>();
  let n = 1;
  for (const r of state.selection.ranges) {
    for (let pos = r.from; pos <= r.to; ) {
      const line = state.doc.lineAt(pos);
      if (!seen.has(line.number)) {
        seen.add(line.number);
        const indent = /^\s*/.exec(line.text)![0];
        const rest = line.text.slice(indent.length);
        const patterns = {
          task: /^[-*+] \[[ xX]\] /,
          bullet: /^[-*+] (?!\[[ xX]\] )/,
          number: /^\d+[.)] /,
          quote: /^> ?/,
        };
        const existing = patterns[kind].exec(rest);
        const any = /^([-*+] \[[ xX]\] |[-*+] |\d+[.)] |> ?)/.exec(rest);
        const from = line.from + indent.length;
        if (existing) {
          changes.push({ from, to: from + existing[0].length, insert: "" });
        } else {
          const prefix = kind === "bullet" ? "- " : kind === "number" ? `${n++}. ` : kind === "task" ? "- [ ] " : "> ";
          changes.push({ from, to: from + (any ? any[0].length : 0), insert: prefix });
        }
      }
      pos = line.to + 1;
    }
  }
  view.dispatch({ changes, userEvent: "input.format" });
  return true;
}

/** Checks / unchecks the task of the current line (or creates it). */
export function toggleTask(view: EditorView): boolean {
  const { state } = view;
  const line = state.doc.lineAt(state.selection.main.head);
  const m = /^(\s*[-*+] )\[([ xX])\]/.exec(line.text);
  if (m) {
    const pos = line.from + m[1].length + 1;
    view.dispatch({ changes: { from: pos, to: pos + 1, insert: m[2] === " " ? "x" : " " } });
    return true;
  }
  return toggleLinePrefix(view, "task");
}

/** Ctrl+K: Markdown link around the selection. */
export function insertLink(view: EditorView): boolean {
  const { state } = view;
  const r = state.selection.main;
  const text = state.sliceDoc(r.from, r.to);
  if (/^https?:\/\//.test(text)) {
    view.dispatch({ changes: { from: r.from, to: r.to, insert: `[](${text})` }, selection: { anchor: r.from + 1 } });
  } else {
    const insert = `[${text}](url)`;
    const urlStart = r.from + text.length + 3;
    view.dispatch({ changes: { from: r.from, to: r.to, insert }, selection: { anchor: urlStart, head: urlStart + 3 } });
  }
  return true;
}

export function insertText(view: EditorView, text: string, cursorMarker = "§"): void {
  const r = view.state.selection.main;
  const idx = text.indexOf(cursorMarker);
  const insert = idx >= 0 ? text.replace(cursorMarker, "") : text;
  view.dispatch({
    changes: { from: r.from, to: r.to, insert },
    selection: { anchor: r.from + (idx >= 0 ? idx : insert.length) },
    scrollIntoView: true,
    userEvent: "input",
  });
  view.focus();
}

/** Inserts a block on its own line. */
export function insertBlock(view: EditorView, block: string): void {
  const { state } = view;
  const r = state.selection.main;
  const line = state.doc.lineAt(r.from);
  const before = line.text.trim() === "" ? "" : "\n\n";
  const atLineStart = line.text.trim() === "" ? line.from : r.to;
  const idx = block.indexOf("§");
  const clean = block.replace("§", "");
  const insert = before + clean + (state.doc.lineAt(r.to).to === state.doc.length ? "\n" : "");
  view.dispatch({
    changes: { from: atLineStart, to: line.text.trim() === "" ? line.to : r.to, insert },
    selection: { anchor: atLineStart + before.length + (idx >= 0 ? idx : clean.length) },
    scrollIntoView: true,
  });
  view.focus();
}

export function tableTemplate(cols = 3, rows = 2): string {
  const head = "| " + Array.from({ length: cols }, (_, i) => (i === 0 ? "§Colonne 1" : `Colonne ${i + 1}`)).join(" | ") + " |";
  const sep = "| " + Array.from({ length: cols }, () => "---").join(" | ") + " |";
  const row = "| " + Array.from({ length: cols }, () => "   ").join(" | ") + " |";
  return [head, sep, ...Array.from({ length: rows }, () => row)].join("\n");
}

/** Aligns the columns of the table under the cursor. */
export function formatTable(view: EditorView): boolean {
  const { state } = view;
  const cur = state.doc.lineAt(state.selection.main.head);
  if (!cur.text.trim().startsWith("|")) return false;
  let first = cur.number;
  let last = cur.number;
  while (first > 1 && state.doc.line(first - 1).text.trim().startsWith("|")) first--;
  while (last < state.doc.lines && state.doc.line(last + 1).text.trim().startsWith("|")) last++;
  const rows: string[][] = [];
  for (let i = first; i <= last; i++) {
    const t = state.doc.line(i).text.trim().replace(/^\||\|$/g, "");
    rows.push(t.split(/(?<!\\)\|/).map((c) => c.trim()));
  }
  const ncol = Math.max(...rows.map((r) => r.length));
  const isSep = (r: string[]) => r.every((c) => /^:?-{1,}:?$/.test(c) || c === "");
  const widths = Array.from({ length: ncol }, (_, c) =>
    Math.max(3, ...rows.filter((r) => !isSep(r)).map((r) => [...(r[c] ?? "")].length)),
  );
  const out = rows.map((r) => {
    if (isSep(r)) {
      return "| " + widths.map((w, c) => {
        const cell = r[c] ?? "---";
        const l = cell.startsWith(":");
        const rr = cell.endsWith(":") && cell.length > 1;
        return (l ? ":" : "-") + "-".repeat(Math.max(1, w - 2)) + (rr ? ":" : "-");
      }).join(" | ") + " |";
    }
    return "| " + widths.map((w, c) => {
      const cell = r[c] ?? "";
      return cell + " ".repeat(Math.max(0, w - [...cell].length));
    }).join(" | ") + " |";
  });
  const from = state.doc.line(first).from;
  const to = state.doc.line(last).to;
  view.dispatch({ changes: { from, to, insert: out.join("\n") } });
  return true;
}

/** Adds a column or a row to the current table. */
export function tableAddRow(view: EditorView): boolean {
  const { state } = view;
  const line = state.doc.lineAt(state.selection.main.head);
  if (!line.text.trim().startsWith("|")) return false;
  const cols = line.text.trim().replace(/^\||\|$/g, "").split(/(?<!\\)\|/).length;
  const row = "\n| " + Array.from({ length: cols }, () => " ").join(" | ") + " |";
  view.dispatch({ changes: { from: line.to, insert: row }, selection: { anchor: line.to + 3 } });
  return true;
}
