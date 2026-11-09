// Types shared with the Rust core (eloria-core). Keep in sync with the serde structures.

export interface Settings {
  language: "fr" | "en";
  /** "system", a gallery theme (src/lib/themes.ts) or "custom:<file>" */
  theme: string;
  accent: string;
  font_ui: string;
  font_editor: string;
  font_mono: string;
  font_size: number;
  line_height: number;
  editor_width: number;
  heading_scale: number;
  live_preview: boolean;
  spellcheck: boolean;
  spellcheck_lang: string;
  show_line_numbers: boolean;
  readable_line_length: boolean;
  autosave_ms: number;
  history_interval: number;
  zotero_data_dir: string;
  zotero_port: number;
  fiches_folder: string;
  /** Copy each item's PDF into the vault */
  copy_pdfs: boolean;
  zotero_auto_sync: boolean;
  backup_dir: string;
  latex_path: string;
  embed_provider: string;
  ai_agents: { name: string; instructions: string }[];
  auto_reading_status: boolean;
  inbox_folder: string;
  local_api: boolean;
  goal_fiches_week: number;
  goal_words_day: number;
  focus_minutes: number;
  local_api_port: number;
  local_api_token: string;
  embed_model: string;
  embed_url: string;
  backup_interval_hours: number;
  backup_keep: number;
  pdf_folder: string;
  sources_folder: string;
  attachments_folder: string;
  templates_folder: string;
  daily_folder: string;
  pandoc_path: string;
  csl_path: string;
  /** AI enabled */
  ai_enabled: boolean;
  /** "anthropic", "openai" or "compatible" */
  ai_provider: "anthropic" | "openai" | "compatible";
  ai_model: string;
  ai_base_url: string;
  ai_effort: "low" | "medium" | "high" | "xhigh" | "max";
  /** Research topic, given to the AI */
  research_topic: string;
  /** Custom shortcuts: command identifier -> keys, "" = disabled */
  keys: Record<string, string>;
  recent_vaults: string[];
  last_vault: string | null;
  /** Typed properties */
  property_defs: PropDef[];
  /** Saved views of the notes database */
  saved_views: SavedView[];
  /** Custom palette commands */
  custom_commands: CustomCommand[];
  /** Approved extensions: managed by the core (extension_enable), never by the interface */
  extension_grants: { vault: string; id: string; hash: string }[];
}

export interface ExtensionInfo {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string;
  /** Folder relative to the vault */
  dir: string;
  hash: string;
  has_styles: boolean;
  /** Approved for this vault, code unchanged */
  enabled: boolean;
  /** Code changed since approval */
  changed: boolean;
  error: string | null;
}

export interface ExtensionCode {
  id: string;
  name: string;
  version: string;
  code: string;
  css: string | null;
}

export type PropKind = "texte" | "nombre" | "date" | "case" | "choix" | "liste" | "lien";

export interface PropDef {
  name: string;
  kind: PropKind;
  options: string[];
}

export interface SavedView {
  name: string;
  folder: string;
  type_filter: string;
  tag: string;
  text: string;
  sort_key: string;
  sort_dir: number;
  hidden: string[];
}

export type CustomKind = "modele" | "texte" | "vue" | "note" | "recherche";

export interface CustomCommand {
  name: string;
  kind: CustomKind;
  arg: string;
  folder: string;
}


export interface CustomTheme {
  id: string;
  name: string;
  scheme: "light" | "dark";
  css: string;
}

export interface VaultInfo {
  root: string;
  name: string;
  /** Items created on opening (layout of a new vault) */
  created?: number;
}

export interface ImportPreview {
  total: number;
  existing: number;
  new: number;
}

export interface ImportReport {
  created: number;
  skipped: number;
  pdfs: number;
  updated: number;
  failed: string[];
}

export interface TreeNode {
  name: string;
  path: string;
  is_dir: boolean;
  kind: "note" | "image" | "pdf" | "dir";
  children: TreeNode[];
}

export interface NoteMeta {
  path: string;
  title: string;
  stem: string;
  aliases: string[];
  tags: string[];
  mtime: number;
  words: number;
  frontmatter: Record<string, unknown>;
}

export interface NoteFile {
  path: string;
  content: string;
  mtime: number;
}

export interface SearchHit {
  path: string;
  title: string;
  snippet: string;
}

export interface LinkRef {
  path: string;
  title: string;
  line: number;
  context: string;
}

export interface OutLink {
  target: string;
  resolved: string | null;
  line: number;
}

