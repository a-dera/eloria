// Access to the Rust core. In a browser (npm run dev without Tauri), an in-memory
// mock backend takes over: handy to work on the interface.

import type {
  ExtensionCode,
  ExtensionInfo,
  Work,
  DiskReport,
  SciTask,
  SupportVerdict,
  KnowledgeMap,
  TaxonomyOverview,
  EntityRow,
  Topic,
  ConceptRow,
  ConceptDetail,
  ResearchStats,
  ArgumentMap,
  SimilarityHit,
  MethodRow,
  ThesisOverview,
  ReliabilityView,
  ReliabilityRow,
  PdfAdded,
  CheckItem,
  SourceCitations,
  CitationNetwork,
  MdImportReport,
  GitStatus,
  BibIssue,
  CitationCheck,
  StyleIssue,
  Dashboard,
  ConceptSuggestion,
  PdfAnnotation,
  SemanticStatus,
  SemanticHit,
  TexEngine,
  CompileResult,
  Review,
  Matrix,
  RewriteTask,
  Rewrite,
  AskAnswer,
  Passage,
  BackupInfo,
  IntegrityReport,
  AiFicheDraft,
  AiLogEntry,
  AiResponse,
  ChangeEvent,
  CustomTheme,
  ExportOptions,
  FicheResult,
  ImportPreview,
  ImportReport,
  Graph,
  GraphOptions,
  ItemDetail,
  ItemSummary,
  LinkRef,
  MirrorStatus,
  NoteFile,
  NoteMeta,
  OutLink,
  RenameResult,
  SearchHit,
  Settings,
  SyncStats,
  TreeNode,
  VaultInfo,
  VaultStats,
  Version,
  ZAnnotation,
  ZCollection,
  ZoteroDetect,
} from "./types";

import { t } from "./i18n.svelte";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function call<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    try {
      return await invoke<T>(cmd, args);
    } catch (e) {
      // Command unknown to the binary: the Rust core is older than the interface.
      if (/command .* not found/i.test(String(e))) window.dispatchEvent(new CustomEvent("eloria:stale-core", { detail: cmd }));
      throw e;
    }
  }
  const { mockCall } = await import("./mock");
  return mockCall(cmd, args) as Promise<T>;
}

