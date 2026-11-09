<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, tk, tn, translateError } from "../lib/i18n.svelte";
  import type { AskAnswer, ItemSummary, Review } from "../lib/types";
  import MatrixView from "./MatrixView.svelte";

  // Literature review: articles to read, included, excluded; matrix of the included ones;
  // state-of-the-art draft written by the AI from the included reading notes only.
  let { path, visible = true }: { path: string; visible?: boolean } = $props();
  let review = $state<Review | null>(null);
  let error = $state("");
  let query = $state("");
  let tab = $state<"articles" | "matrice" | "brouillon">("articles");
  let draft = $state("");
  let answer = $state<AskAnswer | null>(null);
  let drafting = $state(false);

  const STATUS = [
    { id: "à lire", label: tk("À lire") },
    { id: "inclus", label: tk("Inclus") },
    { id: "exclu", label: tk("Exclus") },
  ];

  $effect(() => {
    void app.revision;
    if (!visible) return;
    api.review(path).then((r) => (review = r)).catch((e) => (error = translateError(e)));
  });

  async function setStatus(citekey: string, statut: string) {
    try {
      review = await api.reviewSetArticle(path, citekey, statut);
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }

  /** Library (Zotero and Eloria's own sources) not yet in the review. */
  const candidates = $derived.by((): ItemSummary[] => {
    const q = query.trim().toLowerCase();
    if (!q || !review) return [];
    const inReview = new Set(review.articles.map((a) => a.citekey.toLowerCase()));
    // Zotero, plus the sources added without Zotero (DOI, Crossref).
    const known = new Set(app.zoteroItems.map((i) => i.citekey.toLowerCase()));
    const own: ItemSummary[] = app.notes
      .filter((n) => n.frontmatter.type === "source" && typeof n.frontmatter.citekey === "string" && !known.has(String(n.frontmatter.citekey).toLowerCase()))
      .map((n) => ({
        key: n.path, item_type: String(n.frontmatter.type_document ?? ""), title: n.title, citekey: String(n.frontmatter.citekey),
        authors: Array.isArray(n.frontmatter.auteurs) ? n.frontmatter.auteurs.map(String).join(", ") : "", year: Number(n.frontmatter.annee) || null,
        publication: String(n.frontmatter.publication ?? ""), annotation_count: 0,
      }));
    return [...app.zoteroItems, ...own]
      .filter((i) => !inReview.has(i.citekey.toLowerCase()))
      .filter((i) => `${i.title} ${i.authors} ${i.citekey} ${i.year ?? ""}`.toLowerCase().includes(q))
      .slice(0, 12);
  });

  const included = $derived(review ? review.articles.filter((a) => a.statut === "inclus").map((a) => a.citekey) : []);
  const count = (s: string) => review?.articles.filter((a) => a.statut === s).length ?? 0;

  async function runDraft() {
    tab = "brouillon";
    draft = "";
    answer = null;
    drafting = true;
    try {
      answer = await api.aiReviewDraft(path, (d) => (draft += d));
    } catch (e) {
      app.toast(translateError(e), "error");
    } finally {
      drafting = false;
    }
  }

  async function saveDraft() {
    if (!answer || !review) return;
    const body = await api.aiAnswerMarkdown(answer.text, answer.passages);
    const title = t("État de l'art : {title}", { title: review.title });
    await app.newNote(undefined, title, `# ${title}\n\n> [!info] ${t("Brouillon rédigé par l'IA à partir des fiches incluses dans « {review} ». Relis chaque phrase avant de l'utiliser.", { review: review.title })}\n\n${body.trim()}\n`);
  }
</script>

<div class="rv">
  {#if error}<p class="error">{error}</p>{/if}
  {#if !review && !error}<p class="loading" role="status">{t("Chargement de la revue…")}</p>{/if}
  {#if review}
    <header>
      <div>
        <h2>{review.title}</h2>
        {#if review.question}<p class="q">{review.question}</p>{/if}
        <p class="crit">
          {#if review.inclusion.length}<span><b>{t("Inclusion :")}</b> {review.inclusion.join(" ; ")}</span>{/if}
          {#if review.exclusion.length}<span><b>{t("Exclusion :")}</b> {review.exclusion.join(" ; ")}</span>{/if}
          <button class="link" onclick={() => app.openNote(path, { newTab: true })}>{t("Modifier la question et les critères")}</button>
        </p>
      </div>
      <div class="tabs" role="tablist">
        <button role="tab" aria-selected={tab === "articles"} class:active={tab === "articles"} onclick={() => (tab = "articles")}>{t("Articles")} ({review.articles.length})</button>
        <button role="tab" aria-selected={tab === "matrice"} class:active={tab === "matrice"} onclick={() => (tab = "matrice")}>{t("Matrice des inclus")} ({included.length})</button>
        <button role="tab" aria-selected={tab === "brouillon"} class:active={tab === "brouillon"} onclick={() => (tab = "brouillon")}>{t("Brouillon d'état de l'art")}</button>
      </div>
    </header>

    {#if tab === "articles"}
      <div class="add">
        <input class="input" bind:value={query} placeholder={t("Ajouter un article de la bibliothèque (titre, auteur, clé)…")} aria-label={t("Ajouter un article")} />
        {#each candidates as c (c.key)}
          <div class="cand">
            <span class="ct">{c.authors}{c.year ? ` (${c.year})` : ""} · {c.title}</span>
            <button class="btn small" onclick={() => setStatus(c.citekey, "à lire")}>{t("Ajouter")}</button>
          </div>
        {/each}
      </div>
      <div class="cols">
        {#each STATUS as s (s.id)}
          <section>
            <h3>{t(s.label)} <span class="faint">{count(s.id)}</span></h3>
            {#each review.articles.filter((a) => a.statut === s.id) as a (a.citekey)}
              <div class="art">
                <button class="at" onclick={() => (a.source ? app.openNote(a.source, { newTab: true }) : undefined)} title={a.title}>
                  <b>{a.author_year || `@${a.citekey}`}</b>
                  <span>{a.title}</span>
                </button>
                <div class="aa">
                  {#each STATUS.filter((x) => x.id !== s.id) as x (x.id)}
                    <button class="btn small ghost" onclick={() => setStatus(a.citekey, x.id)}>{t(x.label)}</button>
                  {/each}
                  {#if a.fiches.length}
                    <button class="btn small ghost" onclick={() => app.openNote(a.fiches[0], { newTab: true })}>{tn(a.fiches.length, "{n} fiche", "{n} fiches")}</button>
                  {:else}
                    <span class="faint small">{t("sans fiche")}</span>
                  {/if}
                  <button class="btn small ghost" title={t("Retirer de la revue")} onclick={() => setStatus(a.citekey, "")}>×</button>
                </div>
              </div>
            {:else}
              <p class="faint small">{t("Aucun article.")}</p>
            {/each}
          </section>
        {/each}
      </div>
    {:else if tab === "matrice"}
      <div class="mxwrap">
        {#if included.length}<MatrixView citekeys={included} {visible} />{:else}<p class="faint">{t("Inclus des articles pour voir leur matrice.")}</p>{/if}
      </div>
    {:else}
      <div class="draft">
        <p class="hint">{t("L'IA rédige à partir des seules fiches et résumés des articles inclus ; chaque phrase renvoie à son article. Rien n'est enregistré sans toi.")}</p>
        <div class="row">
          <button class="btn primary" disabled={drafting || !included.length || !app.settings?.ai_enabled} onclick={runDraft}>{drafting ? t("Rédaction…") : t("Rédiger le brouillon (IA)")}</button>
          {#if drafting}<button class="btn" onclick={() => api.aiCancel()}>{t("Arrêter")}</button>{/if}
          {#if answer}<button class="btn" onclick={saveDraft}>{t("Enregistrer en note")}</button>{/if}
          {#if !app.settings?.ai_enabled}<span class="faint small">{t("IA désactivée (Réglages › Intelligence artificielle).")}</span>{/if}
        </div>
        {#if answer?.unverified.length}<p class="error">{t("À vérifier, absent de tes extraits et de ta bibliothèque : {list}", { list: answer.unverified.join(", ") })}</p>{/if}
        {#if draft}<pre class="out">{draft}</pre>{/if}
      </div>
    {/if}
  {/if}
</div>

<style>
  .rv {
    height: 100%;
    overflow: auto;
    padding: 16px 22px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  h2 {
    margin: 0;
    font-size: 20px;
  }
  .q {
    margin: 4px 0;
    font-size: 15px;
    color: var(--text-soft);
  }
  .crit {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    font-size: 12.5px;
    color: var(--text-soft);
    margin: 4px 0 8px;
  }
  .link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent-text);
    font: inherit;
    cursor: pointer;
  }
  .tabs {
    display: flex;
    gap: 4px;
    border-bottom: 1px solid var(--line);
  }
  .tabs button {
    padding: 6px 12px;
    border: 0;
    border-bottom: 2px solid transparent;
    background: none;
    color: var(--text-soft);
    font: 13px var(--font-ui);
    cursor: pointer;
  }
  .tabs button.active {
    color: var(--accent-text);
    border-bottom-color: var(--accent);
  }
  .add .input {
    width: 100%;
  }
  .cand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 2px;
    font-size: 13px;
  }
  .ct {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 14px;
  }
  section {
    background: var(--bg-soft);
    border-radius: var(--radius);
    padding: 10px;
    min-height: 120px;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 13.5px;
  }
  .art {
    background: var(--bg-raised);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 7px 9px;
    margin-bottom: 6px;
  }
  .at {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    border: 0;
    background: none;
    padding: 0;
    color: var(--text);
    font: 12.5px var(--font-ui);
    text-align: left;
    cursor: pointer;
  }
  .at span {
    color: var(--text-soft);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .aa {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 6px;
    align-items: center;
  }
  .mxwrap {
    flex: 1;
    min-height: 420px;
    display: flex;
  }
  .mxwrap :global(.mx) {
    padding: 0;
    flex: 1;
  }
  .draft .row {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .out {
    margin-top: 12px;
    padding: 14px 16px;
    background: var(--bg-soft);
    border-radius: var(--radius);
    font: 15px/1.65 var(--font-editor);
    white-space: pre-wrap;
    user-select: text;
  }
  .hint {
    font-size: 12.5px;
    color: var(--text-soft);
  }
  .faint {
    color: var(--text-faint);
  }
  .small {
    font-size: 12px;
  }
  .error {
    color: var(--danger);
  }
</style>