export interface GraphNode {
  id: string;
  label: string;
  kind: "note" | "tag" | "missing";
  degree: number;
  group: string;
}

export interface GraphEdge {
  source: string;
  target: string;
  kind: "link" | "tag" | "citation";
}

export interface Graph {
  nodes: GraphNode[];
  edges: GraphEdge[];
}

export interface GraphOptions {
  include_tags: boolean;
  include_orphans: boolean;
  include_missing: boolean;
  folder: string | null;
  exclude?: string[];
}

export interface Version {
  id: string;
  timestamp: number;
  size: number;
}

export interface RenameResult {
  path: string;
  updated: string[];
}

export interface SyncStats {
  indexed: number;
  removed: number;
  total: number;
}

export interface VaultStats {
  notes: number;
  words: number;
  fiches: number;
  tags: number;
}

// ------------------------------------------------------------------- Zotero

export interface Creator {
  creator_type: string;
  first_name: string;
  last_name: string;
}

export interface Attachment {
  key: string;
  title: string;
  content_type: string;
  path: string;
}

export interface ZItem {
  key: string;
  item_type: string;
  title: string;
  creators: Creator[];
  date: string;
  year: number | null;
  publication: string;
  doi: string;
  url: string;
  abstract_note: string;
  citekey: string;
  tags: string[];
  collections: string[];
  attachments: Attachment[];
  fields: Record<string, string>;
  date_added: string;
  date_modified: string;
}

export interface ZCollection {
  key: string;
  name: string;
  parent: string | null;
  item_count: number;
}

export interface ZAnnotation {
  key: string;
  item_key: string;
  attachment_key: string;
  kind: string;
  text: string;
  comment: string;
  color: string;
  page_label: string;
  sort_index: string;
  /** Physical page in the file (0 = first); may differ from page_label */
  page_index: number | null;
  tags: string[];
  date_modified: string;
}

export interface ItemSummary {
  key: string;
  item_type: string;
  title: string;
  authors: string;
  year: number | null;
  citekey: string;
  publication: string;
  annotation_count: number;
}

export interface ItemDetail {
  item: ZItem;
  annotations: ZAnnotation[];
}

export interface MirrorStatus {
  items: number;
  annotations: number;
  collections: number;
  last_sync: string | null;
  source: string | null;
}

export interface ZoteroDetect {
  data_dir: string | null;
  api: boolean;
}

export type RewriteTask = "rephrase" | "fix" | "expand" | "shorten" | "academic" | "translate_en" | "translate_fr" | "summarize";

export interface Rewrite {
  text: string;
  model: string;
  input_tokens: number;
  output_tokens: number;
}

/** Excerpt given to Ask Eloria, numbered [S1]… */
export interface Passage {
  id: number;
  path: string;
  title: string;
  citekey: string;
  zotero: string;
  text: string;
}

export interface AskAnswer {
  text: string;
  passages: Passage[];
  unverified: string[];
  model: string;
  input_tokens: number;
  output_tokens: number;
}

export interface ReviewArticle {
  citekey: string;
  statut: string;
  title: string;
  author_year: string;
  source: string;
  fiches: string[];
}

export interface Review {
  path: string;
  title: string;
  question: string;
  inclusion: string[];
  exclusion: string[];
  articles: ReviewArticle[];
}

export interface CitationInfo {
  openalex: string;
  cited_by_count: number;
  references: Work[];
  cited_by: Work[];
  reference_dois: string[];
}

export interface SourceCitations {
  info: CitationInfo | null;
  library: Record<string, string>;
}

export interface CitationNetwork {
  nodes: { citekey: string; title: string; year: number | null; label: string; fetched: boolean }[];
  edges: [string, string][];
  missing: string[];
}

export interface MdImportReport {
  notes: number;
  files: number;
  links_rewritten: number;
  skipped: number;
  folder: string;
}

export interface GitStatus {
  available: boolean;
  repo: boolean;
  branch: string;
  changes: number;
  remote: string;
  log: [string, string, string][];
}

export interface CheckItem {
  label: string;
  ok: boolean;
  detail: string;
}

export interface BibIssue {
  citekey: string;
  title: string;
  kind: string;
  detail: string;
}

export interface CitationCheck {
  key: string;
  lines: number[];
  known: boolean;
}

export interface StyleIssue {
  line: number;
  kind: string;
  excerpt: string;
  message: string;
}

export interface Dashboard {
  notes: number;
  words: number;
  sources: number;
  fiches: Record<string, number>;
  written: [string, number][];
  written_today: number;
  fiches_read_week: number;
  focus_today: number;
  focus_week: number;
  reviews_due: [string, string, string][];
  chapters: [string, string, number, number][];
  to_read: [string, string][];
  recent: [string, string, number][];
  ai_tokens: [number, number];
}

