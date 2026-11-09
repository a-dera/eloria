<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, tn, translateError } from "../lib/i18n.svelte";
  import type { BibIssue, CitationCheck, StyleIssue } from "../lib/types";
  import Modal from "./Modal.svelte";

  // Local checks (no AI): style of a note, citations of a manuscript, bibliography
  // audit, orphan notes and near-duplicates. One click leads to the place concerned.
  const req = app.checkFor!;
  let style = $state<StyleIssue[]>([]);
  let cites = $state<CitationCheck[]>([]);
  let bib = $state<BibIssue[]>([]);
  let orphans = $state<string[]>([]);
  let dups = $state<[string, string, number][]>([]);
  let checklist = $state<import("../lib/types").CheckItem[]>([]);
  let reliab = $state<import("../lib/types").ReliabilityRow[]>([]);
  let simil = $state<import("../lib/types").SimilarityHit[]>([]);
  /** Repeated passage highlighted in the excerpt. */
  function parts(excerpt: string, matched: string): [string, string, string] {
    const i = excerpt.indexOf(matched);
    return i < 0 ? [excerpt, "", ""] : [excerpt.slice(0, i), matched, excerpt.slice(i + matched.length)];
  }
  let unchecked = $state(0);
  let progress = $state<[number, number] | null>(null);
  let filter = $state<"all" | "issues">("issues");
  const shown = $derived(filter === "all" ? reliab : reliab.filter((r) => r.level === "alerte" || r.level === "attention"));

  async function checkAll() {
    progress = [0, unchecked];
    try {
      await api.checkReliabilityAll((p) => (progress = p));
      [reliab, unchecked] = await api.reliabilityReport();
    } catch (e) {
      error = translateError(e);
    } finally {
      progress = null;
    }
  }
  function openRow(r: import("../lib/types").ReliabilityRow) {
    app.checkFor = null;
    if (r.path) app.openNote(r.path, { newTab: true });
    else if (r.key) app.openView("zotero-item", { key: r.key, title: `@${r.citekey}` });
  }
  let busy = $state(true);
  let error = $state("");

  const TITLES: Record<string, string> = {
    style: t("Style académique"),
    citations: t("Citations du document"),
    bib: t("Audit de la bibliographie"),
    notes: t("Notes orphelines et doublons"),
    checklist: t("Avant soumission"),
    fiabilite: t("Fiabilité des sources"),
    similarite: t("Similarité avec la bibliothèque"),
  };
  const KIND: Record<string, string> = {
    longue: t("Phrase longue"),
    personnelle: t("Première personne"),
    vague: t("Terme vague"),
    répétition: t("Répétition"),
  };

  (async () => {
    try {
      if (req.kind === "style") style = await api.styleCheck(req.path!);
      else if (req.kind === "citations") cites = await api.checkCitations(req.path!);
      else if (req.kind === "bib") bib = await api.bibAudit();
      else if (req.kind === "checklist") checklist = await api.submissionChecklist(req.path!);
      else if (req.kind === "fiabilite") [reliab, unchecked] = await api.reliabilityReport();
      else if (req.kind === "similarite") simil = await api.similarityCheck(req.path!);
      else [orphans, dups] = await Promise.all([api.orphanNotes(), api.duplicateNotes()]);
    } catch (e) {
      error = translateError(e);
    } finally {
      busy = false;
    }
  })();

  function goto(path: string, line?: number) {
    app.checkFor = null;
    if (/\.(tex|bib)$/i.test(path)) app.openView("latex", { path, title: path.split("/").pop() });
    else app.openNote(path, { newTab: true, line });
  }
  const name = (p: string) => p.split("/").pop()!.replace(/\.md$/, "");
  const unknown = $derived(cites.filter((c) => !c.known));
</script>