export const api = {
  getSettings: () => call<Settings>("get_settings"),
  defaultSettings: () => call<Settings>("default_settings"),
  saveSettings: (settings: Settings) => call<void>("save_settings", { settings }),

  openVault: (path: string) => call<VaultInfo>("open_vault", { path }),
  closeVault: () => call<void>("close_vault"),
  vaultInfo: () => call<VaultInfo | null>("vault_info"),
  tree: () => call<TreeNode>("vault_tree"),
  folders: () => call<string[]>("list_folders"),
  allNotes: () => call<NoteMeta[]>("all_notes"),
  stats: () => call<VaultStats>("vault_stats"),
  rebuildIndex: () => call<SyncStats>("rebuild_index"),

  readNote: (path: string) => call<NoteFile>("read_note", { path }),
  saveNote: (path: string, content: string) => call<number>("save_note", { path, content }),
  createNote: (folder: string, name: string, content = "") => call<string>("create_note", { folder, name, content }),
  createFromLink: (target: string, from: string) => call<string>("create_from_link", { target, from }),
  createFolder: (path: string) => call<void>("create_folder", { path }),
  deletePath: (path: string) => call<void>("delete_path", { path }),
  rename: (from: string, to: string, updateLinks = true) =>
    call<RenameResult>("rename_path", { from, to, updateLinks }),
  setProperty: (path: string, key: string, value: unknown) => call<string>("set_property", { path, key, value }),
  saveAttachment: (folder: string, name: string, data: string) =>
    call<string>("save_attachment", { folder, name, data }),
  importBibtex: (path: string) => call<{ created: number; existing: number; failed: string[] }>("import_bibtex", { path }),
  diskReport: () => call<DiskReport>("disk_report"),
  cleanLatexAux: () => call<number>("clean_latex_aux"),
  emptyTrash: () => call<number>("empty_trash"),
  clearTemp: () => call<number>("clear_temp"),
  formulas: () => call<{ name: string; latex: string; group: string }[]>("formulas"),
  addFormula: (name: string, latex: string) => call<string>("add_formula", { name, latex }),
  latexLabels: (path: string) => call<string[]>("latex_labels", { path }),
  latexSyncForward: (path: string, line: number) => call<[number, number] | null>("latex_sync_forward", { path, line }),
  latexSyncInverse: (path: string, page: number, y: number) => call<[string, number] | null>("latex_sync_inverse", { path, page, y }),
  latexPreview: (snippet: string) => call<CompileResult>("latex_preview", { snippet }),
  attachFigure: (pdf: string, image: string, page: number, caption: string) => call<string[]>("attach_figure", { pdf, image, page, caption }),

  search: (query: string, limit = 50) => call<SearchHit[]>("search", { query, limit }),
  backlinks: (path: string) => call<LinkRef[]>("backlinks", { path }),
  unlinkedMentions: (path: string) => call<SearchHit[]>("unlinked_mentions", { path }),
  outgoing: (path: string) => call<OutLink[]>("outgoing", { path }),
  tags: () => call<[string, number][]>("tags"),
  resolveLink: (target: string, from?: string) => call<string | null>("resolve_link", { target, from }),
  graph: (options: GraphOptions) => call<Graph>("graph", { options }),
  localGraph: (path: string, depth: number, includeTags: boolean) =>
    call<Graph>("local_graph", { path, depth, includeTags }),

  historyList: (path: string) => call<Version[]>("history_list", { path }),
  historyRead: (path: string, id: string) => call<string>("history_read", { path, id }),
  historySnapshot: (path: string, content: string) => call<string | null>("history_snapshot", { path, content }),

  templates: () => call<string[]>("templates_list"),
  renderTemplate: (path: string, title: string) => call<string>("template_render", { path, title }),
  dailyNote: () => call<string>("daily_note"),

  zoteroDetect: () => call<ZoteroDetect>("zotero_detect"),
  zoteroPdfPath: (key: string) => call<string | null>("zotero_pdf_path", { key }),
  zoteroImportAll: (byCollection: boolean, collection?: string | null) => call<ImportReport>("zotero_import_all", { byCollection, collection: collection ?? null }),
  zoteroImportPreview: (collection?: string | null) => call<ImportPreview>("zotero_import_preview", { collection: collection ?? null }),
  zoteroSync: (source?: "auto" | "sqlite" | "api") => call<MirrorStatus>("zotero_sync", { source }),
  zoteroStatus: () => call<MirrorStatus>("zotero_status"),
  zoteroCollections: () => call<ZCollection[]>("zotero_collections"),
  zoteroItems: (collection?: string | null, query?: string | null, limit = 500) =>
    call<ItemSummary[]>("zotero_items", { collection, query, limit }),
  zoteroItem: (key: string) => call<ItemDetail>("zotero_item", { key }),
  zoteroItemByCitekey: (citekey: string) => call<ItemDetail | null>("zotero_item_by_citekey", { citekey }),
  zoteroSearchAnnotations: (query: string) => call<[ZAnnotation, string][]>("zotero_search_annotations", { query }),
  fichesMap: () => call<Record<string, string>>("zotero_fiches_map"),
  /** New reading note attached to the item's source (source created if needed). */
  createFiche: (key: string) => call<FicheResult>("create_fiche", { key }),
  /** Source note of the item (created if needed). */
  importSource: (key: string) => call<FicheResult>("import_source", { key }),
  fichesOf: (key: string) => call<string[]>("fiches_of", { key }),
  lookupDoi: (doi: string) => call<Work | null>("lookup_doi", { doi }),
  searchWorks: (query: string) => call<Work[]>("search_works", { query }),
  similarWorks: (doi: string | null, title: string) => call<Work[]>("similar_works", { doi, title }),
  addReference: (work: Work) => call<FicheResult>("add_reference", { work }),
  completeSource: (path: string) => call<boolean>("complete_source", { path }),
  completeSources: async (onProgress: (p: [number, number]) => void): Promise<number> => {
    if (isTauri) {
      const { Channel } = await import("@tauri-apps/api/core");
      const channel = new Channel<[number, number]>();
      channel.onmessage = onProgress;
      return call<number>("complete_sources", { onProgress: channel });
    }
    onProgress([1, 1]);
    return call<number>("complete_sources");
  },
  addPdf: (path: string) => call<PdfAdded>("add_pdf", { path }),
  thesisOverview: () => call<ThesisOverview>("thesis_overview"),
  methodsOverview: () => call<MethodRow[]>("methods_overview"),
  conceptsOverview: () => call<ConceptRow[]>("concepts_overview"),
  topics: (k = 0) => call<Topic[]>("topics", { k }),
  entities: () => call<EntityRow[]>("entities"),
  knowledgeMap: (withSources = false) => call<KnowledgeMap>("knowledge_map", { withSources }),
  taxonomy: () => call<TaxonomyOverview>("taxonomy"),
  taxonomyProposals: () => call<[string, string, string[]][]>("taxonomy_proposals"),
  taxonomyCreate: () => call<string>("taxonomy_create"),
  conceptDetail: (path: string) => call<ConceptDetail>("concept_detail", { path }),
  knowledgeCard: (path: string) => call<string>("knowledge_card", { path }),
  researchStats: (year: number, month: number) => call<ResearchStats>("research_stats", { year, month }),
  argumentNew: (title: string) => call<string>("argument_new", { title }),
  argumentLoad: (path: string) => call<[ArgumentMap, string[]]>("argument_load", { path }),
  argumentSave: (path: string, argument: ArgumentMap) => call<string[]>("argument_save", { path, argument }),
  argumentExport: (path: string, format: "md" | "tex") => call<string>("argument_export", { path, format }),
  similarityCheck: (path: string) => call<SimilarityHit[]>("similarity_check", { path }),
  generateOutline: (scope: "chapter" | "tag" | "folder", value: string) => call<string>("generate_outline", { scope, value }),
  createMethodNote: (name: string, aliases: string[]) => call<string>("create_method_note", { name, aliases }),
  questionsFor: (citekey: string) => call<[string, string, string][]>("questions_for", { citekey }),
  newZoteroDocuments: (limit = 20) => call<ItemSummary[]>("new_zotero_documents", { limit }),
  proposeTags: (path: string) => call<string[]>("propose_tags", { path }),
  relatedNotes: (path: string) => call<[string, string][]>("related_notes", { path }),
  addTags: (path: string, tags: string[]) => call<boolean>("add_tags", { path, tags }),
  createFicheWithLinks: (path: string, links: string[]) => call<FicheResult>("create_fiche_with_links", { path, links }),
  reliabilityOf: (doi: string) => call<ReliabilityView | null>("reliability_of", { doi }),
  checkReliability: (doi: string) => call<ReliabilityView>("check_reliability", { doi }),
  checkReliabilityAll: async (onProgress: (p: [number, number]) => void): Promise<number> => {
    if (isTauri) {
      const { Channel } = await import("@tauri-apps/api/core");
      const channel = new Channel<[number, number]>();
      channel.onmessage = onProgress;
      return call<number>("check_reliability_all", { onProgress: channel });
    }
    onProgress([1, 1]);
    return call<number>("check_reliability_all");
  },
  reliabilityReport: () => call<[ReliabilityRow[], number]>("reliability_report"),
  /** PDF dropped on the window: its bytes are put in a temporary folder; returns the path. */
  stagePdf: async (file: File): Promise<string> => {
    if (!isTauri) return call<string>("stage_pdf", { name: file.name });
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<string>("stage_pdf", new Uint8Array(await file.arrayBuffer()), { headers: { "x-name": encodeURIComponent(file.name) } });
  },
  storePdfText: (path: string, text: string) => call<string[]>("store_pdf_text", { path, text }),
  hasPdfText: (path: string) => call<boolean>("has_pdf_text", { path }),
  pdfsWithoutText: () => call<string[]>("pdfs_without_text"),
  searchPdfText: (query: string) => call<[string, string, string][]>("search_pdf_text", { query }),
  sourceDetail: (path: string) => call<ItemDetail>("source_detail", { path }),
  backupNow: (dir?: string) => call<BackupInfo>("backup_now", { dir: dir ?? null }),
  backupList: () => call<BackupInfo[]>("backup_list"),
  backupRestore: (archive: string, target: string) => call<number>("backup_restore", { archive, target }),
  createReview: (title: string, question: string) => call<string>("create_review", { title, question }),
  review: (path: string) => call<Review>("review", { path }),
  reviewSetArticle: (path: string, citekey: string, statut: string) => call<Review>("review_set_article", { path, citekey, statut }),
  aiReviewDraft: async (path: string, onDelta: (d: string) => void): Promise<AskAnswer> => {
    if (isTauri) {
      const { Channel } = await import("@tauri-apps/api/core");
      const channel = new Channel<string>();
      channel.onmessage = onDelta;
      return call<AskAnswer>("ai_review_draft", { path, onDelta: channel });
    }
    const { mockAiAsk } = await import("./mock");
    return mockAiAsk("état de l'art", onDelta);
  },
  pdfAnnotations: (path: string) => call<PdfAnnotation[]>("pdf_annotations", { path }),
  savePdfAnnotations: (path: string, list: PdfAnnotation[]) => call<string[]>("save_pdf_annotations", { path, list }),
  semanticStatus: () => call<SemanticStatus>("semantic_status"),
  semanticSetKey: (provider: string, key: string) => call<void>("semantic_set_key", { provider, key }),
  semanticIndex: async (onProgress: (n: number) => void): Promise<number> => {
    if (isTauri) {
      const { Channel } = await import("@tauri-apps/api/core");
      const channel = new Channel<number>();
      channel.onmessage = onProgress;
      return call<number>("semantic_index", { onProgress: channel });
    }
    const { mockSemanticIndex } = await import("./mock");
    return mockSemanticIndex(onProgress);
  },
  semanticSearch: (query: string, limit = 30) => call<SemanticHit[]>("semantic_search", { query, limit }),
  latexEngine: () => call<TexEngine | null>("latex_engine"),
  latexInstallTectonic: () => call<TexEngine>("latex_install_tectonic"),
  latexTemplates: () => call<[string, string][]>("latex_templates"),
  latexNew: (folder: string, name: string, template: string) => call<string>("latex_new", { folder, name, template }),
  latexCompile: (path: string) => call<CompileResult>("latex_compile", { path }),
  readText: (path: string) => call<string>("read_text", { path }),
  writeText: (path: string, content: string) => call<void>("write_text", { path, content }),
  researchMatrix: (citekeys: string[] | null = null) => call<Matrix>("research_matrix", { citekeys }),
  exportMatrix: (format: "csv" | "md" | "tex", citekeys: string[] | null = null) => call<string>("export_matrix", { format, citekeys }),
  checkIntegrity: () => call<IntegrityReport>("check_integrity"),
  embed: (target: string) => call<{ path: string; title: string; html: string } | null>("embed", { target }),
  fichesOfSource: (path: string) => call<string[]>("fiches_of_source", { path }),
  createFicheFromSource: (path: string) => call<FicheResult>("create_fiche_from_source", { path }),
  legacyImports: () => call<string[]>("legacy_imports"),
  cleanLegacyImports: () => call<number>("clean_legacy_imports"),
  syncFiche: (path: string) => call<string>("sync_fiche", { path }),
  exportBibtex: (output: string, onlyCited: boolean) => call<number>("export_bibtex", { output, onlyCited }),

  exportNote: (path: string, options: ExportOptions) => call<string>("export_note", { path, options }),
  pandocVersion: () => call<string | null>("pandoc_version"),

  customThemes: () => call<CustomTheme[]>("themes_custom"),
  createExampleTheme: () => call<string>("theme_create_example"),

  aiKeyPresent: (provider: string) => call<boolean>("ai_key_present", { provider }),
  aiSetKey: (provider: string, key: string) => call<void>("ai_set_key", { provider, key }),
  aiTest: () => call<AiResponse>("ai_test"),
  aiCancel: () => call<void>("ai_cancel"),
  /** "Eloria method" analysis; the text arrives piece by piece in `onDelta`. */
  aiFicheGenerate: async (key: string, onDelta: (d: string) => void): Promise<AiFicheDraft> => {
    if (isTauri) {
      const { Channel, invoke } = await import("@tauri-apps/api/core");
      const channel = new Channel<string>();
      channel.onmessage = onDelta;
      return invoke<AiFicheDraft>("ai_fiche_generate", { key, onDelta: channel });
    }
    const { mockAiFiche } = await import("./mock");
    return mockAiFiche(key, onDelta);
  },
  aiRewrite: async (task: RewriteTask, text: string, onDelta: (d: string) => void): Promise<Rewrite> => {
    if (isTauri) {
      const { Channel } = await import("@tauri-apps/api/core");
      const channel = new Channel<string>();
      channel.onmessage = onDelta;
      return call<Rewrite>("ai_rewrite", { task, text, onDelta: channel });
    }
    const { mockAiRewrite } = await import("./mock");
    return mockAiRewrite(task, text, onDelta);
  },
  aiScience: async (task: SciTask, text: string, onDelta: (d: string) => void): Promise<Rewrite> => {
    if (isTauri) {
      const { Channel } = await import("@tauri-apps/api/core");
      const channel = new Channel<string>();
      channel.onmessage = onDelta;
      return call<Rewrite>("ai_science", { task, text, onDelta: channel });
    }
    return call<Rewrite>("ai_science", { task, text });
  },
  aiCheckSupport: (path: string) => call<SupportVerdict[]>("ai_check_support", { path }),
  aiSummaryCallout: (summary: string) => call<string>("ai_summary_callout", { summary }),
  aiAsk: async (question: string, onDelta: (d: string) => void, mode = ""): Promise<AskAnswer> => {
    if (isTauri) {
      const { Channel } = await import("@tauri-apps/api/core");
      const channel = new Channel<string>();
      channel.onmessage = onDelta;
      return call<AskAnswer>("ai_ask", { question, mode, onDelta: channel });
    }
    const { mockAiAsk } = await import("./mock");
    return mockAiAsk(question, onDelta, mode);
  },
  sourceCitations: (path: string) => call<SourceCitations>("source_citations", { path }),
  citationNetwork: () => call<CitationNetwork>("citation_network"),
  citationFetch: async (limit: number, onProgress: (n: number) => void): Promise<CitationNetwork> => {
    if (isTauri) {
      const { Channel } = await import("@tauri-apps/api/core");
      const channel = new Channel<number>();
      channel.onmessage = onProgress;
      return call<CitationNetwork>("citation_fetch", { limit, onProgress: channel });
    }
    onProgress(2);
    return call<CitationNetwork>("citation_fetch", { limit });
  },
  noteToLatex: (path: string) => call<string>("note_to_latex", { path }),
  assembleThesis: () => call<string>("assemble_thesis"),
  submissionChecklist: (path: string) => call<CheckItem[]>("submission_checklist", { path }),
  addFocus: (minutes: number) => call<number>("add_focus", { minutes }),
  scheduleReview: (path: string, days: number) => call<string | null>("schedule_review", { path, days }),
  weeklyReview: () => call<string>("weekly_review"),
  localApiEnable: (enabled: boolean) => call<string | null>("local_api_enable", { enabled }),
  mcpInfo: () => call<{ exe: string; exists: boolean; config: string }>("mcp_info"),
  capture: (text: string, url: string | null = null) => call<string>("capture", { text, url }),
  inbox: () => call<[string, string, number][]>("inbox"),
  importMarkdown: (dir: string) => call<MdImportReport>("import_markdown", { dir }),
  gitStatus: () => call<GitStatus>("git_status"),
  gitInit: () => call<GitStatus>("git_init"),
  gitCommit: (message: string) => call<GitStatus>("git_commit", { message }),
  gitSync: () => call<GitStatus>("git_sync"),
  readingChain: (path: string) => call<string[]>("reading_chain", { path }),
  bibAudit: () => call<BibIssue[]>("bib_audit"),
  checkCitations: (path: string) => call<CitationCheck[]>("check_citations", { path }),
  styleCheck: (path: string) => call<StyleIssue[]>("style_check", { path }),
  orphanNotes: () => call<string[]>("orphan_notes"),
  duplicateNotes: () => call<[string, string, number][]>("duplicate_notes"),
  dashboard: () => call<Dashboard>("dashboard"),
  aiConcepts: (path: string) => call<ConceptSuggestion[]>("ai_concepts", { path }),
  aiTags: (path: string) => call<string[]>("ai_tags", { path }),
  createConceptNote: (name: string, definition: string, source: string | null) => call<FicheResult>("create_concept_note", { name, definition, source }),
  aiAnswerMarkdown: (text: string, passages: Passage[]) => call<string>("ai_answer_markdown", { text, passages }),
  aiFicheSave: (draft: AiFicheDraft, path?: string) => call<string>("ai_fiche_save", { draft, path: path ?? null }),
  aiJournal: () => call<AiLogEntry[]>("ai_journal"),

  openUrl: (url: string) => call<void>("open_url", { url }),
  revealPath: (path: string, open = false) => call<void>("reveal_path", { path, open }),

  // Extensions
  extensionsList: () => call<ExtensionInfo[]>("extensions_list"),
  extensionEnable: (id: string, on: boolean) => call<ExtensionInfo[]>("extension_enable", { id, on }),
  extensionLoad: (id: string) => call<ExtensionCode>("extension_load", { id }),
  extensionDataGet: (id: string) => call<unknown>("extension_data_get", { id }),
  extensionDataSet: (id: string, value: unknown) => call<void>("extension_data_set", { id, value }),
  extensionCreate: (name: string) => call<string>("extension_create", { name }),
};

