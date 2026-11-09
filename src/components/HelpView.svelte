<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { i18n, t } from "../lib/i18n.svelte";
  import { commands, keysOf, labelOf, prettyKeys } from "../lib/commands";
  import { helpSections, inline, sectionText } from "../lib/help";
  import Icon from "./Icon.svelte";

  // Help page: getting started and every feature, in French or English according to the
  // interface language. Shortcuts come from the command list.
  let query = $state("");
  let current = $state("bienvenue");
  let body = $state<HTMLDivElement>();
  const sections = $derived(helpSections(i18n.lang));
  const fold = (s: string) => s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
  const shown = $derived(query.trim() ? sections.filter((s) => fold(sectionText(s)).includes(fold(query.trim()))) : sections);
  const shortcuts = $derived(
    commands
      .map((c) => ({ label: labelOf(c), group: t(c.group), keys: keysOf(c) }))
      .filter((c) => c.keys)
      .sort((a, b) => a.group.localeCompare(b.group) || a.label.localeCompare(b.label)),
  );
  function go(id: string) {
    current = id;
    body?.querySelector(`#aide-${id}`)?.scrollIntoView({ behavior: "smooth", block: "start" });
  }
  function onScroll() {
    if (!body) return;
    const top = body.getBoundingClientRect().top;
    let id = current;
    for (const el of body.querySelectorAll<HTMLElement>("section[data-id]")) {
      if (el.getBoundingClientRect().top - top < 80) id = el.dataset.id!;
    }
    current = id;
  }
</script>

<div class="help">
  <nav class="toc" aria-label={t("Sommaire de l'aide")}>
    <div class="search">
      <Icon name="search" size={14} />
      <input placeholder={t("Chercher dans l'aide")} aria-label={t("Chercher dans l'aide")} bind:value={query} />
    </div>
    {#each shown as s (s.id)}
      <button class:on={current === s.id} onclick={() => go(s.id)}>{s.title}</button>
    {:else}
      <p class="faint">{t("Aucun résultat.")}</p>
    {/each}
  </nav>
  <div class="body" bind:this={body} onscroll={onScroll}>
    <h1>{t("Aide d'Eloria")}</h1>
    {#each shown as s (s.id)}
      <section id="aide-{s.id}" data-id={s.id}>
        <h2>{s.title}</h2>
        {#each s.blocks as b, i (i)}
          {#if "p" in b}
            <p>{@html inline(b.p)}</p>
          {:else if "ul" in b}
            <ul>{#each b.ul as li, j (j)}<li>{@html inline(li)}</li>{/each}</ul>
          {:else if "views" in b}
            <div class="views">
              {#each b.views as [label, kind] (kind)}
                <button class="btn small" onclick={() => app.openView(kind)}>{label}</button>
              {/each}
            </div>
          {:else if "shortcuts" in b}
            <table>
              <tbody>
                {#each shortcuts as c (c.label + c.keys)}
                  <tr><td>{c.label}</td><td class="g">{c.group}</td><td class="k"><kbd>{prettyKeys(c.keys)}</kbd></td></tr>
                {/each}
              </tbody>
            </table>
          {/if}
        {/each}
      </section>
    {/each}
  </div>
</div>

<style>
  .help {
    height: 100%;
    display: grid;
    grid-template-columns: minmax(180px, 230px) minmax(0, 1fr);
    min-height: 0;
    container-type: inline-size;
  }
  .toc {
    border-right: 1px solid var(--line);
    padding: 14px 10px;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    margin-bottom: 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg-raised);
    color: var(--text-faint);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: none;
    font: 13px var(--font-ui);
    color: var(--text);
    outline: none;
  }
  .toc button {
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    padding: 5px 8px;
    text-align: left;
    font: 13px var(--font-ui);
    color: var(--text-soft);
    cursor: pointer;
  }
  .toc button:hover {
    background: var(--bg-soft);
    color: var(--text);
  }
  .toc button.on {
    background: var(--accent-soft);
    color: var(--text);
    font-weight: 600;
  }
  .body {
    overflow: auto;
    padding: 22px 32px 60px;
    min-width: 0;
  }
  h1 {
    margin: 0 0 6px;
    font-size: 24px;
  }
  section {
    max-width: 760px;
    padding-top: 8px;
  }
  h2 {
    margin: 22px 0 8px;
    font-size: 17px;
  }
  p,
  li {
    font-size: 14px;
    line-height: 1.6;
    color: var(--text);
  }
  p {
    margin: 0 0 10px;
  }
  ul {
    margin: 0 0 10px;
    padding-left: 20px;
  }
  li {
    margin: 3px 0;
  }
  :global(.help code) {
    font: 12.5px var(--font-mono);
    background: var(--code-bg);
    border-radius: var(--radius-s);
    padding: 1px 5px;
  }
  .views {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 4px 0 10px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }
  td {
    padding: 4px 8px 4px 0;
    border-bottom: 1px solid var(--line);
  }
  td.g {
    color: var(--text-faint);
    font-size: 12px;
  }
  td.k {
    text-align: right;
    white-space: nowrap;
  }
  kbd {
    font: 12px var(--font-mono);
    padding: 1px 6px;
    border: 1px solid var(--line-strong);
    border-bottom-width: 2px;
    border-radius: var(--radius-s);
    background: var(--bg-raised);
  }
  .faint {
    color: var(--text-faint);
    font-size: 12px;
  }
  @container (max-width: 640px) {
    .help {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: auto minmax(0, 1fr);
    }
    .toc {
      border-right: 0;
      border-bottom: 1px solid var(--line);
      flex-direction: row;
      flex-wrap: wrap;
      max-height: 120px;
    }
    .body {
      padding: 16px 18px 40px;
    }
  }
</style>
