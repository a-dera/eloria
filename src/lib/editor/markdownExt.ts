// Syntax extensions for CodeMirror's Markdown parser (Lezer):
// YAML front matter, ==highlight==, [[internal links]], $math$ and $$ blocks.

import type { BlockParser, InlineParser, MarkdownConfig, DelimiterType } from "@lezer/markdown";
import { tags as t, styleTags } from "@lezer/highlight";

const HighlightDelim: DelimiterType = { resolve: "Highlight", mark: "HighlightMark" };

const highlightParser: InlineParser = {
  name: "Highlight",
  parse(cx, next, pos) {
    if (next !== 61 /* = */ || cx.char(pos + 1) !== 61 || cx.char(pos + 2) === 61) return -1;
    const before = cx.slice(pos - 1, pos);
    const after = cx.slice(pos + 2, pos + 3);
    const sBefore = /\s|^$/.test(before);
    const sAfter = /\s|^$/.test(after);
    return cx.addDelimiter(HighlightDelim, pos, pos + 2, !sAfter, !sBefore);
  },
  after: "Emphasis",
};

const wikiLinkParser: InlineParser = {
  name: "WikiLink",
  parse(cx, next, pos) {
    let start = pos;
    let open = pos;
    if (next === 33 /* ! */) {
      if (cx.char(pos + 1) !== 91 || cx.char(pos + 2) !== 91) return -1;
      open = pos + 1;
    } else if (next !== 91 /* [ */ || cx.char(pos + 1) !== 91) {
      return -1;
    }
    const text = cx.slice(open + 2, cx.end);
    const close = text.indexOf("]]");
    const nl = text.indexOf("\n");
    if (close < 0 || (nl >= 0 && nl < close) || text.slice(0, close).includes("[")) return -1;
    const end = open + 2 + close + 2;
    return cx.addElement(
      cx.elt(next === 33 ? "WikiEmbed" : "WikiLink", start, end, [
        cx.elt("WikiLinkMark", start, open + 2),
        cx.elt("WikiLinkTarget", open + 2, end - 2),
        cx.elt("WikiLinkMark", end - 2, end),
      ]),
    );
  },
  before: "Link",
};

const inlineMathParser: InlineParser = {
  name: "InlineMath",
  parse(cx, next, pos) {
    if (next !== 36 /* $ */ || cx.char(pos + 1) === 36) return -1;
    const first = cx.char(pos + 1);
    if (first === 32 || first === 9 || first === -1) return -1;
    for (let i = pos + 1; i < cx.end; i++) {
      const c = cx.char(i);
      if (c === 10) return -1;
      if (c === 92 /* \ */) {
        i++;
        continue;
      }
      if (c === 36) {
        const prev = cx.char(i - 1);
        const nextC = cx.char(i + 1);
        if (prev === 32 || (nextC >= 48 && nextC <= 57)) continue;
        if (i === pos + 1) return -1;
        return cx.addElement(
          cx.elt("InlineMath", pos, i + 1, [cx.elt("InlineMathMark", pos, pos + 1), cx.elt("InlineMathMark", i, i + 1)]),
        );
      }
    }
    return -1;
  },
  before: "Emphasis",
};

const blockMathParser: BlockParser = {
  name: "BlockMath",
  parse(cx, line) {
    if (!/^\s*\$\$/.test(line.text)) return false;
    const from = cx.lineStart;
    const trimmed = line.text.trim();
    if (trimmed.length > 4 && trimmed.endsWith("$$")) {
      const end = from + line.text.length;
      cx.nextLine();
      cx.addElement(cx.elt("BlockMath", from, end));
      return true;
    }
    let end = from + line.text.length;
    while (cx.nextLine()) {
      end = cx.lineStart + line.text.length;
      if (/\$\$\s*$/.test(line.text)) {
        cx.nextLine();
        break;
      }
    }
    cx.addElement(cx.elt("BlockMath", from, end));
    return true;
  },
  before: "Blockquote",
};

const frontmatterParser: BlockParser = {
  name: "Frontmatter",
  parse(cx, line) {
    if (cx.lineStart !== 0 || line.text !== "---") return false;
    const from = 0;
    let end = line.text.length;
    let closed = false;
    while (cx.nextLine()) {
      end = cx.lineStart + line.text.length;
      if (line.text === "---" || line.text === "...") {
        closed = true;
        cx.nextLine();
        break;
      }
    }
    if (!closed) return false;
    cx.addElement(cx.elt("Frontmatter", from, end));
    return true;
  },
  before: "HorizontalRule",
};

export const eloriaMarkdown: MarkdownConfig = {
  defineNodes: [
    { name: "Highlight" },
    { name: "HighlightMark" },
    { name: "WikiLink" },
    { name: "WikiEmbed" },
    { name: "WikiLinkMark" },
    { name: "WikiLinkTarget" },
    { name: "InlineMath" },
    { name: "InlineMathMark" },
    { name: "BlockMath", block: true },
    { name: "Frontmatter", block: true },
  ],
  parseInline: [wikiLinkParser, inlineMathParser, highlightParser],
  parseBlock: [frontmatterParser, blockMathParser],
  props: [
    styleTags({
      "Highlight/...": t.special(t.content),
      HighlightMark: t.processingInstruction,
      "WikiLink WikiEmbed": t.link,
      WikiLinkMark: t.processingInstruction,
      "InlineMath BlockMath": t.special(t.string),
      InlineMathMark: t.processingInstruction,
      Frontmatter: t.meta,
    }),
  ],
};