export interface ConceptSuggestion {
  name: string;
  definition: string;
}

export interface PdfAnnotation {
  id: string;
  page: number;
  kind: "highlight" | "note";
  color: string;
  text: string;
  comment: string;
  /** [x, y, width, height] as fractions of the page */
  rects: [number, number, number, number][];
  created: string;
}

export interface SemanticStatus {
  configured: boolean;
  provider: string;
  model: string;
  key_present: boolean;
  stale: number;
}

export interface SemanticHit {
  path: string;
  title: string;
  score: number;
}

export interface TexEngine {
  kind: "tectonic" | "latexmk" | "pdflatex";
  path: string;
  version: string;
}

export interface TexIssue {
  file: string;
  line: number;
  message: string;
  level: "error" | "warning";
}

export interface CompileResult {
  ok: boolean;
  pdf: string | null;
  engine: string;
  issues: TexIssue[];
  log: string;
  millis: number;
}

export interface MatrixRow {
  path: string;
  citekey: string;
  title: string;
  author_year: string;
  year: number | null;
  statut: string;
  cells: string[];
}

export interface Matrix {
  columns: [string, string][];
  rows: MatrixRow[];
}

export interface BackupInfo {
  path: string;
  name: string;
  bytes: number;
  created: string;
  files: number;
}

export interface Issue {
  path: string;
  line: number;
  detail: string;
}

export interface IntegrityReport {
  notes: number;
  broken_links: Issue[];
  missing_files: Issue[];
  bad_frontmatter: Issue[];
  orphan_sources: Issue[];
  fiches_without_source: Issue[];
}

/** Article described by an online database (Crossref, Semantic Scholar, OpenAlex). */
export interface Work {
  title: string;
  /** [first name, last name] */
  authors: [string, string][];
  year: number | null;
  doi: string;
  url: string;
  venue: string;
  abstract_note: string;
  item_type: string;
  citations: number | null;
  origin: string;
  /** Keywords and subjects (become the source's tags). */
  keywords?: string[];
}

export type ReliabilityLevel = "ok" | "info" | "attention" | "alerte";

export interface ReliabilitySignal {
  level: ReliabilityLevel;
  text: string;
}

/** Reliability of a document: verifiable signals (OpenAlex, Retraction Watch). */
export interface ReliabilityView {
  level: ReliabilityLevel;
  signals: ReliabilitySignal[];
  checked: string;
}

export interface ReliabilityRow {
  citekey: string;
  /** Zotero key (empty: Eloria's own source) */
  key: string;
  title: string;
  /** Source note (empty: Zotero item not imported yet) */
  path: string;
  doi: string;
  level: ReliabilityLevel;
  signals: ReliabilitySignal[];
  /** Cited in a working note */
  cited: boolean;
}

export interface KnowledgeMap {
  nodes: { id: string; title: string; kind: string; degree: number }[];
  /** [a, b, "lien" | "mention"] */
  edges: [string, string, string][];
}

export interface TaxNodeInfo {
  label: string;
  tag: string;
  depth: number;
  terms: string[];
  /** [path, title] of the notes filed under this term or its sub-terms */
  notes: [string, string][];
}

export interface TaxonomyOverview {
  path: string;
  exists: boolean;
  nodes: TaxNodeInfo[];
}

export type EntityKind = "personne" | "organisation" | "methode" | "technologie" | "lieu";

export interface EntityRow {
  kind: EntityKind;
  name: string;
  /** [chemin, titre] */
  notes: [string, string][];
}

/** Topic group of notes. */
export interface Topic {
  terms: string[];
  /** [path, title, closeness to the centre] */
  notes: [string, string, number][];
  tag: string;
}

export interface ConceptRow {
  path: string;
  title: string;
  aliases: string[];
  definition: string;
  sources: number;
  notes: number;
  updated: string;
}

export interface ConceptMention {
  date: string;
  path: string;
  title: string;
  kind: string;
  citekey: string;
  excerpt: string;
  linked: boolean;
}

export interface ConceptDetail {
  path: string;
  title: string;
  aliases: string[];
  definition: string;
  timeline: ConceptMention[];
  related: [string, string][];
  questions: [string, string][];
  definitions: { date: string; definition: string }[];
  card: string;
}