<Modal title={TITLES[req.kind]} width={760} onclose={() => (app.checkFor = null)}>
  <div class="body">
    {#if busy}
      <p class="faint">{t("Vérification…")}</p>
    {:else if error}
      <p class="error">{error}</p>
    {:else if req.kind === "style"}
      <p class="summary">{style.length ? tn(style.length, "{n} remarque", "{n} remarques") : t("Aucune remarque : style net.")}</p>
      <ul>
        {#each style as s, i (i)}
          <li>
            <button class="loc" onclick={() => goto(req.path!, s.line)}>{t("ligne {n}", { n: s.line })}</button>
            <span class="tag {s.kind}">{KIND[s.kind] ?? s.kind}</span>
            <span class="ex">« {s.excerpt} »</span>
            <span class="msg">{s.message}</span>
          </li>
        {/each}
      </ul>
    {:else if req.kind === "citations"}
      <p class="summary">
        {tn(cites.length, "{n} référence citée", "{n} références citées")} ·
        {#if unknown.length}<span class="bad">{tn(unknown.length, "{n} absente de la bibliothèque", "{n} absentes de la bibliothèque")}</span>{:else}<span class="ok">{t("toutes dans la bibliothèque")}</span>{/if}
      </p>
      <ul>
        {#each cites as c (c.key)}
          <li>
            <span class="key" class:bad={!c.known}>@{c.key}</span>
            {#each c.lines as l (l)}<button class="loc" onclick={() => goto(req.path!, l)}>{t("ligne {n}", { n: l })}</button>{/each}
            {#if !c.known}<span class="msg">{t("ajoute-la à Zotero ou avec « Ajouter une référence », ou corrige la clé")}</span>{/if}
          </li>
        {/each}
      </ul>
    {:else if req.kind === "similarite"}
      <p class="hint">{t("Passages repris mot pour mot (au moins 8 mots) du texte des PDF, des résumés et des annotations de ta bibliothèque. Comparaison locale : elle ne remplace pas un logiciel anti-plagiat, mais t'aide à reformuler ou à citer avant.")}</p>
      <p class="summary">
        {#if simil.length}
          {tn(simil.length, "{n} passage à revoir", "{n} passages à revoir")} · {tn(simil.filter((h) => !h.cited).length, "{n} sans citation", "{n} sans citation")}
        {:else}
          <span class="ok">{t("Aucun passage repris de ta bibliothèque.")}</span>
        {/if}
      </p>
      <ul>
        {#each simil as h, i (i)}
          {@const [a, m, b] = parts(h.excerpt, h.matched)}
          <li class="sim">
            <button class="loc" onclick={() => goto(req.path!, h.line)}>{t("ligne {n}", { n: h.line })}</button>
            <span class="tag">{t("{n} mots identiques", { n: h.matched_words })}</span>
            <span class="key" class:bad={!h.cited}>@{h.citekey}</span>
            {#if h.cited}<span class="ok small">{t("cité")}</span>{:else}<span class="bad small">{t("non cité : cite-le ou reformule")}</span>{/if}
            <span class="msg ex">{a}<mark>{m}</mark>{b}</span>
            <span class="title">{t("Dans « {title} » : {text}", { title: h.title, text: h.source_excerpt })}</span>
          </li>
        {/each}
      </ul>
    {:else if req.kind === "fiabilite"}
      <p class="hint">{t("Des faits à vérifier, pas un verdict : rétractations (Retraction Watch), prépublications, revues peu citées ou absentes des index, articles jamais cités. Données d'OpenAlex.")}</p>
      <div class="rbar">
        {#if unchecked}
          <button class="btn small primary" disabled={!!progress} onclick={checkAll}>
            {progress ? t("{n} sur {m}…", { n: progress[0], m: progress[1] }) : tn(unchecked, "Vérifier {n} document", "Vérifier {n} documents")}
          </button>
        {:else}
          <span class="ok">{t("Toute la bibliothèque est vérifiée.")}</span>
        {/if}
        <span class="grow"></span>
        <button class="chip" class:on={filter === "issues"} onclick={() => (filter = "issues")}>{t("À examiner")} {reliab.filter((r) => r.level === "alerte" || r.level === "attention").length}</button>
        <button class="chip" class:on={filter === "all"} onclick={() => (filter = "all")}>{t("Tous")} {reliab.length}</button>
      </div>
      <ul>
        {#each shown as r (r.doi)}
          <li class="rrow">
            <span class="dot {r.level}"></span>
            <button class="loc" onclick={() => openRow(r)}>@{r.citekey}</button>
            {#if r.cited}<span class="tag">{t("cité dans tes notes")}</span>{/if}
            <span class="title">{r.title}</span>
            {#each r.signals.filter((s) => filter === "all" || s.level === "alerte" || s.level === "attention") as s, i (i)}<span class="msg sig {s.level}">{s.text}</span>{/each}
          </li>
        {:else}
          <li class="faint">{reliab.length ? t("Rien à signaler.") : t("Aucun document vérifié pour l'instant.")}</li>
        {/each}
      </ul>
    {:else if req.kind === "checklist"}
      <p class="summary">{checklist.every((c) => c.ok) ? t("Prêt pour la soumission.") : tn(checklist.filter((c) => !c.ok).length, "{n} point à reprendre.", "{n} points à reprendre.")}</p>
      <ul>
        {#each checklist as c (c.label)}
          <li>
            <span class="mark" class:ok={c.ok} class:bad={!c.ok}>{c.ok ? "✓" : "✗"}</span>
            <span>{c.label}</span>
            {#if c.detail}<span class="msg">{c.detail}</span>{/if}
          </li>
        {/each}
      </ul>
    {:else if req.kind === "bib"}
      <p class="summary">{bib.length ? tn(bib.length, "{n} point à revoir", "{n} points à revoir") : t("Bibliographie propre.")}</p>
      <ul>
        {#each bib as b, i (i)}
          <li>
            <span class="key">@{b.citekey}</span>
            <span class="tag">{b.kind}</span>
            <span class="msg">{b.detail}</span>
            <span class="title">{b.title}</span>
          </li>
        {/each}
      </ul>
    {:else}
      <h3>{t("Notes orphelines")} <span class="faint">{orphans.length}</span></h3>
      <p class="hint">{t("Aucun lien entrant ni sortant, aucune citation : relie-les ou range-les.")}</p>
      <ul>
        {#each orphans as o (o)}<li><button class="loc" onclick={() => goto(o)}>{name(o)}</button><span class="faint">{o}</span></li>{/each}
      </ul>
      <h3>{t("Notes presque identiques")} <span class="faint">{dups.length}</span></h3>
      <ul>
        {#each dups as [a, b, s] (a + b)}
          <li>
            <button class="loc" onclick={() => goto(a)}>{name(a)}</button>
            <span class="faint">≈ {Math.round(s * 100)} %</span>
            <button class="loc" onclick={() => goto(b)}>{name(b)}</button>
          </li>
        {:else}
          <li class="faint">{t("Aucune.")}</li>
        {/each}
      </ul>
    {/if}
  </div>
  {#snippet footer()}
    <button class="btn" onclick={() => (app.checkFor = null)}>{t("Fermer")}</button>
  {/snippet}
</Modal>

<style>
  .body {
    padding: 14px 22px;
    max-height: 64vh;
    overflow: auto;
  }
  .summary {
    margin: 0 0 10px;
    font-weight: 600;
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 10px;
    padding: 6px 0;
    border-bottom: 1px solid var(--line);
    font-size: 13px;
  }
  .loc {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent-text);
    font: 600 12.5px var(--font-ui);
    cursor: pointer;
  }
  .loc:hover {
    text-decoration: underline;
  }
  .tag {
    padding: 0 7px;
    border-radius: 999px;
    background: var(--bg-sunk);
    font-size: 11.5px;
    color: var(--text-soft);
  }
  .ex {
    font-style: italic;
  }
  .msg {
    color: var(--text-soft);
    flex-basis: 100%;
  }
  .key {
    font-family: var(--font-mono);
    font-size: 12.5px;
  }
  .title {
    flex-basis: 100%;
    color: var(--text-faint);
    font-size: 12px;
  }
  .bad {
    color: var(--danger);
  }
  .mark {
    font-weight: 700;
    width: 14px;
  }
  .rbar {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 8px;
  }
  .grow {
    flex: 1;
  }
  .chip {
    border: 1px solid var(--line);
    border-radius: 999px;
    background: none;
    padding: 1px 9px;
    font: 12px var(--font-ui);
    color: var(--text-soft);
    cursor: pointer;
  }
  .chip.on {
    border-color: var(--accent);
    color: var(--text);
    background: var(--accent-soft);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
    align-self: center;
    background: var(--text-faint);
  }
  .dot.ok {
    background: var(--success);
  }
  .dot.attention {
    background: var(--warning);
  }
  .dot.alerte {
    background: var(--danger);
  }
  .sig.alerte {
    color: var(--danger);
  }
  .sig.attention {
    color: var(--warning);
  }
  .sim mark {
    background: var(--mark);
    color: inherit;
    border-radius: 2px;
  }
  .small {
    font-size: 12px;
  }
  .ok {
    color: var(--success, var(--accent-text));
  }
  .faint {
    color: var(--text-faint);
    font-weight: 400;
  }
  .error {
    color: var(--danger);
  }
  .hint {
    font-size: 12.5px;
    color: var(--text-soft);
    margin: 0 0 4px;
  }
  h3 {
    margin: 12px 0 2px;
    font-size: 14px;
  }
</style>
