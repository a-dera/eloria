<script lang="ts">
  import { t, translateError } from "../lib/i18n.svelte";
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { editors } from "../lib/editor/registry";
  import type { ItemDetail, Work, ZAnnotation } from "../lib/types";
  import Icon from "./Icon.svelte";
  import { itemTypeLabel, propText } from "../lib/util";

  // `inNote`: card shown at the top of a reading note (instead of the raw properties),
  // the note itself follows below. `onEditProps` puts the cursor in the properties.
  // `sourcePath`: the card of a vault source note, coming from Zotero or added to
  // Eloria's library (DOI, Crossref); the item is then read from the source.
  let { itemKey = "", sourcePath, inNote = false, onEditProps }: { itemKey?: string; sourcePath?: string; inNote?: boolean; onEditProps?: () => void } = $props();
  /** Properties of the source (status, chapters, tags…), when the card is inside a note. */
  const noteMeta = $derived(
    inNote ? (sourcePath ? app.notes.find((n) => n.path === sourcePath) : app.notes.find((n) => n.frontmatter.zotero === itemKey)) : undefined,
  );
  const PROPS = ["statut", "priorite", "chapitres"];
  let noticeOpen = $state(false);
  let detail = $state<ItemDetail | null>(null);
  let error = $state("");
  let citing = $state<{ path: string; title: string }[]>([]);

  $effect(() => {
    void app.zoteroStatus;
    if (sourcePath) void app.revision;
    (sourcePath ? api.sourceDetail(sourcePath) : api.zoteroItem(itemKey)).then((d) => (detail = d)).catch((e) => (error = translateError(e)));
  });

  $effect(() => {
    if (!detail) return;
    void app.revision;
    const ck = detail.item.citekey;
    api.search(`"${ck}"`, 50).then((hits) => (citing = hits.map((h) => ({ path: h.path, title: h.title })))).catch(() => {});
  });

  const fiche = $derived(detail ? app.fiches[detail.item.citekey.toLowerCase()] : undefined);
  /** Reading notes attached to this source. */
  let attached = $state<string[]>([]);
  $effect(() => {
    void app.revision;
    (sourcePath ? api.fichesOfSource(sourcePath) : api.fichesOf(itemKey)).then((f) => (attached = f)).catch(() => (attached = []));
  });
  /** Research questions that cite or link to this item. */
  let questions = $state<[string, string, string][]>([]);
  $effect(() => {
    void app.revision;
    const ck = detail?.item.citekey;
    if (ck) api.questionsFor(ck).then((q) => (questions = q)).catch(() => (questions = []));
  });

  /** Reliability (last stored check; nothing is requested online without a click). */
  let reliab = $state<import("../lib/types").ReliabilityView | null>(null);
  let checking = $state(false);
  $effect(() => {
    const doi = detail?.item.doi;
    reliab = null;
    if (doi) api.reliabilityOf(doi).then((r) => (reliab = r)).catch(() => {});
  });
  async function checkReliab() {
    if (!detail?.item.doi) return;
    checking = true;
    try {
      reliab = await api.checkReliability(detail.item.doi);
    } catch (e) {
      app.toast(translateError(e), "error");
    } finally {
      checking = false;
    }
  }

  function newFiche() {
    if (sourcePath) app.createFicheFromSource(sourcePath);
    else if (detail) app.createFiche(detail.item.key);
  }

  /** Close articles (Semantic Scholar, OpenAlex): real articles, never made up. */
  let similar = $state<Work[] | null>(null);
  let similarBusy = $state(false);
  let similarError = $state("");
  async function loadSimilar() {
    if (!detail) return;
    similarBusy = true;
    similarError = "";
    try {
      similar = await api.similarWorks(detail.item.doi || null, detail.item.title);
    } catch (e) {
      similarError = translateError(e);
    } finally {
      similarBusy = false;
    }
  }
  /** References and citations (OpenAlex), loaded on demand. */
  let cites = $state<import("../lib/types").SourceCitations | null>(null);
  let citesBusy = $state(false);
  let citesError = $state("");
  async function loadCitations() {
    if (!sourcePath) return;
    citesBusy = true;
    citesError = "";
    try {
      cites = await api.sourceCitations(sourcePath);
    } catch (e) {
      citesError = translateError(e);
    } finally {
      citesBusy = false;
    }
  }
  const inLib = (w: Work) => (w.doi && cites ? cites.library[w.doi.toLowerCase()] : undefined);
  function openLib(ck: string) {
    const p = app.fiches[ck.toLowerCase()];
    if (p) app.openNote(p, { newTab: true });
  }
  const workAuthors = (w: Work) => (w.authors.length > 2 ? `${w.authors[0][1]} et al.` : w.authors.map((a) => a[1]).join(" & "));
  const scholar = (title: string) => `https://scholar.google.com/scholar?q=${encodeURIComponent(title)}`;
  /** PDF: the vault copy (the source's pdf property) first. */
  function openPdf(title: string, fallbackKey?: string) {
    const local = noteMeta?.frontmatter.pdf;
    if (typeof local === "string" && local) app.openPdfFile(app.vaultAbs(local), title);
    else if (fallbackKey) app.openZoteroPdf(fallbackKey, title);
  }

  function quote(a: ZAnnotation) {
    const ed = editors.active();
    if (!ed || !detail) return app.toast(t("Ouvre une note pour y insérer l'extrait"), "info");
    const page = a.page_label ? `, p. ${a.page_label}` : "";
    const body = a.text ? `> ${a.text.trim().replace(/\n/g, "\n> ")}\n>\n> [@${detail.item.citekey}${page}]` : `[@${detail.item.citekey}${page}]`;
    ed.insert(`\n${body}\n`);
  }

  const authors = $derived(detail?.item.creators.map((c) => (c.first_name ? `${c.first_name} ${c.last_name}` : c.last_name)).join(", ") ?? "");
  /** Physical page of the annotation (1 = first): Zotero position, otherwise the displayed number. */
  const pageOf = (a: ZAnnotation) => (a.page_index != null ? a.page_index + 1 : Number.parseInt(a.page_label) || undefined);

  const pdf = $derived(detail?.item.attachments.find((a) => a.content_type.includes("pdf") || a.content_type.includes("epub")));
