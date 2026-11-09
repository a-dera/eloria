<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, tn, translateError } from "../lib/i18n.svelte";
  import type { ReliabilityView } from "../lib/types";
  import Modal from "./Modal.svelte";

  // "New document" flow: source in the vault, metadata completed, PDF
  // text, reliability, then suggested tags and links (no AI) and a reading note ready to approve.
  // Each step says what it did; nothing is added to the note without your consent.
  const req = app.flowFor!;
  type Step = { id: string; label: string; state: "wait" | "run" | "done" | "skip" | "fail"; detail: string };
  let steps = $state<Step[]>([
    { id: "source", label: t("Source dans le coffre"), state: "wait", detail: "" },
    { id: "meta", label: t("Métadonnées (résumé, mots-clés)"), state: "wait", detail: "" },
    { id: "pdf", label: t("Texte du PDF"), state: "wait", detail: "" },
    { id: "reliab", label: t("Fiabilité"), state: "wait", detail: "" },
    { id: "suggest", label: t("Tags et liens proposés"), state: "wait", detail: "" },
  ]);
  let path = $state(req.path ?? "");
  let tags = $state<{ name: string; on: boolean }[]>([]);
  let links = $state<{ path: string; title: string; on: boolean }[]>([]);
  let fiches = $state<string[]>([]);
  let reliab = $state<ReliabilityView | null>(null);
  let ready = $state(false);
  let busy = $state(false);

  function set(id: string, state: Step["state"], detail = "") {
    const s = steps.find((x) => x.id === id)!;
    s.state = state;
    s.detail = detail;
  }

  (async () => {
    try {
      // 1. Source
      set("source", "run");
      if (!path && req.key) {
        const r = await api.importSource(req.key);
        path = r.path;
        set("source", "done", r.created ? t("créée : {p}", { p: r.path }) : t("existait déjà : {p}", { p: r.path }));
      } else set("source", "done", path);
      await app.refresh();
      const detail = await api.sourceDetail(path);
      const it = detail.item;
      // 2. Metadata
      if (it.doi && (!it.abstract_note || (app.noteByPath(path)?.tags.length ?? 0) <= 1)) {
        set("meta", "run");
        try {
          const changed = await api.completeSource(path);
          set("meta", "done", changed ? t("complétées en ligne") : t("rien de plus en ligne"));
        } catch (e) {
          set("meta", "fail", translateError(e));
        }
      } else set("meta", "skip", it.doi ? t("déjà complètes") : t("pas de DOI"));
      await app.refresh();
      // 3. PDF text
      const pdf = String(app.noteByPath(path)?.frontmatter.pdf ?? "");
      if (pdf) {
        set("pdf", "run");
        const n = await app.indexPdfText(pdf, true);
        set("pdf", "done", n ? t("extrait ; résumé lu dans le PDF") : t("extrait (recherche, Ask Eloria)"));
      } else set("pdf", "skip", t("pas de PDF dans le coffre"));
      // 4. Reliability
      if (it.doi) {
        set("reliab", "run");
        try {
          reliab = await api.checkReliability(it.doi);
          const worst = reliab.signals.find((s) => s.level === reliab!.level);
          set("reliab", "done", worst?.text ?? "");
        } catch (e) {
          set("reliab", "fail", translateError(e));
        }
      } else set("reliab", "skip", t("pas de DOI"));
      // 5. Propositions
      set("suggest", "run");
      const [tg, rel, fs] = await Promise.all([api.proposeTags(path), api.relatedNotes(path), api.fichesOfSource(path)]);
      tags = tg.map((name) => ({ name, on: true }));
      links = rel.map(([p, title], i) => ({ path: p, title, on: i < 3 }));
      fiches = fs;
      set("suggest", "done", `${tn(tags.length, "{n} tag", "{n} tags")}, ${tn(links.length, "{n} note proche", "{n} notes proches")}`);
      ready = true;
    } catch (e) {
      const cur = steps.find((s) => s.state === "run");
      if (cur) set(cur.id, "fail", translateError(e));
    }
  })();

  async function createFiche() {
    busy = true;
    try {
      const chosen = tags.filter((x) => x.on).map((x) => x.name);
      if (chosen.length) await api.addTags(path, chosen);
      const r = await api.createFicheWithLinks(path, links.filter((x) => x.on).map((x) => x.path));
      await app.refresh();
      app.fiches = await api.fichesMap();
      app.flowFor = null;
      await app.openNote(r.path, { newTab: true });
      app.toast(t("Fiche prête : remplis-la, ou clique « Méthode Eloria » pour que l'IA propose les 13 sections"), "success");
    } catch (e) {
      app.toast(translateError(e), "error");
    } finally {
      busy = false;
    }
  }

  async function openSource() {
    const chosen = tags.filter((x) => x.on).map((x) => x.name);
    if (chosen.length) await api.addTags(path, chosen).catch(() => {});
    app.flowFor = null;
    await app.refresh();
    await app.openNote(path, { newTab: true });
  }

  const ICON: Record<Step["state"], string> = { wait: "○", run: "…", done: "✓", skip: "–", fail: "!" };