export interface ResearchStats {
  citations_total: number;
  documents_cited: number;
  documents_total: number;
  /** [key, title, notes that cite it] */
  top_documents: [string, string, number][];
  top_authors: [string, number][];
  top_venues: [string, number][];
  years: [number, number][];
  heat_tags: string[];
  heat_months: string[];
  heat: number[][];
  cover_themes: string[];
  cover_chapters: string[];
  cover: number[][];
  gaps: [string, number][];
}

export interface DiskReport {
  total: number;
  files: number;
  /** [category, bytes, files] */
  by_type: [string, number, number][];
  by_folder: [string, number, number][];
  largest: [string, number][];
  internal: [string, number][];
  aux: string[];
  aux_bytes: number;
  trash_bytes: number;
}

/** An argument (with its evidence) or an objection (with its answer). */
export interface ArgPoint {
  title: string;
  text: string;
  evidence: string[];
  rebuttal: string;
}

export interface ArgumentMap {
  title: string;
  claim: string;
  arguments: ArgPoint[];
  objections: ArgPoint[];
  conclusion: string;
}

export type SciTask = "latex_table" | "latex_equation" | "tikz" | "explain_formula" | "fix_formula" | "improve";

export interface SupportVerdict {
  line: number;
  sentence: string;
  citekey: string;
  /** soutenue, partielle, non_soutenue, inconnue */
  verdict: string;
  reason: string;
}

/** Passage repeated word for word from a library text. */
export interface SimilarityHit {
  line: number;
  excerpt: string;
  matched: string;
  matched_words: number;
  coverage: number;
  citekey: string;
  title: string;
  path: string;
  source_excerpt: string;
  cited: boolean;
}

export interface MethodUse {
  citekey: string;
  title: string;
  /** Reading note if any, otherwise the source */
  path: string;
  year: number | null;
}

export interface MethodRow {
  name: string;
  aliases: string[];
  /** Method note (empty: catalogue only) */
  path: string;
  documents: MethodUse[];
}

export interface Objective {
  text: string;
  /** true: checked, false: to do, null: plain bullet */
  done: boolean | null;
}

export interface ChapterInfo {
  id: string;
  path: string;
  title: string;
  status: string;
  words: number;
  target: number;
  objectives: Objective[];
  depends_on: string[];
  questions: string[];
  fiches: number;
  linked: number;
}

export interface QuestionInfo {
  path: string;
  title: string;
  status: string;
  chapters: string[];
  sources: string[];
  notes: string[];
}

export interface ThesisOverview {
  chapters: ChapterInfo[];
  layers: string[][];
  missing: [string, string][];
  cycle: string[];
  questions: QuestionInfo[];
}

/** PDF added without Zotero. */
export interface PdfAdded {
  path: string;
  created: boolean;
  /** PDF in the vault */
  pdf: string;
  /** Recognised DOI (empty: title taken from the file name) */
  doi: string;
}

export interface FicheResult {
  path: string;
  created: boolean;
}

export interface ExportOptions {
  format: "docx" | "pdf" | "odt" | "tex" | "html";
  output: string;
  citations: boolean;
  strip_annotations: boolean;
  toc: boolean;
}

export interface AiResponse {
  text: string;
  model: string;
  input_tokens: number;
  output_tokens: number;
  stop_reason: string;
  fell_back: boolean;
  truncated: boolean;
}

export interface AiFicheDraft {
  item_key: string;
  citekey: string;
  block: string;
  tags: string[];
  model: string;
  input_tokens: number;
  output_tokens: number;
  source: "pdf" | "texte" | "résumé";
  refs_in_library: number;
  refs_in_article: number;
  refs_unverified: number;
  fell_back: boolean;
  truncated: boolean;
}

export interface AiLogEntry {
  date: string;
  task: string;
  provider: string;
  model: string;
  input_tokens: number;
  output_tokens: number;
  ok: boolean;
}

export interface ChangeEvent {
  paths: string[];
  structural: boolean;
}

// ------------------------------------------------------------------- Interface

export type TabKind = "note" | "graph" | "kanban" | "table" | "timeline" | "zotero-item" | "image" | "pdf" | "matrix" | "review" | "latex" | "dashboard" | "citations" | "these" | "methodes" | "argument" | "statistiques" | "concepts" | "themes" | "entites" | "taxonomie" | "atlas" | "aide" | "echanges" | "console";

export interface Tab {
  id: string;
  kind: TabKind;
  title: string;
  path?: string;
  key?: string;
  /** PDF tab: displayed physical page (1 = first) */
  page?: number;
  dirty?: boolean;
}

export interface Pane {
  id: string;
  tabs: Tab[];
  active: string | null;
}
