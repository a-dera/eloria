import { locale, t, tk } from "./i18n.svelte";

export function debounce<T extends (...args: any[]) => void>(fn: T, ms: number) {
  let t: ReturnType<typeof setTimeout> | undefined;
  const d = (...args: Parameters<T>) => {
    if (t) clearTimeout(t);
    t = setTimeout(() => fn(...args), ms);
  };
  d.flush = (...args: Parameters<T>) => {
    if (t) clearTimeout(t);
    fn(...args);
  };
  d.cancel = () => t && clearTimeout(t);
  return d;
}

export function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

/** FTS excerpt: \u0001 ... \u0002 wrap the matched terms. */
export function snippetHtml(s: string): string {
  return escapeHtml(s).replace(/\u0001/g, "<mark>").replace(/\u0002/g, "</mark>");
}

export function stem(path: string): string {
  const name = path.split("/").pop() ?? path;
  return name.replace(/\.md$/i, "");
}

export function dirname(path: string): string {
  const i = path.lastIndexOf("/");
  return i < 0 ? "" : path.slice(0, i);
}

export function joinPath(...parts: string[]): string {
  return parts
    .map((p) => p.replace(/^\/+|\/+$/g, ""))
    .filter(Boolean)
    .join("/");
}

export function uid(prefix = "t"): string {
  return `${prefix}${Math.random().toString(36).slice(2, 10)}`;
}