</script>

<Modal title={t("Préparer : {title}", { title: req.title })} width={640} onclose={() => (app.flowFor = null)}>
  <div class="body">
    <ol class="steps">
      {#each steps as s (s.id)}
        <li class={s.state}>
          <span class="ic">{ICON[s.state]}</span>
          <span class="lbl">{s.label}</span>
          {#if s.detail}<span class="det" class:alerte={s.id === "reliab" && reliab?.level === "alerte"} class:attention={s.id === "reliab" && reliab?.level === "attention"}>{s.detail}</span>{/if}
        </li>
      {/each}
    </ol>
    {#if ready}
      <h3>{t("Tags proposés")}</h3>
      {#if tags.length}
        <div class="chips">
          {#each tags as tg (tg.name)}
            <label class="chip" class:on={tg.on}><input type="checkbox" bind:checked={tg.on} />#{tg.name}</label>
          {/each}
        </div>
      {:else}
        <p class="faint">{t("Aucun de tes tags ne correspond à ce document.")}</p>
      {/if}
      <h3>{t("Notes proches, à relier dans la fiche")}</h3>
      {#if links.length}
        <ul class="links">
          {#each links as l (l.path)}
            <li><label><input type="checkbox" bind:checked={l.on} /> <span class="lt">{l.title}</span> <span class="faint">{l.path}</span></label></li>
          {/each}
        </ul>
      {:else}
        <p class="faint">{t("Aucune note proche pour l'instant.")}</p>
      {/if}
      {#if fiches.length}
        <p class="faint">{tn(fiches.length, "Cette source a déjà {n} fiche ; une nouvelle s'ajoutera.", "Cette source a déjà {n} fiches ; une nouvelle s'ajoutera.")}</p>
      {/if}
    {/if}
  </div>
  {#snippet footer()}
    <button class="btn" disabled={!ready} onclick={openSource}>{t("Ouvrir la source seulement")}</button>
    <button class="btn primary" disabled={!ready || busy} onclick={createFiche}>{t("Créer la fiche")}</button>
  {/snippet}
</Modal>

<style>
  .body {
    padding: 14px 22px;
    max-height: 66vh;
    overflow: auto;
  }
  .steps {
    margin: 0 0 6px;
    padding: 0;
    list-style: none;
  }
  .steps li {
    display: grid;
    grid-template-columns: 18px auto 1fr;
    gap: 2px 8px;
    align-items: baseline;
    padding: 4px 0;
    font-size: 13px;
  }
  .ic {
    font-weight: 700;
    text-align: center;
    color: var(--text-faint);
  }
  .done .ic {
    color: var(--success);
  }
  .fail .ic,
  .det.alerte {
    color: var(--danger);
  }
  .run .ic {
    color: var(--accent-text);
  }
  .det.attention {
    color: var(--warning);
  }
  .lbl {
    font-weight: 600;
  }
  .wait .lbl,
  .skip .lbl {
    color: var(--text-soft);
    font-weight: 500;
  }
  .det {
    color: var(--text-soft);
    min-width: 0;
    overflow-wrap: anywhere;
  }
  h3 {
    margin: 14px 0 6px;
    font-size: 13.5px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 10px;
    border: 1px solid var(--line);
    border-radius: 999px;
    font-size: 12.5px;
    color: var(--text-soft);
    cursor: pointer;
  }
  .chip input {
    display: none;
  }
  .chip.on {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--text);
  }
  .links {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .links li {
    padding: 3px 0;
    font-size: 13px;
  }
  .links label {
    display: flex;
    align-items: baseline;
    gap: 6px;
    cursor: pointer;
    min-width: 0;
  }
  .lt {
    font-weight: 500;
  }
  .faint {
    color: var(--text-faint);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  p.faint {
    white-space: normal;
  }
</style>
