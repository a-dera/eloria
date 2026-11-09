// Mock backend to develop the interface in a browser (without Tauri).
// It loads the demo vault `examples/demo-vault` and a small
// fake Zotero library. Nothing is written to disk.

import type {
  ExtensionInfo,
  AiFicheDraft,
  Graph,
  GraphEdge,
  GraphNode,
  GraphOptions,
  ItemDetail,
  NoteMeta,
  Settings,
  TreeNode,
  ZAnnotation,
  ZItem,
  Work,
} from "./types";

const raw = import.meta.glob("/examples/demo-vault/**/*.md", { query: "?raw", import: "default", eager: true }) as Record<
  string,
  string
>;

const files = new Map<string, string>();
const mtimes = new Map<string, number>();
for (const [k, v] of Object.entries(raw)) {
  const p = k.replace("/examples/demo-vault/", "");
  files.set(p, v);
  mtimes.set(p, Date.now());
}
const folders = new Set<string>();
/** Mock custom themes (.eloria/themes). */
const customThemes: { id: string; name: string; scheme: "light" | "dark"; css: string }[] = [];
let opened = false;

let settings: Settings = {
  language: (localStorage.getItem("eloria:demo-lang") as "fr" | "en") || "fr",
  theme: "system",
  accent: "#7c5cff",
  font_ui: '"Inter Variable", Inter, "Segoe UI", system-ui, sans-serif',
  font_editor: '"Literata Variable", Literata, Georgia, serif',
  font_mono: '"JetBrains Mono Variable", "JetBrains Mono", Consolas, monospace',
  font_size: 17,
  line_height: 1.7,
  editor_width: 760,
  heading_scale: 1,
  live_preview: true,
  spellcheck: true,
  spellcheck_lang: "fr",
  show_line_numbers: false,
  readable_line_length: true,
  autosave_ms: 800,
  history_interval: 300,
  zotero_data_dir: "",
  zotero_port: 23119,
  fiches_folder: "Fiches de lecture",
  copy_pdfs: true,
  pdf_folder: "PDF",
  sources_folder: "Sources",
  zotero_auto_sync: true,
  backup_dir: "",
  latex_path: "",
  embed_provider: "",
  ai_agents: [{ name: "Relecteur de chapitre", instructions: "Relis comme un jury." }],
  auto_reading_status: true,
  inbox_folder: "Boîte de réception",
  local_api: false,
  goal_fiches_week: 3,
  goal_words_day: 500,
  focus_minutes: 25,
  local_api_port: 27123,
  local_api_token: "",
  embed_model: "",
  embed_url: "",
  backup_interval_hours: 24,
  backup_keep: 10,
  attachments_folder: "Pièces jointes",
  templates_folder: "Modèles",
  daily_folder: "Journal",
  pandoc_path: "",
  csl_path: "",
  keys: {},
  ai_enabled: false,
  ai_provider: "anthropic",
  ai_model: "",
  ai_base_url: "",
  ai_effort: "high",
  research_topic: "",
  recent_vaults: ["/demo/Thèse"],
  last_vault: null,
  property_defs: [
    { name: "statut", kind: "choix", options: ["à lire", "en cours", "lue"] },
    { name: "annee", kind: "nombre", options: [] },
    { name: "priorite", kind: "choix", options: ["haute", "moyenne", "basse"] },
  ],
  saved_views: [{ name: "Sources par année", folder: "", type_filter: "source", tag: "", text: "", sort_key: "annee", sort_dir: -1, hidden: [] }],
  custom_commands: [],
  extension_grants: [],
};

// Extensions of the demo vault.
const EXT_DEMO_CODE = [
  "// Extension de démonstration.",
  "eloria.commands.add({",
  "  id: 'compter',",
  "  name: 'Compteur de mots : compter les mots de la note',",
  "  run: async () => {",
  "    const path = eloria.notes.active();",
  "    if (!path) return eloria.ui.toast('Aucune note ouverte.');",
  "    const text = await eloria.notes.read(path);",
  "    const n = text.split(/\\s+/).filter(Boolean).length;",
  "    const data = (await eloria.data.load()) || { runs: 0 };",
  "    data.runs += 1;",
  "    await eloria.data.save(data);",
  "    eloria.ui.toast(n + ' mots (comptage n° ' + data.runs + ')');",
  "    eloria.log('comptage', path, n);",
  "  },",
  "});",
  "const item = eloria.ui.statusItem({ text: 'compteur prêt', title: 'Compteur de mots' });",
  "eloria.events.on('note-open', (p) => item.set(p ? 'compteur : ' + p.split('/').pop() : 'compteur prêt'));",
  "eloria.onunload(() => eloria.log('déchargée'));",
].join("\n");
const mockExt: Record<string, { name: string; version: string; description: string; author: string; code: string; css: string | null; data: unknown }> = {
  "compteur-de-mots": { name: "Compteur de mots", version: "1.0.0", description: "Compte les mots de la note ouverte (exemple).", author: "Eloria", code: EXT_DEMO_CODE, css: ".ext-compteur { color: var(--accent); }", data: null },
  "ancienne": { name: "Ancienne extension", version: "0.1.0", description: "", author: "", code: "", css: null, data: null },
};
const extHash = (id: string) => ([...(mockExt[id].code + "\u0000" + (mockExt[id].css ?? ""))].reduce((h, c) => (Math.imul(h, 31) + c.charCodeAt(0)) | 0, 7) >>> 0).toString(16).padStart(8, "0").repeat(8);
function extList(): ExtensionInfo[] {
  return Object.entries(mockExt)
    .map(([id, e]) => {
      const error = id === "ancienne" ? "Extension : demande Eloria 2.0.0 ou plus récent" : null;
      const grant = settings.extension_grants.find((g) => g.id === id);
      const hash = error ? "" : extHash(id);
      return { id, name: e.name, version: e.version, description: e.description, author: e.author, dir: `.eloria/extensions/${id}`, hash, has_styles: !!e.css, enabled: !!grant && grant.hash === hash, changed: !!grant && grant.hash !== hash, error };
    })
    .sort((a, b) => a.name.localeCompare(b.name));
}

// --------------------------------------------------------------- analyse

function splitFm(c: string): [Record<string, unknown>, string] {
  const m = c.match(/^---\n([\s\S]*?)\n---\n?/);
  if (!m) return [{}, c];
  const fm: Record<string, unknown> = {};
  for (const line of m[1].split("\n")) {
    const kv = line.match(/^([\w\-À-ÿ]+):\s*(.*)$/);
    if (!kv) continue;
    let v: unknown = kv[2].trim();
    const s = v as string;
    if (s.startsWith("[") && s.endsWith("]")) v = s.slice(1, -1).split(",").map((x) => x.trim().replace(/^"|"$/g, "")).filter(Boolean);
    else if (/^-?\d+$/.test(s)) v = Number(s);
    else if (s.startsWith('"') && s.endsWith('"')) v = s.slice(1, -1);
    fm[kv[1]] = v;
  }
  return [fm, c.slice(m[0].length)];
}

const stemOf = (p: string) => (p.split("/").pop() ?? p).replace(/\.md$/, "");