</script>

<div class="zi" class:in-note={inNote}>
  {#if error && !inNote}<p class="empty">{error}</p>{/if}
  {#if detail}
    {@const it = detail.item}
    <article>
      <p class="type">{itemTypeLabel(it.item_type)}{it.publication ? ` · ${it.publication}` : ""}</p>
      {#if !inNote}<h1>{it.title}</h1>{/if}
      <p class="authors">{authors}{it.year ? ` (${it.year})` : ""}</p>

      {#if noteMeta}
        <div class="note-props">
          {#each PROPS.filter((k) => noteMeta.frontmatter[k] !== undefined && noteMeta.frontmatter[k] !== "") as k (k)}
            <span class="np"><span class="k">{k}</span> {propText(noteMeta.frontmatter[k])}</span>
          {/each}
          {#if onEditProps}<button class="btn small ghost" onclick={onEditProps}>{t("Modifier les propriétés")}</button>{/if}
        </div>
      {/if}

      <div class="actions">
        {#if !inNote}
          {#if fiche}
            <button class="btn" onclick={() => app.openNote(fiche!)}>{t("Ouvrir la source")}</button>
          {:else}
            <button class="btn" onclick={() => app.importSource(it.key)}>{t("Importer dans le coffre")}</button>
          {/if}
        {/if}
        {#if pdf || noteMeta?.frontmatter.pdf}<button class="btn" onclick={() => openPdf(it.title, pdf?.key)}><Icon name="pdf" size={14} /> {t("Ouvrir le PDF")}</button>{/if}
        <button class="btn primary" onclick={newFiche}><Icon name="file-plus" size={14} /> {t("Créer une fiche")}</button>
        <button class="btn" title={t("Métadonnées, texte du PDF, fiabilité, tags et liens proposés, puis fiche")} onclick={() => (app.flowFor = sourcePath ? { path: sourcePath, title: it.title } : { key: it.key, title: it.title })}>{t("Préparer…")}</button>
        <button class="btn" onclick={() => editors.active()?.insert(`[@${it.citekey}]`)}>Citer [@{it.citekey}]</button>
        {#if it.doi}<button class="btn" onclick={() => api.openUrl(`https://doi.org/${it.doi}`)}>DOI</button>{/if}
        {#if sourcePath && it.doi && (!it.abstract_note || (noteMeta?.tags.length ?? 0) <= 1)}
          <button class="btn" title={t("Résumé et mots-clés depuis OpenAlex, Semantic Scholar, DataCite ou la page de l'éditeur")} onclick={() => app.completeSource(sourcePath!)}>{t("Compléter les métadonnées")}</button>
        {/if}
      </div>

      {#if attached.length}
        <div class="attached">
          <span class="lbl-small">{t("Fiches de cette source")}</span>
          {#each attached as f (f)}
            <button class="fiche-link" title={app.noteByPath(f)?.title ?? f} onclick={() => app.openNote(f, { newTab: true })}><Icon name="note" size={13} /> {f.split("/").pop()?.replace(/\.md$/, "") ?? f}</button>
          {/each}
        </div>
      {/if}
      {#if questions.length}
        <div class="attached qlist">
          <span class="lbl-small">{t("Questions de recherche")}</span>
          {#each questions as [qp, qt, qs] (qp)}
            <button class="fiche-link" title={qs} onclick={() => app.openNote(qp, { newTab: true })}><Icon name="list" size={13} /> {qt}</button>
          {/each}
        </div>
      {/if}

      {#if it.doi}
        <div class="reliab">
          <span class="lbl-small">{t("Fiabilité")}</span>
          {#if reliab}
            <ul>
              {#each reliab.signals as sg, i (i)}<li class={sg.level}><span class="dot"></span>{sg.text}</li>{/each}
            </ul>
            <span class="faint small">{t("Vérifié le {date} (OpenAlex, Retraction Watch)", { date: reliab.checked })}</span>
            <button class="btn small ghost" disabled={checking} onclick={checkReliab}>{checking ? t("Vérification…") : t("Revérifier")}</button>
          {:else}
            <button class="btn small" disabled={checking} onclick={checkReliab}>{checking ? t("Vérification…") : t("Vérifier la fiabilité")}</button>
            <span class="faint small">{t("Rétractation, type de publication, revue et citations, d'après OpenAlex.")}</span>
          {/if}
        </div>
      {/if}

      {#if it.abstract_note}
        <h2>{t("Résumé")}</h2>
        <p class="abstract">{it.abstract_note}</p>
      {/if}

      {#if inNote && noteMeta?.tags.length}
        <div class="tags">{#each noteMeta.tags as tg (tg)}<button class="tagbtn" onclick={() => { app.searchQuery = `#${tg}`; app.leftPanel = "search"; app.leftOpen = true; }}>#{tg}</button>{/each}</div>
      {:else if it.tags.length}
        <div class="tags">{#each it.tags as tg (tg)}<span>{tg}</span>{/each}</div>
      {/if}

      {#if it.key}<h2>{t("Annotations")} <span class="faint">{detail.annotations.length}</span></h2>{/if}
      {#each detail.annotations as a (a.key)}
        <figure class="ann" style:--c={a.color || "#ffd400"}>
          {#if a.text}<blockquote>{a.text}</blockquote>{/if}
          {#if a.comment}<figcaption>{a.comment}</figcaption>{/if}
          <div class="ann-foot">
            <span class="faint">{t("p.")} {a.page_label || "?"} · {a.kind}</span>
            {#each a.tags as tg (tg)}<span class="atag">#{tg}</span>{/each}
            <span class="grow"></span>
            <button class="btn small ghost" onclick={() => quote(a)}>{t("Insérer dans la note")}</button>
            <button class="btn small ghost" onclick={() => app.openZoteroPdf(a.attachment_key, it.title, pageOf(a))}>{t("Voir dans le PDF")}</button>
          </div>
        </figure>
      {:else}
        {#if it.key}<p class="faint">{t("Aucune annotation dans Zotero pour ce document.")}</p>{/if}
      {/each}

      <h2>{t("Cité dans")}</h2>
      {#each citing as c (c.path)}
        <button class="cite" onclick={() => app.openNote(c.path)}>{c.title}</button>
      {:else}
        <p class="faint">{t("Pas encore cité dans le coffre.")}</p>
      {/each}

      {#if sourcePath}
        <h2>{t("Références et citations")}</h2>
        {#if !cites}
          <p class="faint">
            <button class="btn small" disabled={citesBusy} onclick={loadCitations}>{citesBusy ? t("Recherche…") : t("Afficher les références et les citations")}</button>
            {t("OpenAlex : ce que l'article cite, et qui le cite.")}
          </p>
          {#if citesError}<p class="error">{citesError}</p>{/if}
        {:else if !cites.info}
          <p class="faint">{t("Article introuvable dans OpenAlex.")}</p>
        {:else}
          <p class="faint">{t("Cité {n} fois", { n: cites.info.cited_by_count.toLocaleString() })} · <button class="linkish" onclick={() => app.openView("citations")}>{t("Réseau de citations de la bibliothèque")}</button></p>
          {#each [{ label: t("Références ({n})", { n: cites.info.references.length }), list: cites.info.references }, { label: t("Cité par (les plus cités)"), list: cites.info.cited_by }] as { label, list } (label)}
            <details>
              <summary>{label}</summary>
              {#each list as w (w.doi || w.title)}
                <div class="work">
                  <button class="w-title" onclick={() => api.openUrl(w.url || scholar(w.title))}>{w.title}</button>
                  <div class="w-meta">{workAuthors(w)}{w.year ? ` (${w.year})` : ""}{w.venue ? ` · ${w.venue}` : ""}</div>
                  <div class="w-actions">
                    {#if inLib(w)}
                      <button class="btn small ghost lib" onclick={() => openLib(inLib(w)!)}>{t("Dans ta bibliothèque : @{key}", { key: inLib(w) ?? "" })}</button>
                    {:else}
                      <button class="btn small" onclick={() => app.addReference($state.snapshot(w) as Work)}>{t("Ajouter à ma bibliothèque")}</button>
                    {/if}
                  </div>
                </div>
              {/each}
            </details>
          {/each}
        {/if}
      {/if}

      <h2>{t("Articles similaires")}</h2>
      {#if similar === null}
        <p class="faint">
          <button class="btn small" disabled={similarBusy} onclick={loadSimilar}>{similarBusy ? t("Recherche…") : t("Chercher des articles similaires")}</button>
          {t("Semantic Scholar et OpenAlex : de vrais articles, à ajouter d'un clic.")}
        </p>
        {#if similarError}<p class="error">{similarError}</p>{/if}
      {:else}
        {#each similar as w (w.doi || w.title)}
          <div class="work">
            <button class="w-title" title={t("Ouvrir dans le navigateur")} onclick={() => api.openUrl(w.url || scholar(w.title))}>{w.title}</button>
            <div class="w-meta">
              {workAuthors(w)}{w.year ? ` (${w.year})` : ""}{w.venue ? ` · ${w.venue}` : ""}{w.citations != null ? ` · ${t("{n} citations", { n: w.citations })}` : ""}
            </div>
            <div class="w-actions">
              <button class="btn small" onclick={() => app.addReference($state.snapshot(w) as Work)}>{t("Ajouter à ma bibliothèque")}</button>
              <button class="btn small ghost" onclick={() => api.openUrl(scholar(w.title))}>Google Scholar</button>
            </div>
          </div>
        {:else}
          <p class="faint">{t("Aucun article similaire trouvé.")}</p>
        {/each}
      {/if}

      <h2><button class="fold" onclick={() => (noticeOpen = !noticeOpen)}>{noticeOpen || !inNote ? "▾" : "▸"} {t("Notice")}</button></h2>
      {#if noticeOpen || !inNote}
        <dl>
          <dt>{t("Clé")}</dt><dd>@{it.citekey}</dd>
          {#each Object.entries(it.fields).filter(([k]) => !["title", "abstractNote", "extra"].includes(k)) as [k, v] (k)}
            <dt>{k}</dt><dd>{v}</dd>
          {/each}
        </dl>
      {/if}
    </article>
  {/if}
</div>

<style>
  .zi {
    flex: 1;
    overflow: auto;
    user-select: text;
  }
  /* At the top of a reading note: no own scrolling, the note follows in the same page. */
  .zi.in-note {
    overflow: visible;
    font-family: var(--font-ui);
    font-size: 14px;
    line-height: 1.5;
    white-space: normal;
    cursor: auto;
  }
  .zi.in-note article {
    max-width: none;
    margin: 0 0 8px;
    padding: 18px 20px 6px;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--bg-soft);
  }
  .error {
    color: var(--danger);
  }
  .linkish {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent-text);
    font: inherit;
    cursor: pointer;
  }
  .lib {
    color: var(--accent-text);
  }
  details {
    margin: 4px 0;
  }
  details summary {
    cursor: pointer;
    font-size: 13px;
    color: var(--text-soft);
    padding: 4px 0;
  }
  .work {
    padding: 8px 0;
    border-bottom: 1px solid var(--line);
  }
  .w-title {
    display: block;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    font: inherit;
    font-weight: 600;
    text-align: left;
    cursor: pointer;
  }
  .w-title:hover {
    color: var(--accent-text);
    text-decoration: underline;
  }
  .w-meta {
    font-size: 12.5px;
    color: var(--text-soft);
    margin: 2px 0 4px;
  }
  .w-actions {
    display: flex;
    gap: 6px;
  }
  .reliab {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 10px;
    margin: 12px 0 4px;
  }
  .reliab ul {
    flex-basis: 100%;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 13px;
  }
  .reliab li {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 2px 0;
    color: var(--text-soft);
  }
  .reliab .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--text-faint);
    transform: translateY(-1px);
  }
  .reliab li.ok .dot {
    background: var(--success);
  }
  .reliab li.attention .dot {
    background: var(--warning);
  }
  .reliab li.attention {
    color: var(--text);
  }
  .reliab li.alerte .dot {
    background: var(--danger);
  }
  .reliab li.alerte {
    color: var(--danger);
    font-weight: 600;
  }
  .reliab .small {
    font-size: 12px;
  }
  .attached {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 8px;
    margin: 12px 0 0;
  }
  .lbl-small {
    font-size: 12px;
    color: var(--text-faint);
    margin-right: 4px;
  }
  .fiche-link {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 9px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: var(--bg-raised);
    color: var(--accent-text);
    font-size: 12.5px;
    cursor: pointer;
  }
  .fiche-link:hover {
    border-color: var(--accent);
  }
  .note-props {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
    margin: 6px 0 10px;
    font-size: 12.5px;
  }
  .np .k {
    color: var(--text-faint);
  }
  .tagbtn {
    border: 0;
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent-text);
    font-size: 12px;
    cursor: pointer;
  }
  .fold {
    border: 0;
    background: transparent;
    padding: 0;
    font: inherit;
    color: inherit;
    cursor: pointer;
  }
  article {
    max-width: 760px;
    margin: 0 auto;
    padding: 32px 40px 80px;
  }
  .type {
    color: var(--text-faint);
    margin: 0 0 6px;
    font-size: 12.5px;
  }
  h1 {
    font-family: var(--font-editor);
    font-size: 28px;
    line-height: 1.25;
    font-weight: 600;
    margin: 0 0 8px;
    letter-spacing: -0.01em;
  }
  .authors {
    font-family: var(--font-editor);
    font-size: 16px;
    color: var(--text-soft);
    margin: 0 0 18px;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 8px;
  }
  h2 {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-soft);
    margin: 30px 0 10px;
  }
  .abstract {
    font-family: var(--font-editor);
    font-size: 15.5px;
    line-height: 1.7;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-top: 12px;
  }
  .tags span {
    font-size: 12px;
    padding: 1px 8px;
    border-radius: 10px;
    background: var(--bg-sunk);
  }
  .ann {
    margin: 0 0 12px;
    padding: 10px 14px;
    border-left: 3px solid var(--c);
    background: color-mix(in srgb, var(--c) 8%, transparent);
    border-radius: 0 var(--radius) var(--radius) 0;
  }
  blockquote {
    margin: 0;
    font-family: var(--font-editor);
    font-size: 15px;
    line-height: 1.6;
  }
  figcaption {
    margin-top: 6px;
    font-size: 13px;
    color: var(--text-soft);
  }
  figcaption::before {
    content: "Commentaire : ";
    font-weight: 600;
  }
  .ann-foot {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
    font-size: 12px;
  }
  .atag {
    color: var(--accent-text);
  }
  .grow {
    flex: 1;
  }
  .cite {
    display: block;
    border: 0;
    background: transparent;
    padding: 3px 0;
    color: var(--accent-text);
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 4px 16px;
    font-size: 12.5px;
  }
  dt {
    color: var(--text-faint);
  }
  dd {
    margin: 0;
    word-break: break-word;
  }
</style>