/** Listens to changes made to the vault outside the application. */
export async function onVaultChanged(cb: (e: ChangeEvent) => void): Promise<() => void> {
  if (!isTauri) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<ChangeEvent>("vault:changed", (e) => cb(e.payload));
}

/** Zotero saved a change (zotero.sqlite watched on the Rust side). */
export async function onZoteroChanged(cb: () => void): Promise<() => void> {
  if (!isTauri) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen("zotero:changed", () => cb());
}

/** A page was sent to Eloria from the browser (local API). */
export async function onCaptured(cb: (path: string) => void): Promise<() => void> {
  if (!isTauri) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<string>("eloria:captured", (e) => cb(e.payload));
}

export async function pickFolder(title = t("Choisir un dossier")): Promise<string | null> {
  if (!isTauri) return "/demo/Thèse";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ directory: true, multiple: false, title });
  return typeof r === "string" ? r : null;
}

export async function pickSaveFile(defaultPath: string, ext: string, label: string): Promise<string | null> {
  if (!isTauri) return `/demo/export/${defaultPath}`;
  const { save } = await import("@tauri-apps/plugin-dialog");
  return save({ defaultPath, filters: [{ name: label, extensions: [ext] }] });
}

export async function pickFile(exts: string[], label: string): Promise<string | null> {
  if (!isTauri) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ multiple: false, filters: [{ name: label, extensions: exts }] });
  return typeof r === "string" ? r : null;
}

let convert: ((p: string) => string) | null = null;
/** Displayable URL of a local file (vault images). */
export async function assetUrl(absPath: string): Promise<string> {
  if (!isTauri) return absPath.startsWith("/demo/") ? "" : absPath;
  if (!convert) convert = (await import("@tauri-apps/api/core")).convertFileSrc;
  return convert(absPath);
}

/**
 * Window closing: Tauri waits for `cb` to finish before closing
 * (at most `maxMs`, so as never to block closing).
 */
export async function onCloseRequested(cb: () => Promise<unknown>, maxMs = 3000): Promise<() => void> {
  if (!isTauri) return () => {};
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  return getCurrentWindow().onCloseRequested(async () => {
    await Promise.race([cb().catch(() => {}), new Promise((r) => setTimeout(r, maxMs))]);
  });
}

export async function setWindowTitle(title: string) {
  if (!isTauri) {
    document.title = title;
    return;
  }
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().setTitle(title);
}