export function countWords(text: string): number {
  const body = text.replace(/^---\n[\s\S]*?\n---\n/, "");
  const m = body.match(/[\p{L}\p{N}][\p{L}\p{N}'’-]*/gu);
  return m ? m.length : 0;
}

export function readingTime(words: number): string {
  const min = Math.max(1, Math.round(words / 230));
  return `${min} min`;
}

export function fmtDate(ts: number | string): string {
  const d = typeof ts === "number" ? new Date(ts > 1e11 ? ts : ts * 1000) : new Date(ts);
  if (isNaN(d.getTime())) return String(ts);
  return d.toLocaleString(locale(), { dateStyle: "medium", timeStyle: "short" });
}

export function today(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

/** Valeur de frontmatter en texte lisible. */
export function propText(v: unknown): string {
  if (v === null || v === undefined) return "";
  if (Array.isArray(v)) return v.map(propText).join(", ");
  if (typeof v === "object") return JSON.stringify(v);
  return String(v);
}

/** Interprets a property input (comma-separated list, number, boolean). */
export function parsePropInput(s: string, previous: unknown): unknown {
  const t = s.trim();
  if (Array.isArray(previous)) return t ? t.split(",").map((x) => x.trim()).filter(Boolean) : [];
  if (t === "") return "";
  if (t === "true" || t === "false") return t === "true";
  if (/^-?\d+(\.\d+)?$/.test(t) && typeof previous === "number") return Number(t);
  return t;
}

export function fuzzyScore(query: string, text: string): number {
  if (!query) return 1;
  const q = query.toLowerCase().normalize("NFD").replace(/\p{Diacritic}/gu, "");
  const t = text.toLowerCase().normalize("NFD").replace(/\p{Diacritic}/gu, "");
  const idx = t.indexOf(q);
  if (idx >= 0) return 1000 - idx - t.length * 0.1;
  let score = 0;
  let ti = 0;
  let streak = 0;
  for (const ch of q) {
    const found = t.indexOf(ch, ti);
    if (found < 0) return -1;
    streak = found === ti ? streak + 1 : 0;
    score += 10 + streak * 5 - (found - ti) * 0.5;
    ti = found + 1;
  }
  return score - t.length * 0.1;
}

export const ZOTERO_COLORS: Record<string, string> = {
  jaune: "#ffd400",
  rouge: "#ff6666",
  vert: "#5fb236",
  bleu: "#2ea8e5",
  violet: "#a28ae5",
  magenta: "#e56eee",
  orange: "#f19837",
  gris: "#aaaaaa",
};

export const STATUS_ORDER = ["à lire", "en cours", "lue", "exploitée"];

export function isMac(): boolean {
  return navigator.platform.toLowerCase().includes("mac");
}

let keyboardLayout: Map<string, string> | null = null;

/** Loads the keyboard layout (AZERTY, QWERTY…), known to WebView2 and Chromium. */
export async function initKeyboardLayout() {
  try {
    const kb = (navigator as Navigator & { keyboard?: { getLayoutMap?: () => Promise<Map<string, string>> } }).keyboard;
    if (kb?.getLayoutMap) keyboardLayout = await kb.getLayoutMap();
  } catch {
    keyboardLayout = null;
  }
}

/**
 * On Windows, AltGr arrives as Ctrl+Alt. If the key produces a character other
 * than its base character (AltGr+3 gives "#" on AZERTY), it is typing,
 * not a shortcut.
 */
export function isAltGrText(e: KeyboardEvent): boolean {
  if (!e.ctrlKey || !e.altKey || e.metaKey || e.key.length !== 1) return false;
  const base = keyboardLayout?.get(e.code);
  if (base !== undefined) return e.key.toLowerCase() !== base.toLowerCase();
  return !/[\p{L}\p{N}]/u.test(e.key);
}

export function modKey(e: KeyboardEvent | MouseEvent): boolean {
  return isMac() ? e.metaKey : e.ctrlKey;
}

/** Line-by-line diff (longest common subsequence), as HTML: .same, .add, .del. */
export function lineDiff(a: string, b: string): string {
  const x = a.split("\n");
  const y = b.split("\n");
  if (x.length * y.length > 4_000_000) return escapeHtml(a);
  const n = x.length;
  const m = y.length;
  const dp: Uint32Array[] = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
  for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--) dp[i][j] = x[i] === y[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
  const out: string[] = [];
  let i = 0;
  let j = 0;
  while (i < n || j < m) {
    if (i < n && j < m && x[i] === y[j]) {
      out.push(`<div class="same">${escapeHtml(x[i]) || "&nbsp;"}</div>`);
      i++;
      j++;
    } else if (j < m && (i >= n || dp[i][j + 1] >= dp[i + 1][j])) {
      out.push(`<div class="add">${escapeHtml(y[j]) || "&nbsp;"}</div>`);
      j++;
    } else {
      out.push(`<div class="del">${escapeHtml(x[i]) || "&nbsp;"}</div>`);
      i++;
    }
  }
  return out.join("");
}

/** Zotero item types, in the interface language. */
const ITEM_TYPES: Record<string, string> = {
  journalArticle: tk("Article de revue"),
  conferencePaper: tk("Communication de conférence"),
  book: tk("Livre"),
  bookSection: tk("Chapitre de livre"),
  thesis: tk("Thèse"),
  report: tk("Rapport"),
  preprint: tk("Prépublication"),
  webpage: tk("Page web"),
  document: tk("Document"),
  manuscript: tk("Manuscrit"),
  magazineArticle: tk("Article de magazine"),
  newspaperArticle: tk("Article de presse"),
  presentation: tk("Présentation"),
  dataset: tk("Jeu de données"),
  computerProgram: tk("Logiciel"),
  blogPost: tk("Billet de blog"),
  patent: tk("Brevet"),
  standard: tk("Norme"),
  encyclopediaArticle: tk("Article d'encyclopédie"),
  interview: tk("Entretien"),
  videoRecording: tk("Vidéo"),
};

export function itemTypeLabel(type: string): string {
  const k = ITEM_TYPES[type];
  return k ? t(k) : type;
}

/** Short Markdown text made readable: `[[target|alias]]` becomes the alias, without bold or code. */
export function plainMd(s: string): string {
  return s
    .replace(/!?\[\[([^\]|#]*)(?:#[^\]|]*)?(?:\|([^\]]*))?\]\]/g, (_m, a: string, b?: string) => (b ?? a).trim())
    .replace(/\*\*|==|`/g, "");
}