function meta(p: string): NoteMeta {
  const c = files.get(p) ?? "";
  const [fm, body] = splitFm(c);
  const h1 = body.match(/^# (.+)$/m)?.[1];
  const tags = new Set<string>();
  const fmTags = fm.tags;
  if (Array.isArray(fmTags)) fmTags.forEach((t) => tags.add(String(t)));
  for (const m of body.replace(/```[\s\S]*?```/g, "").matchAll(/(?:^|\s)#([\p{L}\p{N}_][\p{L}\p{N}_/\-]*)/gu)) tags.add(m[1]);
  return {
    path: p,
    title: (fm.titre as string) || (fm.title as string) || h1 || stemOf(p),
    stem: stemOf(p),
    aliases: Array.isArray(fm.aliases) ? (fm.aliases as string[]) : [],
    tags: [...tags],
    mtime: mtimes.get(p) ?? 0,
    words: (body.match(/[\p{L}\p{N}]+/gu) ?? []).length,
    frontmatter: fm,
  };
}

function links(p: string): { target: string; line: number }[] {
  const c = (files.get(p) ?? "").replace(/```[\s\S]*?```/g, (m) => m.replace(/[^\n]/g, " "));
  const out: { target: string; line: number }[] = [];
  c.split("\n").forEach((line, i) => {
    for (const m of line.matchAll(/!?\[\[([^\]|#\n]*)(?:#[^\]|\n]*)?(?:\|[^\]\n]*)?\]\]/g)) out.push({ target: m[1].trim(), line: i });
  });
  return out;
}

function resolve(target: string): string | null {
  const t = target.toLowerCase().replace(/\.md$/, "");
  for (const p of files.keys()) if (p.toLowerCase().replace(/\.md$/, "") === t) return p;
  for (const p of files.keys()) if (stemOf(p).toLowerCase() === t) return p;
  for (const p of files.keys()) if (meta(p).aliases.some((a) => a.toLowerCase() === t)) return p;
  return null;
}

function tree(): TreeNode {
  const root: TreeNode = { name: "Thèse", path: "", is_dir: true, kind: "dir", children: [] };
  const ensure = (dir: string): TreeNode => {
    if (!dir) return root;
    const parts = dir.split("/");
    let cur = root;
    let acc = "";
    for (const part of parts) {
      acc = acc ? `${acc}/${part}` : part;
      let n = cur.children.find((c) => c.is_dir && c.name === part);
      if (!n) {
        n = { name: part, path: acc, is_dir: true, kind: "dir", children: [] };
        cur.children.push(n);
      }
      cur = n;
    }
    return cur;
  };
  for (const f of folders) ensure(f);
  for (const p of files.keys()) {
    const dir = p.includes("/") ? p.slice(0, p.lastIndexOf("/")) : "";
    ensure(dir).children.push({ name: p.split("/").pop()!, path: p, is_dir: false, kind: "note", children: [] });
  }
  const sort = (n: TreeNode) => {
    n.children.sort((a, b) => Number(b.is_dir) - Number(a.is_dir) || a.name.localeCompare(b.name, "fr"));
    n.children.forEach(sort);
  };
  sort(root);
  return root;
}

function graph(opts: GraphOptions): Graph {
  const nodes = new Map<string, GraphNode>();
  for (const p of files.keys()) {
    if (opts.folder && !p.startsWith(opts.folder)) continue;
    if (opts.exclude?.some((e) => e && p.startsWith(e + "/"))) continue;
    const m = meta(p);
    nodes.set(p, { id: p, label: m.title, kind: "note", degree: 0, group: p.includes("/") ? p.split("/")[0] : "" });
  }
  const edges: GraphEdge[] = [];
  const seen = new Set<string>();
  for (const p of nodes.keys()) {
    for (const l of links(p)) {
      let r = resolve(l.target);
      if (!r && opts.include_missing && !l.target.includes(".")) {
        r = `missing:${l.target.toLowerCase()}`;
        if (!nodes.has(r)) nodes.set(r, { id: r, label: l.target, kind: "missing", degree: 0, group: "" });
      }
      if (!r || r === p || !nodes.has(r) || seen.has(p + r)) continue;
      seen.add(p + r);
      edges.push({ source: p, target: r, kind: "link" });
    }
    if (opts.include_tags) {
      for (const t of meta(p).tags) {
        const id = `tag:${t.toLowerCase()}`;
        if (!nodes.has(id)) nodes.set(id, { id, label: `#${t}`, kind: "tag", degree: 0, group: "" });
        edges.push({ source: p, target: id, kind: "tag" });
      }
    }
  }
  for (const e of edges) {
    nodes.get(e.source)!.degree++;
    nodes.get(e.target)!.degree++;
  }
  let ns = [...nodes.values()];
  if (!opts.include_orphans) ns = ns.filter((n) => n.degree > 0);
  return { nodes: ns, edges };
}

// ---------------------------------------------------------------- Zotero

const au = (first: string, last: string) => ({ creator_type: "author", first_name: first, last_name: last });
const zitems: ZItem[] = [
  {
    key: "ITEMAAAA", item_type: "journalArticle", title: "Graph Neural Networks for Mobile Money Fraud Detection",
    creators: [au("Jane", "Smith"), au("Ali", "Ouédraogo")], date: "2024-03", year: 2024, publication: "IEEE Access",
    doi: "10.1109/ACCESS.2024.0001", url: "", abstract_note: "We propose a GNN-based approach for detecting fraud rings in mobile money transaction graphs in West Africa.",
    citekey: "smith2024graph", tags: ["GNN", "fraude"], collections: ["C2"],
    attachments: [{ key: "ATTAAAAA", title: "Full Text PDF", content_type: "application/pdf", path: "" }],
    fields: {}, date_added: "2024-04-01", date_modified: "2024-04-01",
  },
  {
    key: "ITEMBBBB", item_type: "conferencePaper", title: "A Unified Approach to Interpreting Model Predictions",
    creators: [au("Scott M.", "Lundberg"), au("Su-In", "Lee")], date: "2017", year: 2017, publication: "NeurIPS",
    doi: "", url: "", abstract_note: "Introduces SHAP (SHapley Additive exPlanations), a unified framework for interpreting predictions.",
    citekey: "lundberg2017unified", tags: ["XAI", "SHAP"], collections: ["C3"],
    attachments: [{ key: "ATTBBBBB", title: "Full Text PDF", content_type: "application/pdf", path: "" }],
    fields: {}, date_added: "2024-05-10", date_modified: "2024-05-10",
  },
  {
    key: "ITEMCCCC", item_type: "report", title: "État de l'inclusion financière dans l'UEMOA",
    creators: [{ creator_type: "author", first_name: "", last_name: "BCEAO" }], date: "2023", year: 2023, publication: "BCEAO",
    doi: "", url: "https://www.bceao.int", abstract_note: "", citekey: "bceao2023etat", tags: ["UEMOA"], collections: ["C1"],
    attachments: [], fields: {}, date_added: "2024-06-01", date_modified: "2024-06-01",
  },
  {
    key: "ITEMDDDD", item_type: "conferencePaper", title: "\"Why Should I Trust You?\": Explaining the Predictions of Any Classifier",
    creators: [au("Marco Tulio", "Ribeiro"), au("Sameer", "Singh"), au("Carlos", "Guestrin")], date: "2016", year: 2016,
    publication: "KDD", doi: "", url: "", abstract_note: "Introduces LIME.", citekey: "ribeiro2016why", tags: ["XAI", "LIME"],
    collections: ["C3"], attachments: [{ key: "ATTDDDDD", title: "PDF", content_type: "application/pdf", path: "" }],
    fields: {}, date_added: "2024-05-12", date_modified: "2024-05-12",
  },
];
const pdfAnns: Record<string, unknown[]> = {};
let demoFocus = 0;
let demoGit = { available: true, repo: false, branch: "", changes: 0, remote: "", log: [] as [string, string, string][] };
const reviews: Record<string, { citekey: string; statut: string }[]> = {};
const demoBackups: { path: string; name: string; bytes: number; created: string; files: number }[] = [];
/** Mock Crossref / Semantic Scholar results. */
/** Mock disk usage: LaTeX auxiliaries and trash. */
let auxFiles = ["Thèse/Manuscrit/main.aux", "Thèse/Manuscrit/main.log", "Thèse/Manuscrit/main.synctex.gz"];
let trashBytes = 35_000;

/** Argumentations (chemin -> carte). */
const args = new Map<string, { title: string; claim: string; arguments: { title: string; text: string; evidence: string[] }[]; objections: { title: string; text: string; rebuttal: string }[]; conclusion: string }>();
function unknownKeys(arg: { claim: string; arguments: { text: string; evidence: string[] }[]; conclusion: string }): string[] {
  const known = ["smith2024graph", "lundberg2017unified", "ribeiro2016why", "bceao2023etat"];
  const all = [arg.claim, arg.conclusion, ...arg.arguments.flatMap((x) => [x.text, ...x.evidence])].join(" ");
  return [...new Set([...all.matchAll(/@([\w:-]+)/g)].map((m) => m[1]))].filter((k) => !known.includes(k.toLowerCase()));
}

/** Reliability checks (DOI -> signals). */
const reliability = new Map<string, { level: string; checked: string; signals: { level: string; text: string }[] }>();

/** PDF text (extracted by the reader). */
const pdfText = new Map<string, string>();

const DEMO_WORKS: Work[] = [
  { title: "Mobile Money Fraud Detection with Machine Learning", authors: [["Awa", "Konaté"], ["Paul", "Traoré"]], year: 2022, doi: "10.1000/mm.2022.1",
    url: "https://doi.org/10.1000/mm.2022.1", venue: "Journal of Financial Technology", abstract_note: "A study of fraud in West African mobile money.", item_type: "journalArticle", citations: 14, origin: "crossref", keywords: ["Mobile money", "Fraud detection"] },
  { title: "Explainable Fraud Detection in Digital Payments", authors: [["Li", "Wei"]], year: 2024, doi: "10.1000/xfd.2024", url: "https://doi.org/10.1000/xfd.2024",
    venue: "Expert Systems with Applications", abstract_note: "", item_type: "journalArticle", citations: 31, origin: "semanticscholar" },
  { title: "Graph Learning for Transaction Networks", authors: [["Ana", "Silva"], ["Joe", "Brown"], ["Kim", "Lee"]], year: 2023, doi: "", url: "https://openalex.org/W1",
    venue: "KDD", abstract_note: "", item_type: "conferencePaper", citations: 7, origin: "openalex" },
];
const zann: ZAnnotation[] = [
  { key: "AN1", item_key: "ITEMAAAA", attachment_key: "ATTAAAAA", kind: "highlight", text: "Fraud rings tend to form dense subgraphs with high transaction velocity.", comment: "Argument central pour le chapitre 4.", color: "#ffd400", page_label: "4", sort_index: "1", page_index: null, tags: ["ch4"], date_modified: "" },
  { key: "AN2", item_key: "ITEMAAAA", attachment_key: "ATTAAAAA", kind: "highlight", text: "Our dataset covers 2.3M transactions from three operators.", comment: "", color: "#5fb236", page_label: "7", sort_index: "2", page_index: null, tags: [], date_modified: "" },
  { key: "AN3", item_key: "ITEMAAAA", attachment_key: "ATTAAAAA", kind: "note", text: "", comment: "Comparer avec les données UEMOA : volumes plus faibles, plus de P2P.", color: "#2ea8e5", page_label: "9", sort_index: "3", page_index: null, tags: [], date_modified: "" },
  { key: "AN4", item_key: "ITEMBBBB", attachment_key: "ATTBBBBB", kind: "highlight", text: "SHAP values are the unique additive feature attribution satisfying local accuracy, missingness and consistency.", comment: "Définition à citer.", color: "#a28ae5", page_label: "3", sort_index: "1", page_index: null, tags: [], date_modified: "" },
];

function summary(it: ZItem) {
  const a = it.creators;
  const authors = a.length === 1 ? a[0].last_name : a.length === 2 ? `${a[0].last_name} & ${a[1].last_name}` : `${a[0].last_name} et al.`;
  return {
    key: it.key, item_type: it.item_type, title: it.title, authors, year: it.year, citekey: it.citekey,
    publication: it.publication, annotation_count: zann.filter((x) => x.item_key === it.key).length,
  };
}

/** Citation key -> note of the item: the source first (like the core). */
function fichesMap(): Record<string, string> {
  const m: Record<string, string> = {};
  for (const wantSource of [false, true]) {
    for (const p of files.keys()) {
      const fm = meta(p).frontmatter;
      if ((fm.type === "source") !== wantSource) continue;
      if (typeof fm.citekey === "string") m[fm.citekey.toLowerCase()] = p;
    }
  }
  return m;
}

function sourceOf(it: ZItem): string | undefined {
  for (const p of files.keys()) {
    const fm = meta(p).frontmatter;
    if (fm.type === "source" && (fm.zotero === it.key || fm.citekey === it.citekey)) return p;
  }
  return undefined;
}

const SECTIONS = ["Problématique", "Résumé tiré de l'article", "Mots-clés", "Objectifs", "Méthodes", "Sources de données", "Résultats",
  "Discussion des résultats", "Contraintes rencontrées par les auteurs", "Défis", "Perspectives", "Meilleures références citées", "Commentaire"];

function makeSource(it: ZItem, folder = "Sources"): string {
  const p = `${folder}/@${it.citekey}.md`;
  files.set(p, `---\ntype: source\ntitre: "${it.title.replace(/"/g, "'")}"\nannee: ${it.year}\npublication: ${it.publication}\ncitekey: ${it.citekey}\nzotero: ${it.key}\ntags: [source]\n---\n# ${it.title}\n\n${annBlock(it)}\n`);
  return p;
}

function makeFiche(it: ZItem, source: string): string {
  let p = `Fiches de lecture/Fiche @${it.citekey}.md`;
  for (let i = 1; files.has(p); i++) p = `Fiches de lecture/Fiche @${it.citekey} ${i}.md`;
  const stemS = source.split("/").pop()!.replace(/\.md$/, "");
  const body = SECTIONS.map((s) => `## ${s}\n\n\n`).join("");
  files.set(p, `---\ntype: fiche\ntitre: "Fiche : ${it.title.replace(/"/g, "'")}"\nsource: "[[${stemS}]]"\ncitekey: ${it.citekey}\nzotero: ${it.key}\nstatut: à lire\ntags: [fiche]\n---\n# Fiche : ${it.title}\n\n**Source :** [[${stemS}|${it.title}]]\n\n<!-- eloria:ia:debut -->\n${body}<!-- eloria:ia:fin -->\n`);
  return p;
}

function annBlock(it: ZItem): string {
  const anns = zann.filter((a) => a.item_key === it.key);
  let s = `<!-- zotero:debut -->\n## Annotations Zotero\n\n*${anns.length} annotation(s), synchronisées (démo).*\n`;
  for (const a of anns) {
    s += `\n> [!${a.kind === "note" ? "note" : "citation"}|jaune] p. ${a.page_label} · [ouvrir dans Zotero](zotero://open-pdf/library/items/${a.attachment_key}?page=${a.page_label}&annotation=${a.key})\n`;
    if (a.text) s += `> ${a.text}\n`;
    if (a.comment) s += `>\n> **Commentaire :** ${a.comment}\n`;
  }
  return s + "\n<!-- zotero:fin -->";
}

// ------------------------------------------------------------------- IA

const mockKeys = new Set<string>();
let aiCancelled = false;

/** Mock generation: text sent in small pieces, like the real stream. */
export async function mockAiFiche(key: string, onDelta: (d: string) => void): Promise<AiFicheDraft> {
  const it = zitems.find((i) => i.key === key);
  if (!it) throw "Introuvable";
  if (!mockKeys.has(settings.ai_provider) && settings.ai_provider !== "compatible") throw "IA : aucune clé API enregistrée";
  aiCancelled = false;
  const text = `### Problématique\n\nComment **détecter la fraude** dans « ${it.title} » ?\n\n### Méthodes\n\n\`\`\`mermaid\nflowchart LR\n  A[Transactions] --> B[Graphe] --> C[GNN] --> D[Score]\n\`\`\`\n\n### Commentaire\n\n==Étude solide== (démonstration).`;
  for (let i = 0; i < text.length; i += 12) {
    if (aiCancelled) throw "IA : annulé";
    onDelta(text.slice(i, i + 12));
    await sleep(8);
  }
  return {
    item_key: it.key, citekey: it.citekey, tags: ["gnn", "fraude"], model: "claude-opus-5-5", input_tokens: 41200, output_tokens: 2300,
    block: `<!-- eloria:ia:debut -->\n*Fiche rédigée par l'IA (démonstration).*\n\n${text.replaceAll("### ", "## ")}\n\n## Meilleures références citées\n\n- *Graph survey* [@lundberg2017unified]\n<!-- eloria:ia:fin -->`,
    source: "pdf", refs_in_library: 1, refs_in_article: 3, refs_unverified: 1, fell_back: false, truncated: false,
  };
}

let semanticDone = false;
export async function mockSemanticIndex(onProgress: (n: number) => void) {
  const n = files.size;
  for (let i = 8; i < n; i += 8) {
    onProgress(i);
    await sleep(5);
  }
  onProgress(n);
  semanticDone = true;
  return n;
}

/** Mock rewriting: keeps citations, as the core requires. */
export async function mockAiRewrite(task: string, text: string, onDelta: (d: string) => void) {
  if (!mockKeys.has(settings.ai_provider) && settings.ai_provider !== "compatible") throw "IA : aucune clé API enregistrée";
  aiCancelled = false;
  const out = task === "summarize" ? `En bref : ${text.slice(0, 120).trim()}…` : task === "fix" ? text.replace(/ ,/g, ",") : `${text.trim()} (version « ${task} »)`;
  for (let i = 0; i < out.length; i += 10) {
    if (aiCancelled) throw "IA : annulé";
    onDelta(out.slice(i, i + 10));
    await sleep(5);
  }
  return { text: out, model: "claude-opus-5-5", input_tokens: 800, output_tokens: 120 };
}

/** Mock Ask Eloria: excerpts of the demo vault, sourced answer. */
export async function mockAiAsk(question: string, onDelta: (d: string) => void, mode = "") {
  if (!mockKeys.has(settings.ai_provider) && settings.ai_provider !== "compatible") throw "IA : aucune clé API enregistrée";
  aiCancelled = false;
  const passages = [
    { id: 1, path: "Sources/Thèse/Ch2 État de l'art/@smith2024graph.md", title: "Graph Neural Networks for Mobile Money Fraud Detection", citekey: "smith2024graph", zotero: "ITEMAAAA", text: "" },
    { id: 2, path: "Concepts/SHAP.md", title: "SHAP", citekey: "", zotero: "", text: "" },
  ];
  const prefix = mode === "contradictions" ? "**Divergence** : " : mode === "debat" ? "**Position A** : " : mode.startsWith("custom:") ? "**Relecture** : " : "";
  const text = `${prefix}Selon tes lectures, les réseaux de fraude forment des sous-graphes denses [S1]. Pour expliquer les scores, SHAP attribue la contribution de chaque variable [S2].\n\nAnalyse : combiner les deux répond à « ${question} ». Une étude [S7] serait à vérifier.`;
  for (let i = 0; i < text.length; i += 12) {
    if (aiCancelled) throw "IA : annulé";
    onDelta(text.slice(i, i + 12));
    await sleep(5);
  }
  return { text, passages, unverified: ["[S7]"], model: "claude-opus-5-5", input_tokens: 5200, output_tokens: 180 };
}

// ------------------------------------------------------------- dispatch

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export async function mockCall(cmd: string, a: Record<string, any>): Promise<unknown> {
  await sleep(5);
  switch (cmd) {
    case "get_settings":
    case "default_settings":
      return structuredClone(settings);
    case "save_settings":
      settings = { ...a.settings };
      localStorage.setItem("eloria:demo-lang", settings.language);
      return null;
    case "vault_info":
      // ?accueil in the URL to see the welcome screen
      if (location.search.includes("accueil") && !opened) return null;
      return { root: "/demo/Thèse", name: "Thèse" };
    case "open_vault":
      opened = true;
      return { root: "/demo/Thèse", name: "Thèse" };
    case "close_vault":
      return null;
    case "vault_tree":
      return tree();
    case "list_folders":
      return [...new Set([...files.keys()].filter((p) => p.includes("/")).map((p) => p.slice(0, p.lastIndexOf("/"))))].sort();
    case "all_notes":
      return [...files.keys()].map(meta).sort((x, y) => x.title.localeCompare(y.title, "fr"));
    case "vault_stats": {
      const all = [...files.keys()].map(meta);
      return { notes: all.length, words: all.reduce((s, n) => s + n.words, 0), fiches: all.filter((n) => n.frontmatter.type === "fiche").length, tags: 0 };
    }
    case "rebuild_index":
      return { indexed: files.size, removed: 0, total: files.size };
    case "read_note":
      if (!files.has(a.path)) throw `Introuvable : ${a.path}`;
      return { path: a.path, content: files.get(a.path), mtime: mtimes.get(a.path) };
    case "save_note":
      files.set(a.path, a.content);
      mtimes.set(a.path, Date.now());
      return Date.now();
    case "create_note": {
      let base = a.folder ? `${a.folder}/${a.name}` : a.name;
      let p = `${base}.md`;
      let i = 1;
      while (files.has(p)) p = `${base} ${i++}.md`;
      files.set(p, a.content ?? "");
      return p;
    }
    case "create_from_link": {
      const r = resolve(a.target);
      if (r) return r;
      const p = a.target.includes("/") ? `${a.target}.md` : `${a.from.includes("/") ? a.from.slice(0, a.from.lastIndexOf("/")) + "/" : ""}${a.target}.md`;
      files.set(p, `# ${a.target.split("/").pop()}\n\n`);
      return p;
    }
    case "create_folder":
      folders.add(a.path);
      return null;
    case "delete_path":
      for (const p of [...files.keys()]) if (p === a.path || p.startsWith(a.path + "/")) files.delete(p);
      folders.delete(a.path);
      return null;
    case "rename_path": {
      const updated: string[] = [];
      for (const p of [...files.keys()]) {
        if (p === a.from || p.startsWith(a.from + "/")) {
          const np = a.to + p.slice(a.from.length);
          files.set(np, files.get(p)!);
          files.delete(p);
        }
      }
      if (a.updateLinks && a.from.endsWith(".md")) {
        const o = stemOf(a.from), n = stemOf(a.to);
        for (const [p, c] of files) {
          const nc = c.replace(new RegExp(`\\[\\[${o.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}([#|\\]])`, "gi"), `[[${n}$1`);
          if (nc !== c) { files.set(p, nc); updated.push(p); }
        }
      }
      return { path: a.to, updated };
    }
    case "set_property": {
      const c = files.get(a.path) ?? "";
      const [fm, body] = splitFm(c);
      if (a.value === null) delete fm[a.key];
      else fm[a.key] = a.value;
      const yaml = Object.entries(fm).map(([k, v]) => `${k}: ${Array.isArray(v) ? `[${v.join(", ")}]` : v}`).join("\n");
      const nc = `---\n${yaml}\n---\n${body}`;
      files.set(a.path, nc);
      return nc;
    }
    case "save_attachment":
      return `${a.folder}/${a.name}`;
    case "search": {
      const q = String(a.query).toLowerCase().trim();
      if (!q) return [];
      const tagQ = q.startsWith("#") ? q.slice(1) : null;
      const hits = [];
      for (const p of files.keys()) {
        const m = meta(p);
        const c = files.get(p)!;
        if (tagQ) {
          if (m.tags.some((t) => t.toLowerCase() === tagQ || t.toLowerCase().startsWith(tagQ + "/"))) hits.push({ path: p, title: m.title, snippet: "" });
          continue;
        }
        const i = c.toLowerCase().indexOf(q);
        if (i >= 0) {
          const s = Math.max(0, i - 60);
          const snip = c.slice(s, i) + "\u0001" + c.slice(i, i + q.length) + "\u0002" + c.slice(i + q.length, i + q.length + 80);
          hits.push({ path: p, title: m.title, snippet: (s > 0 ? "…" : "") + snip.replace(/\n/g, " ") + "…" });
        }
      }
      return hits.slice(0, a.limit ?? 50);
    }
    case "backlinks": {
      const out = [];
      for (const p of files.keys()) {
        if (p === a.path) continue;
        for (const l of links(p)) {
          if (resolve(l.target) === a.path) out.push({ path: p, title: meta(p).title, line: l.line, context: files.get(p)!.split("\n")[l.line].trim() });
        }
      }
      return out;
    }
    case "unlinked_mentions":
      return [];
    case "outgoing":
      return links(a.path).map((l) => ({ target: l.target, resolved: resolve(l.target), line: l.line }));
    case "tags": {
      const counts = new Map<string, number>();
      for (const p of files.keys()) for (const t of meta(p).tags) counts.set(t, (counts.get(t) ?? 0) + 1);
      return [...counts.entries()].sort((x, y) => x[0].localeCompare(y[0], "fr"));
    }
    case "resolve_link":
      return resolve(a.target);
    case "graph":
      return graph(a.options);
    case "local_graph": {
      const g = graph({ include_tags: a.includeTags, include_orphans: true, include_missing: false, folder: null });
      const keep = new Set([a.path]);
      let frontier = [a.path as string];
      for (let d = 0; d < a.depth; d++) {
        const next: string[] = [];
        for (const e of g.edges) {
          for (const [from, to] of [[e.source, e.target], [e.target, e.source]]) {
            if (frontier.includes(from) && !from.startsWith("tag:") && !keep.has(to)) {
              keep.add(to);
              next.push(to);
            }
          }
        }
        frontier = next;
      }
      return { nodes: g.nodes.filter((n) => keep.has(n.id)), edges: g.edges.filter((e) => keep.has(e.source) && keep.has(e.target)) };
    }
    case "history_list":
      return [];
    case "history_read":
      return "";
    case "history_snapshot":
      return "demo";
    case "templates_list":
      return [...files.keys()].filter((p) => p.startsWith("Modèles/"));
    case "template_render":
      return (files.get(a.path) ?? "").replaceAll("{{title}}", a.title).replaceAll("{{date}}", new Date().toISOString().slice(0, 10));
    case "daily_note": {
      const p = `Journal/${new Date().toISOString().slice(0, 10)}.md`;
      if (!files.has(p)) files.set(p, `# ${new Date().toLocaleDateString("fr-FR", { dateStyle: "full" })}\n\n`);
      return p;
    }
    case "zotero_import_preview": {
      const inColl = (it: ZItem) => !a.collection || it.collections.includes(a.collection) || (a.collection === "C1" && it.collections.some((c) => c === "C2" || c === "C3"));
      const list = zitems.filter(inColl);
      const existing = list.filter((it) => sourceOf(it)).length;
      return { total: list.length, existing, new: list.length - existing };
    }
    case "zotero_import_all": {
      let created = 0;
      let skipped = 0;
      for (const it of zitems) {
        if (a.collection && !it.collections.includes(a.collection)) continue;
        if (sourceOf(it)) {
          skipped++;
          continue;
        }
        const coll = a.byCollection && it.collections[0] === "C3" ? "/Thèse/XAI" : "";
        makeSource(it, `Sources${coll}`);
        created++;
      }
      return { created, skipped, pdfs: settings.copy_pdfs ? created : 0, updated: 0, failed: [] };
    }
    case "zotero_pdf_path": {
      // The vault copy first (like the core): annotatable in Eloria.
      const it = zitems.find((i) => i.key === a.key);
      const src = it ? sourceOf(it) : undefined;
      const local = src ? meta(src).frontmatter.pdf : undefined;
      return typeof local === "string" ? `/demo/Thèse/${local}` : `/demo/zotero/${a.key}.pdf`;
    }
    case "pdf_annotations":
      return pdfAnns[a.path] ?? [];
    case "save_pdf_annotations": {
      pdfAnns[a.path] = a.list;
      const srcs = [...files.keys()].filter((p) => meta(p).frontmatter.pdf === a.path);
      for (const p of srcs) {
        const block = `<!-- eloria:annotations:debut -->\n## Annotations dans Eloria\n\n${(a.list as { page: number; text: string; comment: string }[]).map((x) => `> [!citation|jaune] p. ${x.page}\n> ${x.text}${x.comment ? `\n>\n> **Commentaire :** ${x.comment}` : ""}\n`).join("\n")}<!-- eloria:annotations:fin -->`;
        const c = files.get(p)!;
        files.set(p, c.includes("<!-- eloria:annotations:debut -->") ? c.replace(/<!-- eloria:annotations:debut -->[\s\S]*<!-- eloria:annotations:fin -->/, block) : `${c.trimEnd()}\n\n${block}\n`);
      }
      return srcs;
    }
    case "ai_key_present":
      return mockKeys.has(a.provider);
    case "ai_set_key":
      if (a.key) mockKeys.add(a.provider);
      else mockKeys.delete(a.provider);
      return null;
    case "ai_test":
      if (!mockKeys.has(settings.ai_provider)) throw "IA : aucune clé API enregistrée";
      return { text: "Bonjour !", model: "claude-opus-5-5", input_tokens: 30, output_tokens: 5, stop_reason: "end_turn", fell_back: false, truncated: false };
    case "ai_cancel":
      aiCancelled = true;
      return null;
    case "ai_fiche_save": {
      const it = zitems.find((i) => i.key === a.draft.item_key)!;
      const p: string = a.path || makeFiche(it, sourceOf(it) ?? makeSource(it));
      const c = files.get(p)!;
      const next = c.includes("<!-- eloria:ia:debut -->")
        ? c.replace(/<!-- eloria:ia:debut -->[\s\S]*<!-- eloria:ia:fin -->/, a.draft.block)
        : c + "\n" + a.draft.block + "\n";
      files.set(p, next);
      return p;
    }
    case "ai_journal":
      return [];
    case "zotero_detect":
      return { data_dir: "C:/Users/demo/Zotero", api: false };
    case "zotero_sync":
    case "zotero_status":
      return { items: zitems.length, annotations: zann.length, collections: 3, last_sync: new Date().toISOString(), source: "démo" };
    case "zotero_collections":
      return [
        { key: "C1", name: "Thèse", parent: null, item_count: 1 },
        { key: "C2", name: "Ch2 État de l'art", parent: "C1", item_count: 1 },
        { key: "C3", name: "XAI", parent: "C1", item_count: 2 },
      ];
    case "zotero_items": {
      let list = zitems;
      if (a.collection) list = list.filter((i) => i.collections.includes(a.collection));
      if (a.query) {
        const q = String(a.query).toLowerCase();
        list = list.filter((i) => `${i.title} ${i.citekey} ${i.creators.map((c) => c.last_name).join(" ")} ${i.year}`.toLowerCase().includes(q));
      }
      return list.map(summary);
    }
    case "search_works":
    case "lookup_doi": {
      const list = DEMO_WORKS.filter((w) => !a.query || /10\./.test(a.query) || w.title.toLowerCase().includes(String(a.query).toLowerCase().split(" ")[0]));
      return cmd === "lookup_doi" ? (DEMO_WORKS[0] ?? null) : list.length ? list : DEMO_WORKS;
    }
    case "similar_works":
      return DEMO_WORKS.slice(1);
    case "add_reference": {
      const w = a.work as Work;
      for (const p of files.keys()) {
        const fm = meta(p).frontmatter;
        if (fm.type === "source" && w.doi && String(fm.doi ?? "").toLowerCase() === w.doi) return { path: p, created: false };
      }
      const ck = `${(w.authors[0]?.[1] ?? "anonyme").toLowerCase().normalize("NFD").replace(/[^a-z]/g, "")}${w.year ?? ""}${w.title.toLowerCase().split(/\W+/).find((x) => x.length > 3) ?? ""}`;
      const p = `Sources/@${ck}.md`;
      const auteurs = w.authors.map(([f, l]) => `"${f ? `${f} ${l}` : l}"`).join(", ");
      files.set(p, `---
type: source
titre: "${w.title.replace(/"/g, "'")}"
auteurs: [${auteurs}]
annee: ${w.year ?? ""}
type_document: ${w.item_type}
publication: "${w.venue}"
doi: ${w.doi}
citekey: ${ck}
${w.abstract_note ? `resume: "${w.abstract_note.replace(/"/g, "'")}"\n` : ""}tags: [source${(w.keywords ?? []).map((k) => `, ${k.toLowerCase().replace(/\s+/g, "-")}`).join("")}]
---
# ${w.title}
`);
      return { path: p, created: true };
    }
    case "complete_source": {
      const c = files.get(String(a.path)) ?? "";
      if (/^resume:/m.test(c)) return false;
      files.set(String(a.path), c.replace(/^type: source$/m, `type: source\nresume: "Résumé complété en ligne (démonstration)."`));
      return true;
    }
    case "complete_sources":
      return 0;
    case "reliability_of":
      return reliability.get(String(a.doi).toLowerCase()) ?? null;
    case "check_reliability": {
      const doi = String(a.doi).toLowerCase();
      const v = doi.includes("xfd")
        ? { level: "attention", checked: "2026-11-08", signals: [{ level: "attention", text: "Prépublication ou dépôt (arXiv) : pas d'évaluation par les pairs attestée. Cherche une version publiée." }, { level: "info", text: "31 citation(s)." }] }
        : { level: "ok", checked: "2026-11-08", signals: [{ level: "ok", text: "Revue établie : Journal of Financial Technology (h-index 61)." }, { level: "info", text: "14 citation(s)." }] };
      reliability.set(doi, v);
      return v;
    }
    case "check_reliability_all":
      reliability.set("10.1000/retracted", { level: "alerte", checked: "2026-11-08", signals: [{ level: "alerte", text: "Article rétracté (base Retraction Watch, via OpenAlex). Ne le cite pas comme preuve." }] });
      return 1;
    case "reliability_report": {
      const rows = [...files.keys()]
        .map((p) => ({ p, fm: meta(p).frontmatter }))
        .filter(({ fm }) => fm.type === "source" && fm.doi && reliability.has(String(fm.doi).toLowerCase()))
        .map(({ p, fm }) => {
          const v = reliability.get(String(fm.doi).toLowerCase())!;
          return { citekey: String(fm.citekey ?? ""), key: "", title: String(fm.titre ?? ""), path: p, doi: String(fm.doi), level: v.level, signals: v.signals, cited: false };
        });
      const r = reliability.get("10.1000/retracted");
      if (r) rows.unshift({ citekey: "wakefield1998", key: "", title: "Ileal-lymphoid-nodular hyperplasia (rétracté)", path: "", doi: "10.1000/retracted", level: r.level, signals: r.signals, cited: true });
      return [rows, r ? 0 : 1];
    }
    case "stage_pdf":
      return `/demo/${String(a.name ?? "document.pdf")}`;
    case "add_pdf": {
      const name = String(a.path).split("/").pop()!.replace(/\.pdf$/i, "");
      const ck = name.toLowerCase().normalize("NFD").replace(/[^a-z0-9]/g, "").slice(0, 24) || "document";
      const p = `Sources/@${ck}.md`;
      if (files.has(p)) return { path: p, created: false, pdf: `PDF/@${ck}.pdf`, doi: "" };
      files.set(p, `---
type: source
titre: "${name.replace(/[_-]+/g, " ")}"
auteurs: []
citekey: ${ck}
pdf: PDF/@${ck}.pdf
tags: [source]
---
# ${name.replace(/[_-]+/g, " ")}
`);
      return { path: p, created: true, pdf: `PDF/@${ck}.pdf`, doi: "" };
    }
    case "store_pdf_text":
      pdfText.set(String(a.path), String(a.text));
      return [];
    case "has_pdf_text":
      return pdfText.has(String(a.path));
    case "pdfs_without_text":
      return [];
    case "search_pdf_text": {
      const q = String(a.query).toLowerCase();
      const out: [string, string, string][] = [];
      for (const [pdf, text] of pdfText) {
        const i = text.toLowerCase().indexOf(q);
        const src = [...files.keys()].find((k) => meta(k).frontmatter.pdf === pdf);
        if (i >= 0 && src) out.push([src, pdf, text.slice(Math.max(0, i - 60), i + q.length + 60)]);
      }
      return out;
    }
    case "source_detail": {
      const fm = meta(a.path).frontmatter;
      const z = zitems.find((i) => i.key === fm.zotero);
      if (z) return { item: z, annotations: zann.filter((x) => x.item_key === z.key) } satisfies ItemDetail;
      const names = (Array.isArray(fm.auteurs) ? fm.auteurs : []).map(String);
      const item: ZItem = {
        key: "", item_type: String(fm.type_document ?? "journalArticle"), title: String(fm.titre ?? ""),
        creators: names.map((n) => { const i = n.lastIndexOf(" "); return au(i > 0 ? n.slice(0, i) : "", i > 0 ? n.slice(i + 1) : n); }),
        date: String(fm.annee ?? ""), year: Number(fm.annee) || null, publication: String(fm.publication ?? ""), doi: String(fm.doi ?? ""),
        url: "", abstract_note: String(fm.resume ?? ""), citekey: String(fm.citekey ?? ""), tags: [], collections: [], attachments: [],
        fields: {}, date_added: "", date_modified: "",
      };
      return { item, annotations: [] } satisfies ItemDetail;
    }
    case "backup_now":
      if (!a.dir && !settings.backup_dir) throw "Choisis d'abord un dossier de sauvegarde (Réglages › Fichiers).";
      demoBackups.unshift({ path: `${a.dir || settings.backup_dir}/Eloria-Thèse-demo-${demoBackups.length}.zip`, name: `Eloria-Thèse-demo-${demoBackups.length}.zip`, bytes: 52_000, created: new Date().toISOString().slice(0, 19).replace("T", " "), files: files.size });
      return demoBackups[0];
    case "backup_list":
      return demoBackups;
    case "backup_restore":
      return files.size;
    case "check_integrity": {
      const broken: { path: string; line: number; detail: string }[] = [];
      for (const [p, c] of files) {
        c.split("\n").forEach((l, i) => {
          // Like the core: inline code is not a link.
          for (const m of l.replace(/`[^`]*`/g, (s) => " ".repeat(s.length)).matchAll(/(?<!!)\[\[([^\]|#]+)\]\]/g)) {
            const target = m[1].trim();
            if (!p.startsWith("Modèles/") && target && !target.includes("{{") && ![...files.keys()].some((f) => f.split("/").pop()!.replace(/\.md$/, "").toLowerCase() === target.toLowerCase()))
              broken.push({ path: p, line: i + 1, detail: target });
          }
        });
      }
      return { notes: files.size, broken_links: broken, missing_files: [], bad_frontmatter: [], orphan_sources: [], fiches_without_source: [] };
    }
    case "ai_summary_callout":
      return `> [!résumé] Résumé (IA, relu)\n${String(a.summary).split("\n").map((l: string) => `> ${l}`).join("\n")}\n`;
    case "ai_answer_markdown":
      return String(a.text).replace(/\[S(\d+)\]/g, (m: string, n: string) => {
        const p = (a.passages as { id: number; citekey: string; path: string }[]).find((x) => x.id === Number(n));
        return p ? (p.citekey ? `[@${p.citekey}]` : `[[${p.path.split("/").pop()!.replace(/\.md$/, "")}]]`) : m;
      });
    case "create_review": {
      const p = `Revues/${a.title}.md`;
      files.set(p, `---\ntype: revue\nquestion: "${a.question}"\ninclusion: []\nexclusion: []\narticles: []\ntags: [revue]\n---\n# ${a.title}\n`);
      reviews[p] = [];
      return p;
    }
    case "review_set_article": {
      const list = (reviews[a.path] ??= []);
      const i = list.findIndex((x) => x.citekey.toLowerCase() === String(a.citekey).toLowerCase());
      if (a.statut === "") list.splice(i, 1);
      else if (i >= 0) list[i].statut = a.statut;
      else list.push({ citekey: a.citekey, statut: a.statut });
      return mockCall("review", { path: a.path });
    }
    case "review": {
      const fm = meta(a.path).frontmatter;
      return {
        path: a.path, title: meta(a.path).title, question: String(fm.question ?? ""), inclusion: [], exclusion: [],
        articles: (reviews[a.path] ?? []).map((x) => {
          const it = zitems.find((i) => i.citekey.toLowerCase() === x.citekey.toLowerCase());
          const fiches = [...files.keys()].filter((p) => meta(p).frontmatter.type === "fiche" && meta(p).frontmatter.citekey === x.citekey);
          return { ...x, title: it?.title ?? `@${x.citekey}`, author_year: it ? `${summary(it).authors} (${it.year})` : "", source: it ? (sourceOf(it) ?? "") : "", fiches };
        }),
      };
    }
    case "source_citations":
      return {
        info: {
          openalex: "W1", cited_by_count: 412, reference_dois: ["10.1000/xfd.2024"],
          references: [DEMO_WORKS[1], { ...DEMO_WORKS[2], doi: "10.5555/shap" }],
          cited_by: [DEMO_WORKS[0]],
        },
        library: { "10.5555/shap": "lundberg2017unified" },
      };
    case "citation_network":
    case "citation_fetch":
      return {
        nodes: [
          { citekey: "smith2024graph", title: "Graph Neural Networks for Mobile Money Fraud Detection", year: 2024, label: "Smith & Ouédraogo 2024", fetched: true },
          { citekey: "lundberg2017unified", title: "A Unified Approach to Interpreting Model Predictions", year: 2017, label: "Lundberg & Lee 2017", fetched: true },
          { citekey: "ribeiro2016why", title: "Why Should I Trust You?", year: 2016, label: "Ribeiro et al. 2016", fetched: cmd === "citation_fetch" },
        ],
        edges: cmd === "citation_fetch" ? [["smith2024graph", "lundberg2017unified"], ["lundberg2017unified", "ribeiro2016why"]] : [["smith2024graph", "lundberg2017unified"]],
        missing: cmd === "citation_fetch" ? [] : ["10.1145/2939672.2939778"],
      };
    case "note_to_latex": {
      const p = `${String(a.path).replace(/[^/]+$/, "")}LaTeX/${String(a.path).split("/").pop()!.replace(/\.md$/, "")}.tex`;
      files.set(p, "\\documentclass{article}\n\\addbibresource{references.bib}\n\\begin{document}\n\\section{Démo}\nTexte \\autocite{smith2024graph}.\n\\printbibliography\n\\end{document}\n");
      return p;
    }
    case "assemble_thesis":
      files.set("Thèse/Manuscrit/main.tex", "\\documentclass{report}\n\\addbibresource{references.bib}\n\\begin{document}\n\\include{ch1}\n\\printbibliography\n\\end{document}\n");
      return "Thèse/Manuscrit/main.tex";
    case "submission_checklist":
      return [
        { label: "Citations présentes dans la bibliothèque", ok: true, detail: "3" },
        { label: "Aucune marque « TODO » restante", ok: false, detail: "l. 12" },
        { label: "Bibliographie imprimée", ok: true, detail: "" },
      ];
    case "add_focus":
      demoFocus += a.minutes;
      return demoFocus;
    case "schedule_review": {
      const c = files.get(a.path)!;
      const date = new Date(Date.now() + a.days * 864e5).toISOString().slice(0, 10);
      files.set(a.path, a.days > 0 ? c.replace(/^---\n/, `---\nrevoir_le: ${date}\n`) : c.replace(/^revoir_le: .*\n/m, ""));
      return a.days > 0 ? date : null;
    }
    case "weekly_review": {
      const p = "Journal/Revue de la semaine 2026-S45.md";
      if (!files.has(p)) files.set(p, "---\ntype: revue-hebdo\n---\n# Revue de la semaine 2026-S45\n\n## En chiffres\n\n- Mots écrits : **1370**\n");
      return p;
    }
    case "local_api_enable":
      settings = { ...settings, local_api: a.enabled, local_api_token: settings.local_api_token || "demo0123456789abcdef0123456789ab" };
      return a.enabled ? `javascript:(()=>{fetch('http://127.0.0.1:27123/capture',{method:'POST',headers:{'Authorization':'Bearer ${settings.local_api_token}'}})})()` : null;
    case "mcp_info":
      return { exe: "C:\\Program Files\\Eloria\\eloria-mcp.exe", exists: true, config: '{\n  "mcpServers": {\n    "eloria": { "command": "C:\\\\Program Files\\\\Eloria\\\\eloria-mcp.exe", "args": ["/demo/Thèse"] }\n  }\n}' };
    case "capture": {
      const title = String(a.text).split("\n").map((l: string) => l.trim()).find(Boolean)?.replace(/[\\/:*?"<>|#]/g, "").split(/\s+/).slice(0, 9).join(" ") || "Capture";
      const p = `Boîte de réception/${title}.md`;
      files.set(p, `---\ntype: capture\ntags: [a-classer]\n---\n${a.text}\n`);
      mtimes.set(p, Date.now());
      return p;
    }
    case "inbox":
      return [...files.keys()].filter((p) => p.startsWith("Boîte de réception/")).map((p) => [p, meta(p).title, mtimes.get(p) ?? Date.now()]);
    case "import_markdown":
      files.set("Importé/export/Accueil.md", "# Accueil importé\n\n[[Plan]]\n");
      files.set("Importé/export/Plan.md", "# Plan\n");
      return { notes: 2, files: 0, links_rewritten: 1, skipped: 0, folder: "Importé/export" };
    case "git_status":
      return demoGit;
    case "git_init":
      demoGit = { ...demoGit, repo: true, branch: "main", changes: files.size };
      return demoGit;
    case "git_commit":
      demoGit = { ...demoGit, changes: 0, log: [["a1b2c3d", new Date().toISOString().slice(0, 16).replace("T", " "), a.message || "Eloria : version"], ...demoGit.log] };
      return demoGit;
    case "git_sync":
      return demoGit;
    case "reading_chain":
      return [];
    case "bib_audit":
      return [{ citekey: "bceao2023etat", title: "État de l'inclusion financière dans l'UEMOA", kind: "champs manquants", detail: "DOI" }];
    case "check_citations": {
      const keys = [...String(files.get(a.path) ?? "").matchAll(/@([A-Za-z0-9_]+)/g)].map((m) => m[1]);
      return [...new Set(keys)].map((k) => ({ key: k, lines: [1], known: zitems.some((i) => i.citekey === k) }));
    }
    case "style_check":
      return [{ line: 6, kind: "vague", excerpt: "très", message: "terme vague : préciser (chiffre, exemple, source)" }];
    case "orphan_notes":
      return ["Journal/2026-11-07.md"].filter((p) => files.has(p));
    case "duplicate_notes":
      return [];
    case "dashboard": {
      const fiches: Record<string, number> = { "à lire": 0, "en cours": 0, lue: 0, "exploitée": 0 };
      for (const p of files.keys()) {
        const fm = meta(p).frontmatter;
        if (fm.type === "fiche") fiches[String(fm.statut ?? "à lire")] = (fiches[String(fm.statut ?? "à lire")] ?? 0) + 1;
      }
      const days: [string, number][] = [...Array(14)].map((_, i) => [new Date(Date.now() - (13 - i) * 864e5).toISOString().slice(0, 10), [0, 120, 340, 0, 80, 610, 220][i % 7]]);
      return {
        notes: files.size, words: [...files.values()].reduce((n, c) => n + c.split(/\s+/).length, 0),
        sources: [...files.keys()].filter((p) => meta(p).frontmatter.type === "source").length, fiches, written: days, written_today: days[13][1],
        chapters: [...files.keys()].filter((p) => p.startsWith("Thèse/Ch")).map((p) => [p, meta(p).title, 1200, 10000]),
        to_read: [...files.keys()].filter((p) => meta(p).frontmatter.statut === "à lire").map((p) => [p, meta(p).title]),
        recent: [...files.keys()].slice(0, 5).map((p) => [p, meta(p).title, Date.now()]), ai_tokens: [5200, 180],
        fiches_read_week: 1, focus_today: demoFocus, focus_week: demoFocus + 50,
        reviews_due: [...files.keys()].filter((p) => /revoir_le: /.test(files.get(p)!)).map((p) => [p, meta(p).title, "2026-11-08"]),
      };
    }
    case "ai_concepts":
      return [
        { name: "Compte mule", definition: "Compte qui fait transiter des fonds frauduleux." },
        { name: "Sous-graphe dense", definition: "Groupe de comptes très reliés entre eux." },
      ];
    case "ai_tags":
      return ["fraude", "graphes", "detection-temps-reel"];
    case "create_concept_note": {
      const p = `Concepts/${a.name}.md`;
      if (files.has(p)) return { path: p, created: false };
      files.set(p, `---\ntype: concept\ntags: [concept]\n---\n# ${a.name}\n\n> [!definition] Définition\n> ${a.definition}\n`);
      return { path: p, created: true };
    }
    case "semantic_status":
      return settings.embed_provider
        ? { configured: true, provider: settings.embed_provider, model: settings.embed_model || "nomic-embed-text", key_present: true, stale: semanticDone ? 0 : files.size }
        : { configured: false, provider: "", model: "", key_present: false, stale: 0 };
    case "semantic_set_key":
      return null;
    case "semantic_search": {
      // Mock: notes sharing a word with the question, closest first.
      const words = String(a.query).toLowerCase().split(/\W+/).filter((w) => w.length > 3);
      return [...files.keys()]
        .map((p) => ({ path: p, title: meta(p).title, score: words.filter((w) => files.get(p)!.toLowerCase().includes(w.slice(0, 5))).length / Math.max(1, words.length) }))
        .filter((h) => h.score > 0)
        .sort((x, y) => y.score - x.score)
        .slice(0, a.limit ?? 30);
    }
    case "latex_engine":
      // Mock: an engine exists as soon as a path is configured (Settings › Export).
      return settings.latex_path ? { kind: "tectonic", path: settings.latex_path, version: "démo" } : null;
    case "latex_install_tectonic":
      throw "Réseau : indisponible en mode démonstration";
    case "latex_templates":
      return [["article", "Article"], ["chapitre", "Chapitre de thèse"], ["beamer", "Présentation (Beamer)"]];
    case "latex_new": {
      const p = `${a.folder}/${a.name}.tex`;
      files.set(p, "\\documentclass{report}\n\\addbibresource{references.bib}\n\\begin{document}\n\\chapter{État de l'art}\\label{ch:etat}\nTexte \\autocite{cle}.\n\\end{document}\n");
      return p;
    }
    case "latex_compile":
      return { ok: false, pdf: null, engine: "tectonic", issues: [{ file: "Chapitre.tex", line: 5, message: "Undefined control sequence.", level: "error" }], log: "", millis: 900 };
    case "read_text":
      return files.get(a.path) ?? "";
    case "write_text":
      files.set(a.path, a.content);
      return null;
    case "research_matrix": {
      const want = a.citekeys ? new Set((a.citekeys as string[]).map((k) => k.toLowerCase())) : null;
      const cols: [string, string][] = [["problematique", "Problématique"], ["objectifs", "Objectifs"], ["methodes", "Méthodes"], ["donnees", "Données"], ["resultats", "Résultats"], ["limites", "Limites"], ["defis", "Défis"], ["perspectives", "Perspectives"], ["apport", "Apport"]];
      const names: Record<string, string[]> = { problematique: ["problématique"], objectifs: ["objectifs"], methodes: ["méthodes"], donnees: ["sources de données"], resultats: ["résultats"], limites: ["contraintes rencontrées par les auteurs"], defis: ["défis"], perspectives: ["perspectives"], apport: ["commentaire"] };
      const rows = [...files.keys()]
        .filter((p) => meta(p).frontmatter.type === "fiche")
        .filter((p) => !want || want.has(String(meta(p).frontmatter.citekey ?? "").toLowerCase()))
        .map((p) => {
          const fm = meta(p).frontmatter;
          const body = splitFm(files.get(p)!)[1];
          const secs: Record<string, string> = {};
          for (const m of body.matchAll(/^## (.+)\n([\s\S]*?)(?=^## |^<!--|(?![\s\S]))/gm)) secs[m[1].trim().toLowerCase()] = m[2].trim().replace(/\n+/g, " ; ");
          const it = zitems.find((i) => i.citekey === fm.citekey);
          return {
            path: p, citekey: String(fm.citekey ?? ""), title: it?.title ?? meta(p).title, author_year: it ? `${summary(it).authors} (${it.year})` : meta(p).title,
            year: it?.year ?? null, statut: String(fm.statut ?? ""), cells: cols.map(([id]) => names[id].map((n) => secs[n]).find(Boolean) ?? ""),
          };
        });
      return { columns: cols, rows };
    }
    case "export_matrix": {
      const p = `Matrices/Matrice de recherche.${a.format}`;
      files.set(p, "# Matrice de recherche\n");
      return p;
    }
    case "embed": {
      const [name, heading] = String(a.target).split("#");
      const p = [...files.keys()].find((f) => f.split("/").pop()!.replace(/\.md$/, "").toLowerCase() === name.trim().toLowerCase());
      if (!p) return null;
      const body = splitFm(files.get(p)!)[1];
      let lines = body.split("\n");
      if (heading) {
        const i = lines.findIndex((l) => /^#{1,6} /.test(l) && l.replace(/^#+ /, "").trim().toLowerCase() === heading.trim().toLowerCase());
        if (i < 0) return null;
        const level = lines[i].match(/^#+/)![0].length;
        const j = lines.findIndex((l, k) => k > i && /^#{1,6} /.test(l) && l.match(/^#+/)![0].length <= level);
        lines = lines.slice(i, j < 0 ? undefined : j);
      }
      const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
      const html = lines
        .filter((l) => l.trim() && !l.startsWith("<!--"))
        .map((l) => {
          const h = /^(#{1,6}) (.*)$/.exec(l);
          return h ? `<h${h[1].length}>${esc(h[2])}</h${h[1].length}>` : `<p>${esc(l.replace(/!?\[\[([^\]|]+)\|?([^\]]*)\]\]/g, (_m, t, al) => al || t))}</p>`;
        })
        .join("");
      return { path: p, title: meta(p).title, html };
    }
    case "fiches_of_source": {
      const stemS = String(a.path).split("/").pop()!.replace(/\.md$/, "").toLowerCase();
      const zk = meta(a.path).frontmatter.zotero;
      return [...files.keys()].filter((p) => {
        const fm = meta(p).frontmatter;
        return fm.type === "fiche" && ((zk && fm.zotero === zk) || String(fm.source ?? "").replace(/[[\]"]/g, "").split("|")[0].toLowerCase() === stemS);
      }).sort();
    }
    case "knowledge_map": {
      const kinds = a.withSources ? ["concept", "methode", "question", "chapitre", "argument", "source", "fiche"] : ["concept", "methode", "question", "chapitre", "argument"];
      const keep = [...files.keys()].filter((p) => p.endsWith(".md") && !p.startsWith("Modèles/") && kinds.includes(String(meta(p).frontmatter.type ?? "")));
      const stemOf = (q: string) => q.split("/").pop()!.replace(/\.md$/, "");
      const edges: [string, string, string][] = [];
      const seen = new Set<string>();
      for (const p of keep) {
        for (const q of keep) {
          if (p === q) continue;
          const key = [p, q].sort().join("|");
          if (seen.has(key)) continue;
          const c = files.get(p)!;
          if (c.includes(`[[${stemOf(q)}`)) { edges.push([p, q, "lien"]); seen.add(key); }
          else if (meta(q).frontmatter.type === "concept" && c.toLowerCase().includes(meta(q).title.toLowerCase())) { edges.push([p, q, "mention"]); seen.add(key); }
        }
      }
      const deg = (id: string) => edges.filter((e) => e[0] === id || e[1] === id).length;
      return { nodes: keep.map((p) => ({ id: p, title: meta(p).title, kind: String(meta(p).frontmatter.type), degree: deg(p) })), edges };
    }
    case "taxonomy_create":
      if (!files.has("Taxonomie.md")) files.set("Taxonomie.md", "---\ntype: taxonomie\n---\n# Taxonomie\n\n- thèse\n  - méthodo\n- xai (explicabilité, shap)\n- fraude (fraud, ingénierie sociale)\n");
      return "Taxonomie.md";
    case "taxonomy": {
      const c = files.get("Taxonomie.md");
      if (!c) return { path: "Taxonomie.md", exists: false, nodes: [] };
      const nodes: { label: string; tag: string; depth: number; terms: string[]; notes: [string, string][] }[] = [];
      const stack: string[] = [];
      for (const line of c.split("\n")) {
        const m = /^(\s*)- (.+)$/.exec(line);
        if (!m) continue;
        const depth = Math.floor(m[1].length / 2);
        const [label, al] = m[2].split("(");
        const slug = label.trim().toLowerCase().normalize("NFD").replace(/[\u0300-\u036f]/g, "").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
        stack.length = depth;
        const tag = [...stack, slug].join("/");
        stack.push(slug);
        const terms = [label.trim(), ...(al ? al.replace(")", "").split(",").map((x) => x.trim()) : [])];
        const notes = [...files.keys()].filter((p) => (meta(p).tags ?? []).some((t: string) => { const l = t.toLowerCase().normalize("NFD").replace(/[\u0300-\u036f]/g, ""); return l === tag || l.startsWith(`${tag}/`); })).map((p) => [p, meta(p).title] as [string, string]);
        nodes.push({ label: label.trim(), tag, depth, terms, notes });
      }
      return { path: "Taxonomie.md", exists: true, nodes };
    }
    case "taxonomy_proposals":
      return files.has("Taxonomie.md")
        ? [...files.keys()].filter((p) => p !== "Taxonomie.md" && !p.startsWith("Modèles/") && /SHAP|explicabilit/i.test(files.get(p)!) && !(meta(p).tags ?? []).includes("xai")).map((p) => [p, meta(p).title, ["xai"]])
        : [];
    case "entities": {
      const cat: [string, string, RegExp][] = [
        ["organisation", "BCEAO", /\bBCEAO\b/], ["organisation", "UEMOA", /\bUEMOA\b/], ["personne", "Scott Lundberg", /Lundberg/], ["personne", "Marco Tulio Ribeiro", /Ribeiro/],
        ["methode", "SHAP", /\bSHAP\b/], ["methode", "LIME", /\bLIME\b/], ["methode", "Réseaux de neurones sur graphes (GNN)", /\bGNN|graph neural/i], ["technologie", "Python", /\bPython\b/], ["lieu", "Afrique de l'Ouest", /Afrique de l'Ouest|West Africa/i],
      ];
      return cat
        .map(([kind, name, re]) => ({ kind, name, notes: [...files.keys()].filter((p) => p.endsWith(".md") && !p.startsWith("Modèles/") && re.test(files.get(p)!)).map((p) => [p, meta(p).title]) }))
        .filter((r) => r.notes.length)
        .sort((x, y) => y.notes.length - x.notes.length);
    }
    case "topics": {
      const all = [...files.keys()].filter((p) => p.endsWith(".md") && !p.startsWith("Modèles/"));
      const groups: [string[], (p: string) => boolean][] = [
        [["explicabilité", "shap", "lime"], (p) => /SHAP|Explicab|lundberg|ribeiro/i.test(p)],
        [["fraude", "graphes", "transactions"], (p) => /fraude|smith|Ch4|Mobile/i.test(p)],
        [["thèse", "chapitre", "plan"], () => true],
      ];
      const used = new Set<string>();
      const k = Number(a.k) || groups.length;
      return groups.slice(0, Math.max(2, Math.min(k, groups.length))).map(([terms, test], i, arr) => {
        const notes = all.filter((p) => !used.has(p) && (i === arr.length - 1 || test(p)));
        notes.forEach((p) => used.add(p));
        return { terms, notes: notes.map((p, j) => [p, meta(p).title, Math.max(0.3, 0.95 - j * 0.07)]), tag: `thème/${terms[0]}` };
      }).filter((g) => g.notes.length);
    }
    case "concepts_overview":
      return [...files.keys()]
        .filter((p) => meta(p).frontmatter.type === "concept" && !p.startsWith("Modèles/"))
        .map((p) => {
          const m = meta(p);
          const name = m.title.toLowerCase();
          const mentions = [...files.keys()].filter((q) => q !== p && (files.get(q)!.toLowerCase().includes(name) || files.get(q)!.includes(`[[${p.split("/").pop()!.replace(/\.md$/, "")}`)));
          const body = files.get(p)!.split(/\n---\n/)[1] ?? "";
          const def = body.split("\n\n").map((x) => x.trim()).find((x) => x && !x.startsWith("#")) ?? "";
          return {
            path: p, title: m.title, aliases: [], definition: def.replace(/^>\s*(\[!\w+\]\s*)?/gm, "").replace(/\n/g, " ").trim(),
            sources: mentions.filter((q) => ["source", "fiche"].includes(String(meta(q).frontmatter.type))).length, notes: mentions.length, updated: "2026-11-08",
          };
        })
        .sort((a, b) => b.notes - a.notes);
    case "concept_detail": {
      const p = String(a.path);
      const m = meta(p);
      const name = m.title.toLowerCase();
      const stemOf = (q: string) => q.split("/").pop()!.replace(/\.md$/, "");
      const timeline = [...files.keys()]
        .filter((q) => q !== p && !q.startsWith("Fiches de connaissance/") && !q.startsWith("Modèles/") && (files.get(q)!.toLowerCase().includes(name) || files.get(q)!.includes(`[[${stemOf(p)}`)))
        .map((q, i) => {
          const fm = meta(q).frontmatter;
          // Like the core: body without properties, the whole sentence containing the name.
          const text = files.get(q)!.replace(/^---\n[\s\S]*?\n---\n/, "");
          const plain = (l: string) =>
            l.trim().replace(/^[>\s]+/, "").replace(/^\[![\w-]+\][+-]?\s*/, "").replace(/^[-*+]\s+/, "")
              .replace(/!?\[\[([^\]|#]*)(?:#[^\]|]*)?(?:\|([^\]]*))?\]\]/g, (_m, a: string, b?: string) => (b ?? a).trim())
              .replace(/\*\*|==|`/g, "");
          const sentence = text.split("\n")
            .filter((l) => l.trim() && !/^\s*(#|<!--|\||---|```)/.test(l))
            .flatMap((l) => plain(l).split(/(?<=[.?!])\s/))
            .find((s) => s.toLowerCase().includes(name)) ?? "";
          const idx = sentence ? 0 : -1;
          return {
            date: String(fm.date ?? fm.cree_le ?? `2026-0${(i % 9) + 1}-1${i % 9}`), path: q, title: meta(q).title, kind: String(fm.type ?? "note"),
            citekey: String(fm.citekey ?? ""), excerpt: idx >= 0 ? sentence.replace(/^[#>*\-\s]+/, "").slice(0, 220) : "", linked: text.includes(`[[${stemOf(p)}`),
          };
        })
        .sort((x, y) => x.date.localeCompare(y.date));
      const card = `Fiches de connaissance/${m.title}.md`;
      return {
        path: p, title: m.title, aliases: [], definition: "Définition de démonstration.", timeline,
        related: timeline.filter((x) => x.kind === "concept").map((x) => [x.path, x.title]),
        questions: timeline.filter((x) => x.kind === "question").map((x) => [x.path, x.title]),
        definitions: [{ date: "2026-03-01", definition: "Première définition, plus vague." }, { date: "2026-11-08", definition: "Définition de démonstration." }],
        card: files.has(card) ? card : "",
      };
    }
    case "knowledge_card": {
      const m = meta(a.path);
      const card = `Fiches de connaissance/${m.title}.md`;
      files.set(card, `---\ntype: fiche-connaissance\n---\n# ${m.title}\n\n<!-- eloria:carte:debut -->\n## Définition\n\nDéfinition de démonstration.\n\n## Points clés des sources\n\n## Évolution\n<!-- eloria:carte:fin -->\n`);
      return card;
    }
    case "research_stats": {
      const months = Array.from({ length: 12 }, (_, i) => {
        const d = new Date(Number(a.year), Number(a.month) - 12 + i, 1);
        return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
      });
      const tags = ["fraude", "xai", "mobile-money", "gnn", "thèse/méthodo"];
      return {
        citations_total: 7, documents_cited: 4, documents_total: 4,
        top_documents: [["smith2024graph", "Graph Neural Networks for Mobile Money Fraud Detection", 3], ["lundberg2017unified", "A Unified Approach to Interpreting Model Predictions", 2], ["ribeiro2016why", "\"Why Should I Trust You?\"", 1], ["bceao2023etat", "État de l'inclusion financière dans l'UEMOA", 1]],
        top_authors: [["Smith", 3], ["Ouédraogo", 3], ["Lundberg", 2], ["Lee", 2], ["Ribeiro", 1]],
        top_venues: [["IEEE Access", 3], ["NeurIPS", 2], ["KDD", 1], ["BCEAO", 1]],
        years: [[2016, 1], [2017, 1], [2023, 1], [2024, 1]],
        heat_tags: tags, heat_months: months,
        heat: tags.map((_, i) => months.map((_, j) => (j >= 9 ? (i + j) % 4 : j % 5 === i ? 1 : 0))),
        cover_themes: ["gnn", "fraude", "xai", "mobile-money"], cover_chapters: ["ch2", "ch4"],
        cover: [[1, 1], [1, 0], [2, 1], [1, 0]],
        gaps: [["blockchain", 3]],
      };
    }
    case "import_bibtex":
      files.set("Sources/@demo2020bib.md", "---\ntype: source\ntitre: \"Importé de BibTeX\"\ncitekey: demo2020bib\ntags: [source]\n---\n# Importé de BibTeX\n");
      return { created: 1, existing: 0, failed: [] };
    case "disk_report": {
      const notes = [...files.keys()].filter((p) => p.endsWith(".md"));
      const size = notes.reduce((n, p) => n + files.get(p)!.length, 0);
      return {
        total: size + 5_400_000, files: notes.length + 4,
        by_type: [["PDF", 5_200_000, 3], ["notes", size, notes.length], ["auxiliaires LaTeX", 180_000, 6], ["images", 20_000, 1]],
        by_folder: [["PDF", 5_200_000, 3], ["Sources", Math.round(size / 2), 4], ["Thèse", 190_000, 8]],
        largest: [["PDF/Thèse/Ch2 État de l'art/@smith2024graph.pdf", 2_400_000], ["PDF/lundberg.pdf", 1_800_000]],
        internal: [["index.db", 900_000], ["history", 240_000], ["trash", trashBytes], ["pdftext", 60_000]],
        aux: auxFiles, aux_bytes: auxFiles.length ? 180_000 : 0, trash_bytes: trashBytes,
      };
    }
    case "clean_latex_aux": {
      const n = auxFiles.length;
      auxFiles = [];
      trashBytes += 180_000;
      return n;
    }
    case "empty_trash": {
      const b = trashBytes;
      trashBytes = 0;
      return b;
    }
    case "clear_temp":
      return 12_000;
    case "extensions_list":
      return extList();
    case "extension_enable": {
      const id = String(a.id);
      const info = extList().find((x) => x.id === id);
      if (!info) throw `Extension : identifiant d'extension invalide : ${id}`;
      if (a.on && info.error) throw info.error;
      settings = { ...settings, extension_grants: settings.extension_grants.filter((g) => g.id !== id) };
      if (a.on) settings.extension_grants.push({ vault: "/demo/Thèse", id, hash: info.hash });
      return extList();
    }
    case "extension_load": {
      const id = String(a.id);
      const info = extList().find((x) => x.id === id);
      if (!info?.enabled) throw `Extension : l'extension « ${info?.name ?? id} » n'est pas activée pour ce coffre`;
      const e = mockExt[id];
      return { id, name: e.name, version: e.version, code: e.code, css: e.css };
    }
    case "extension_data_get":
      return mockExt[String(a.id)]?.data ?? null;
    case "extension_data_set":
      if (!mockExt[String(a.id)]) throw `Extension : extension inconnue : ${a.id}`;
      mockExt[String(a.id)].data = a.value;
      return null;
    case "extension_create": {
      const id = String(a.name).normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
      if (id.length < 2) throw `Extension : nom d'extension invalide : « ${a.name} »`;
      if (mockExt[id]) throw `Extension : l'extension « ${id} » existe déjà`;
      mockExt[id] = { name: String(a.name).trim(), version: "0.1.0", description: "", author: "", code: "eloria.log('bonjour');", css: null, data: null };
      return `.eloria/extensions/${id}`;
    }
    case "formulas": {
      const c = files.get("Formules.md") ?? "";
      return [...c.matchAll(/## (.+)\n\n\$\$\n([\s\S]*?)\n\$\$/g)].map((m) => ({ name: m[1], latex: m[2], group: "Mes formules" }));
    }
    case "add_formula": {
      const old = files.get("Formules.md") ?? "---\ntype: formules\n---\n# Mes formules\n";
      files.set("Formules.md", `${old.trimEnd()}\n\n## ${a.name}\n\n$$\n${a.latex}\n$$\n`);
      return "Formules.md";
    }
    case "latex_labels":
      return [...(files.get(String(a.path)) ?? "").matchAll(/\\label\{([^}]+)\}/g)].map((m) => m[1]).concat(["fig:architecture", "tab:resultats", "eq:f1"]);
    case "latex_sync_forward":
      return [1, 300];
    case "latex_sync_inverse":
      return [a.path, 3];
    case "latex_preview":
      return { ok: true, pdf: "/demo/exemple.pdf", engine: "tectonic", issues: [], log: "", millis: 900 };
    case "attach_figure": {
      const out: string[] = [];
      for (const p of files.keys()) {
        const fm = meta(p).frontmatter;
        if (fm.type === "source" && fm.pdf === a.pdf) {
          const label = a.caption ? `${a.caption} (p. ${a.page})` : `Figure, p. ${a.page}`;
          files.set(p, `${files.get(p)!.trimEnd()}\n\n## Figures\n\n![[${a.image}]]\n*${label}*\n`);
          out.push(p);
        }
      }
      return out;
    }
    case "ai_science": {
      const out: Record<string, string> = {
        latex_table: "\\begin{table}[ht]\n  \\centering\n  \\begin{tabular}{lrr}\n    \\toprule\n    Modèle & AUC & F1 \\\\\n    \\midrule\n    GNN & 0,94 & 0,81 \\\\\n    XGBoost & 0,91 & 0,77 \\\\\n    \\bottomrule\n  \\end{tabular}\n  \\caption{Comparaison des modèles}\n  \\label{tab:modeles}\n\\end{table}",
        latex_equation: "\\begin{equation}\n  F_1 = \\frac{2PR}{P + R}\n  \\label{eq:f1}\n\\end{equation}\noù $P$ est la précision et $R$ le rappel.",
        tikz: "\\begin{figure}[ht]\n  \\centering\n  \\begin{tikzpicture}\n    \\node (a) {Transaction};\n    \\node[right=of a] (b) {Score};\n    \\draw[->] (a) -- (b);\n  \\end{tikzpicture}\n  \\caption{Chaîne de détection}\n  \\label{fig:chaine}\n\\end{figure}",
        explain_formula: "La formule calcule la **moyenne harmonique** de la précision $P$ et du rappel $R$.",
        fix_formula: "```latex\n\\frac{2PR}{P+R}\n```\n\n- Accolade fermante manquante.",
        improve: "- « Il est clair que » : préciser ce qui est clair.",
      };
      return { text: out[String(a.task)] ?? "", model: "démo", input_tokens: 0, output_tokens: 0 };
    }
    case "ai_check_support":
      return [
        { line: 12, sentence: "Les GNN dépassent XGBoost de 25 % [@smith2024graph].", citekey: "smith2024graph", verdict: "partielle", reason: "Le document compare GNN et gradient boosting, sans chiffrer l'écart à 25 %." },
        { line: 15, sentence: "SHAP est la seule méthode cohérente [@lundberg2017unified].", citekey: "lundberg2017unified", verdict: "soutenue", reason: "Unicité des attributions additives prouvée par les auteurs." },
      ];
    case "argument_new": {
      const title = String(a.title).trim();
      const p = `Thèse/Arguments/${title.replace(/[\\/:*?"<>|]/g, "-")}.md`;
      const arg = { title, claim: "", arguments: [{ title: "", text: "", evidence: [], rebuttal: "" }], objections: [{ title: "", text: "", evidence: [], rebuttal: "" }], conclusion: "" };
      args.set(p, arg);
      files.set(p, `---\ntype: argument\nstatut: brouillon\n---\n# ${title}\n\n## Affirmation\n\n`);
      return p;
    }
    case "argument_load": {
      const arg = args.get(String(a.path)) ?? { title: meta(a.path).title, claim: "", arguments: [], objections: [], conclusion: "" };
      return [arg, unknownKeys(arg)];
    }
    case "argument_save": {
      const arg = a.argument as { title: string; claim: string; arguments: { title: string; text: string; evidence: string[] }[]; objections: { title: string; text: string; rebuttal: string }[]; conclusion: string };
      args.set(String(a.path), arg);
      const md = [`# ${arg.title}`, "## Affirmation", arg.claim, ...arg.arguments.flatMap((x) => [`## Argument : ${x.title}`, x.text, ...x.evidence.map((e) => `- Preuve : ${e}`)]), ...arg.objections.flatMap((x) => [`## Objection : ${x.title}`, x.text, x.rebuttal ? `> Réponse : ${x.rebuttal}` : ""]), "## Conclusion", arg.conclusion].join("\n\n");
      files.set(String(a.path), `---\ntype: argument\nstatut: brouillon\n---\n${md}\n`);
      return unknownKeys(arg);
    }
    case "argument_export": {
      const arg = args.get(String(a.path))!;
      const base = String(a.path).replace(/\.md$/, "");
      const text = [arg.claim, ...arg.arguments.map((x, i) => `${["Premièrement", "Deuxièmement", "Enfin"][Math.min(i, 2)]}, ${x.title}. ${x.text} ${x.evidence.join(". ")}`), arg.conclusion ? `En conclusion, ${arg.conclusion}` : ""].filter(Boolean).join("\n\n");
      const p = `${base} (texte).md`;
      files.set(p, `---\ntype: brouillon\n---\n# ${arg.title}\n\n${text}\n`);
      if (a.format === "tex") {
        const tp = `${base.replace(/[^/]+$/, "")}LaTeX/${base.split("/").pop()} (texte).tex`;
        files.set(tp, `\\documentclass{article}\n\\begin{document}\n${text}\n\\end{document}\n`);
        return tp;
      }
      return p;
    }
    case "similarity_check": {
      const c = files.get(String(a.path)) ?? "";
      const needle = "fraud rings tend to form dense subgraphs with high transaction velocity";
      const i = c.toLowerCase().indexOf(needle);
      if (i < 0) return [];
      const line = c.slice(0, i).split("\n").length;
      const cited = /@smith2024graph/.test(c.split("\n")[line - 1] ?? "");
      return [{
        line, excerpt: c.split("\n")[line - 1], matched: c.slice(i, i + needle.length), matched_words: 11, coverage: 0.62,
        citekey: "smith2024graph", title: "Graph Neural Networks for Mobile Money Fraud Detection", path: "Sources/Thèse/Ch2 État de l'art/@smith2024graph.md",
        source_excerpt: "…Fraud rings tend to form dense subgraphs with high transaction velocity. Our dataset covers 2.3M transactions…", cited,
      }];
    }
    case "generate_outline": {
      const want = String(a.value).replace(/^#/, "").toLowerCase();
      const list = (v: unknown) => (Array.isArray(v) ? v.map((x) => String(x).toLowerCase()) : []);
      const inScope = (p: string) => {
        const m = meta(p);
        if (m.frontmatter.type === "chapitre" || p.startsWith("Modèles/") || p.startsWith("Thèse/Plans/")) return false;
        if (a.scope === "chapter") return list(m.frontmatter.chapitres).includes(want);
        if (a.scope === "tag") return (m.tags ?? []).some((t: string) => t.toLowerCase() === want || t.toLowerCase().startsWith(`${want}/`));
        return p.toLowerCase().startsWith(`${want}/`);
      };
      const items = [...files.keys()].filter((p) => p.endsWith(".md") && inScope(p));
      const chap = [...files.keys()].find((p) => meta(p).frontmatter.type === "chapitre" && String(meta(p).frontmatter.chapitre).toLowerCase() === want);
      const label = a.scope === "chapter" && chap ? meta(chap).title : a.scope === "tag" ? `#${want}` : String(a.value);
      const body = items.map((p) => `- [[${p.split("/").pop()!.replace(/\.md$/, "")}]]${meta(p).frontmatter.citekey ? ` [@${meta(p).frontmatter.citekey}]` : ""}`).join("\n");
      const p = `Thèse/Plans/Plan - ${label.replace(/[\\/:*?"<>|]/g, "-")}.md`;
      files.set(p, `---\ntype: brouillon-plan\ntags: [thèse/plan]\n---\n# Plan : ${label}\n\n> [!note] Brouillon de plan généré à partir de tes notes : réordonne, fusionne, renomme. Chaque point renvoie à sa note.\n\n## 1. Autres\n\n${body || "Aucune note dans ce périmètre."}\n`);
      return p;
    }
    case "methods_overview": {
      const docs = (cks: string[]) =>
        cks.map((ck) => {
          const f = [...files.keys()].find((p) => meta(p).frontmatter.type === "fiche" && String(meta(p).frontmatter.citekey ?? "").toLowerCase() === ck);
          const s = [...files.keys()].find((p) => meta(p).frontmatter.type === "source" && String(meta(p).frontmatter.citekey ?? "").toLowerCase() === ck);
          const p = f ?? s ?? "";
          const tp = s ?? p;
          return { citekey: ck, title: tp ? String(meta(tp).frontmatter.titre ?? meta(tp).title) : ck, path: p, year: Number(meta(s ?? p).frontmatter.annee) || null };
        });
      const noteOf = (name: string) => [...files.keys()].find((p) => meta(p).frontmatter.type === "methode" && meta(p).title === name) ?? "";
      const rows = [
        { name: "Réseaux de neurones sur graphes (GNN)", aliases: ["gnn", "graph neural network"], path: noteOf("Réseaux de neurones sur graphes (GNN)"), documents: docs(["smith2024graph"]) },
        { name: "SHAP", aliases: ["shap"], path: noteOf("SHAP"), documents: docs(["lundberg2017unified", "smith2024graph"]) },
        { name: "LIME", aliases: ["lime"], path: noteOf("LIME"), documents: docs(["ribeiro2016why"]) },
      ];
      for (const p of files.keys()) {
        const m = meta(p);
        if (m.frontmatter.type === "methode" && !rows.some((r) => r.name === m.title)) rows.push({ name: m.title, aliases: [], path: p, documents: [] });
      }
      return rows;
    }
    case "create_method_note": {
      const p = `Méthodes/${String(a.name).replace(/[\\/:*?"<>|]/g, "-")}.md`;
      files.set(p, `---\ntype: methode\naliases: [${(a.aliases as string[]).map((x) => `"${x}"`).join(", ")}]\ntags: [méthode]\n---\n# ${a.name}\n\n## Principe\n\n\n## Forces\n\n\n## Limites\n\n\n## Usage dans ma thèse\n\n`);
      return p;
    }
    case "thesis_overview": {
      const list = (v: unknown) => (Array.isArray(v) ? v.map((x) => String(x).toLowerCase()) : typeof v === "string" && v ? v.split(",").map((x) => x.trim().toLowerCase()) : []);
      const notes = [...files.keys()].filter((p) => p.endsWith(".md")).map((p) => ({ p, m: meta(p), c: files.get(p)! }));
      const questions = notes
        .filter(({ m }) => m.frontmatter.type === "question")
        .map(({ p, m, c }) => ({
          path: p,
          title: m.title,
          status: String(m.frontmatter.statut ?? "ouverte"),
          chapters: list(m.frontmatter.chapitres),
          sources: [...new Set([...c.matchAll(/@([\w:-]+)/g)].map((x) => x[1]))],
          notes: notes.filter(({ c: o, p: q }) => q !== p && o.includes(`[[${p.split("/").pop()!.replace(/\.md$/, "")}`)).map(({ p: q }) => q),
        }));
      const chapters = notes
        .filter(({ m }) => m.frontmatter.type === "chapitre")
        .map(({ p, m, c }) => {
          const id = String(m.frontmatter.chapitre ?? "").toLowerCase();
          const sec = c.split(/\n## Objectifs\n/)[1]?.split(/\n#/)[0] ?? "";
          const objectives = [...sec.matchAll(/^- (\[( |x)\] )?(.+)$/gm)].map((x) => ({ text: x[3], done: x[1] ? x[2] === "x" : null }));
          const attached = notes.filter(({ m: o }) => list(o.frontmatter.chapitres).includes(id));
          return {
            id, path: p, title: m.title, status: String(m.frontmatter.statut ?? ""), words: m.words, target: Number(m.frontmatter.objectif) || 0,
            objectives, depends_on: list(m.frontmatter.depend_de), questions: questions.filter((q) => q.chapters.includes(id)).map((q) => q.path),
            fiches: attached.filter(({ m: o }) => o.frontmatter.type === "fiche").length, linked: attached.length,
          };
        })
        .sort((a, b) => a.id.localeCompare(b.id, "fr", { numeric: true }));
      const level = new Map<string, number>();
      for (let pass = 0; pass < chapters.length + 1; pass++)
        for (const c of chapters) {
          const ds = c.depends_on.filter((d) => chapters.some((x) => x.id === d));
          if (ds.every((d) => level.has(d))) level.set(c.id, Math.max(0, ...ds.map((d) => level.get(d)! + 1)));
        }
      const layers: string[][] = [];
      for (const c of chapters) (layers[level.get(c.id) ?? 0] ??= []).push(c.id);
      const missing = chapters.flatMap((c) => c.depends_on.filter((d) => !chapters.some((x) => x.id === d)).map((d) => [c.id, d] as [string, string]));
      return { chapters, layers, missing, cycle: [], questions };
    }
    case "questions_for": {
      const ck = String(a.citekey).toLowerCase();
      return [...files.keys()]
        .filter((p) => meta(p).frontmatter.type === "question" && files.get(p)!.toLowerCase().includes(`@${ck}`))
        .map((p) => [p, meta(p).title, String(meta(p).frontmatter.statut ?? "ouverte")]);
    }
    case "new_zotero_documents":
      return zitems.filter((i) => !sourceOf(i)).map(summary);
    case "propose_tags": {
      const have = meta(a.path).tags ?? [];
      return ["fraude", "mobile-money"].filter((x) => !have.includes(x));
    }
    case "related_notes":
      return [...files.keys()]
        .filter((p) => p !== a.path && p.endsWith(".md") && !["source", "fiche"].includes(String(meta(p).frontmatter.type ?? "")) && !p.startsWith("Modèles/"))
        .slice(0, 3)
        .map((p) => [p, meta(p).title]);
    case "add_tags": {
      const c = files.get(String(a.path)) ?? "";
      const extra = (a.tags as string[]).map((x) => x.toLowerCase());
      files.set(String(a.path), c.replace(/^tags: \[(.*)\]$/m, (_m, cur: string) => `tags: [${[...new Set([...cur.split(",").map((x) => x.trim()).filter(Boolean), ...extra])].join(", ")}]`));
      return extra.length > 0;
    }
    case "create_fiche_with_links": {
      const fm = meta(a.path).frontmatter;
      const z = zitems.find((i) => i.key === fm.zotero);
      const it = z ?? ({ key: "", citekey: String(fm.citekey), title: String(fm.titre ?? "") } as ZItem);
      const p = makeFiche(it, a.path);
      const links = (a.links as string[]).map((l) => `[[${l.split("/").pop()!.replace(/\.md$/, "")}]]`).join(" · ");
      if (links) files.set(p, files.get(p)!.replace(/\n## /, `\n**Voir aussi :** ${links}\n\n## `));
      return { path: p, created: true };
    }
    case "create_fiche_from_source": {
      const fm = meta(a.path).frontmatter;
      const z = zitems.find((i) => i.key === fm.zotero);
      const it = z ?? ({ key: "", citekey: String(fm.citekey), title: String(fm.titre ?? "") } as ZItem);
      return { path: makeFiche(it, a.path), created: true };
    }
    case "zotero_item": {
      const it = zitems.find((i) => i.key === a.key);
      if (!it) throw "Introuvable";
      return { item: it, annotations: zann.filter((x) => x.item_key === it.key) } satisfies ItemDetail;
    }
    case "zotero_item_by_citekey": {
      const it = zitems.find((i) => i.citekey.toLowerCase() === String(a.citekey).toLowerCase());
      return it ? { item: it, annotations: zann.filter((x) => x.item_key === it.key) } : null;
    }
    case "zotero_search_annotations": {
      const q = String(a.query).toLowerCase();
      return zann.filter((x) => (x.text + x.comment).toLowerCase().includes(q)).map((x) => [x, zitems.find((i) => i.key === x.item_key)!.citekey]);
    }
    case "zotero_fiches_map":
      return fichesMap();
    case "import_source": {
      const it = zitems.find((i) => i.key === a.key)!;
      const existing = sourceOf(it);
      return existing ? { path: existing, created: false } : { path: makeSource(it), created: true };
    }
    case "create_fiche": {
      const it = zitems.find((i) => i.key === a.key)!;
      const source = sourceOf(it) ?? makeSource(it);
      return { path: makeFiche(it, source), created: true };
    }
    case "legacy_imports":
      return [];
    case "clean_legacy_imports":
      return 0;
    case "fiches_of":
      return [...files.keys()].filter((p) => meta(p).frontmatter.type === "fiche" && meta(p).frontmatter.zotero === a.key).sort();
    case "sync_fiche": {
      const c = files.get(a.path)!;
      const ck = splitFm(c)[0].citekey as string;
      const it = zitems.find((i) => i.citekey === ck);
      if (!it) throw "Document absent du miroir";
      const nc = c.replace(/<!-- zotero:debut -->[\s\S]*<!-- zotero:fin -->/, annBlock(it));
      files.set(a.path, nc);
      return nc;
    }
    case "export_bibtex":
      return zitems.length;
    case "export_note":
      return a.options.output;
    case "pandoc_version":
      return null;
    case "themes_custom":
      return customThemes;
    case "theme_create_example": {
      const id = customThemes.length ? `Mon thème ${customThemes.length + 1}` : "Mon thème";
      customThemes.push({ id, name: id, scheme: "dark", css: ":root{--accent:#e0a542;--bg:#1b1a22;--bg-soft:#16151c;--text:#ece8f4}" });
      return `.eloria/themes/${id}.css`;
    }
    case "open_url":
      window.open(a.url, "_blank");
      return null;
    case "reveal_path":
      return null;
  }
  throw new Error(`Commande simulée inconnue : ${cmd}`);
}
