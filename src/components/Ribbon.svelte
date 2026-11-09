<script lang="ts">
  import { t, tk } from "../lib/i18n.svelte";
  import { app, type LeftPanel } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";

  // Icon bar: one icon per function, no duplicates. Left panels, then views,
  // then tools. Everything else is in the palette (Ctrl+P).
  const panels: { id: LeftPanel; icon: string; label: string }[] = [
    { id: "files", icon: "files", label: tk("Fichiers") },
    { id: "search", icon: "search", label: tk("Recherche") },
    { id: "tags", icon: "tag", label: tk("Tags") },
    { id: "zotero", icon: "book", label: tk("Zotero") },
    { id: "these", icon: "thesis", label: tk("Thèse") },
    { id: "inbox", icon: "inbox", label: tk("Boîte de réception") },
  ];

  const views: { icon: string; label: string; run: () => void }[] = [
    { icon: "pulse", label: tk("Tableau de bord"), run: () => app.openView("dashboard") },
    { icon: "graph", label: tk("Graphe (Ctrl+G)"), run: () => app.openView("graph") },
    { icon: "kanban", label: tk("Kanban des fiches"), run: () => app.openView("kanban") },
    { icon: "table", label: tk("Base de notes"), run: () => app.openView("table") },
    { icon: "list", label: tk("Matrice de recherche"), run: () => app.openView("matrix") },
    { icon: "timeline", label: tk("Frise des lectures"), run: () => app.openView("timeline") },
    { icon: "calendar", label: tk("Note du jour (Ctrl+Maj+D)"), run: () => app.openDaily() },
    { icon: "flag", label: tk("Espace thèse"), run: () => app.openView("these") },
    { icon: "bulb", label: tk("Concepts"), run: () => app.openView("concepts") },
    { icon: "clusters", label: tk("Thèmes"), run: () => app.openView("themes") },
    { icon: "person", label: tk("Entités"), run: () => app.openView("entites") },
    { icon: "tree", label: tk("Taxonomie"), run: () => app.openView("taxonomie") },
    { icon: "flask", label: tk("Méthodologies"), run: () => app.openView("methodes") },
    { icon: "map", label: tk("Carte des connaissances"), run: () => app.openView("atlas") },
    { icon: "chart", label: tk("Statistiques de recherche"), run: () => app.openView("statistiques") },
  ];

  const tools: { icon: string; label: string; run: () => void }[] = [
    { icon: "quote", label: tk("Ask Eloria (Ctrl+Maj+A)"), run: () => app.toggleAsk() },
    { icon: "plus", label: tk("Ajouter une référence (DOI, titre)"), run: () => (app.addRefOpen = true) },
    { icon: "code", label: tk("Nouveau document LaTeX…"), run: () => app.newLatex() },
    { icon: "check", label: tk("Vérifier le coffre"), run: () => (app.integrityOpen = true) },
  ];

  // Fade at the bottom when icons are hidden below the fold.
  let scroller = $state<HTMLDivElement>();
  let more = $state(false);
  function measure() {
    if (scroller) more = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight > 2;
  }
  $effect(() => {
    if (!scroller) return;
    const ro = new ResizeObserver(measure);
    ro.observe(scroller);
    return () => ro.disconnect();
  });

  function pick(id: LeftPanel) {
    if (app.leftPanel === id && app.leftOpen) app.leftOpen = false;
    else {
      app.leftPanel = id;
      app.leftOpen = true;
    }
  }
</script>

<nav class="ribbon" aria-label={t("Barre latérale")}>
  <!-- Upper part: scrolls if the window is short; help, palette and settings stay visible. -->
  <div class="scroll" class:more bind:this={scroller} onscroll={measure}>
  {#each panels as p (p.id)}
    <button class="icon-btn" class:active={app.leftOpen && app.leftPanel === p.id} title={t(p.label)} aria-label={t(p.label)} onclick={() => pick(p.id)}>
      <Icon name={p.icon} size={19} />
    </button>
  {/each}
  <span class="sep"></span>
  {#each views as v (v.label)}
    <button class="icon-btn" title={t(v.label)} aria-label={t(v.label)} onclick={v.run}><Icon name={v.icon} size={19} /></button>
  {/each}
  <span class="sep"></span>
  {#each tools as x (x.label)}
    <button class="icon-btn" title={t(x.label)} aria-label={t(x.label)} onclick={x.run}><Icon name={x.icon} size={19} /></button>
  {/each}
  </div>
  <span class="sep bottom"></span>
  <button class="icon-btn" title={t("Aide (F1)")} aria-label={t("Aide")} onclick={() => app.openView("aide")}><Icon name="help" size={19} /></button>
  <button class="icon-btn" title={t("Palette de commandes (Ctrl+P)")} aria-label={t("Commandes")} onclick={() => (app.palette = "commands")}><Icon name="command" size={18} /></button>
  <button class="icon-btn" title={t("Réglages (Ctrl+,)")} aria-label={t("Réglages")} onclick={() => (app.settingsOpen = true)}><Icon name="settings" size={19} /></button>
</nav>

<style>
  .ribbon {
    width: 46px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 10px 0;
    background: var(--bg-soft);
    border-right: 1px solid var(--line);
    overflow: hidden;
  }
  .scroll {
    flex: 1;
    min-height: 0;
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    overflow-y: auto;
    scrollbar-width: none;
  }
  .scroll.more {
    mask-image: linear-gradient(to bottom, #000 calc(100% - 22px), transparent);
  }
  /* Short window: icons slightly tighter before scrolling. */
  @media (max-height: 900px) {
    .ribbon :global(.icon-btn) {
      height: 30px !important;
    }
    .sep {
      margin: 3px 0 !important;
    }
  }
  .ribbon :global(.icon-btn) {
    width: 34px;
    height: 34px;
    flex-shrink: 0;
  }
  .sep {
    width: 18px;
    height: 1px;
    background: var(--line-strong);
    margin: 6px 0;
    flex-shrink: 0;
  }
  .sep.bottom {
    margin-top: 4px;
  }
</style>
