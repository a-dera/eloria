<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, tn, translateError } from "../lib/i18n.svelte";
  import type { IntegrityReport, Issue } from "../lib/types";
  import Modal from "./Modal.svelte";

  // Integrity check: changes nothing, suggests fixes one by one.
  let report = $state<IntegrityReport | null>(null);
  let error = $state("");
  let busy = $state(false);

  async function run() {
    busy = true;
    error = "";
    try {
      report = await api.checkIntegrity();
    } catch (e) {
      error = translateError(e);
    } finally {
      busy = false;
    }
  }
  run();

  async function open(i: Issue) {
    app.integrityOpen = false;
    await app.openNote(i.path, { newTab: true, line: i.line > 0 ? i.line : undefined });
  }

  async function createTarget(i: Issue) {
    await app.followLink(i.detail, i.path, { newTab: true });
    run();
  }

  const groups = $derived(
    report
      ? [
          { key: "broken", title: t("Liens vers des notes absentes"), hint: t("Crée la note, ou corrige le lien."), items: report.broken_links, create: true },
          { key: "files", title: t("Fichiers introuvables"), hint: t("Images, PDF et pièces jointes déplacés ou supprimés."), items: report.missing_files, create: false },
          { key: "yaml", title: t("Propriétés illisibles"), hint: t("Le bloc YAML en tête de note contient une erreur : la note est ignorée par les vues."), items: report.bad_frontmatter, create: false },
          { key: "fiches", title: t("Fiches sans source"), hint: t("La source jointe a été renommée ou supprimée : corrige la propriété source."), items: report.fiches_without_source, create: false },
          { key: "orphans", title: t("Sources absentes de Zotero"), hint: t("Supprimées de Zotero : le coffre les garde. Rien à faire si c'est voulu."), items: report.orphan_sources, create: false },
        ]
      : [],
  );
</script>

<Modal title={t("Vérification du coffre")} width={760} onclose={() => (app.integrityOpen = false)}>
  <div class="body">
    {#if busy && !report}
      <p>{t("Vérification…")}</p>
    {:else if error}
      <p class="error">{error}</p>
    {:else if report}
      <p class="summary">
        {#if report.broken_links.length + report.missing_files.length + report.bad_frontmatter.length + report.fiches_without_source.length + report.orphan_sources.length === 0}
          <b class="ok">{t("Tout est en ordre.")}</b>
        {/if}
        {tn(report.notes, "{n} note vérifiée.", "{n} notes vérifiées.")}
      </p>
      {#each groups.filter((g) => g.items.length) as g (g.key)}
        <section>
          <h3>{g.title} <span class="count">{g.items.length}</span></h3>
          <p class="hint">{g.hint}</p>
          <ul>
            {#each g.items.slice(0, 200) as i (i.path + i.line + i.detail)}
              <li>
                <button class="link" onclick={() => open(i)}>{i.path}{i.line > 0 ? ` : ${i.line}` : ""}</button>
                <span class="detail">{i.detail}</span>
                {#if g.create}<button class="btn small ghost" onclick={() => createTarget(i)}>{t("Créer la note")}</button>{/if}
              </li>
            {/each}
          </ul>
        </section>
      {/each}
    {/if}
  </div>
  {#snippet footer()}
    <button class="btn" disabled={busy} onclick={run}>{t("Vérifier à nouveau")}</button>
    <button class="btn" onclick={() => (app.integrityOpen = false)}>{t("Fermer")}</button>
  {/snippet}
</Modal>

<style>
  .body {
    padding: 16px 22px;
    max-height: 65vh;
    overflow: auto;
  }
  .summary {
    margin: 0 0 8px;
  }
  .ok {
    color: var(--success, var(--accent-text));
    margin-right: 6px;
  }
  .error {
    color: var(--danger);
  }
  h3 {
    margin: 16px 0 2px;
    font-size: 14px;
  }
  .count {
    color: var(--text-faint);
    font-weight: 400;
  }
  .hint {
    margin: 0 0 6px;
    font-size: 12.5px;
    color: var(--text-soft);
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 3px 0;
    font-size: 13px;
  }
  .link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent-text);
    font: inherit;
    cursor: pointer;
    text-align: left;
  }
  .link:hover {
    text-decoration: underline;
  }
  .detail {
    flex: 1;
    color: var(--text-soft);
    font-family: var(--font-mono);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
