<script lang="ts">
  import { t } from "../lib/i18n.svelte";
  import { app } from "../lib/state.svelte";
  import type { Pane, Tab } from "../lib/types";
  import Icon from "./Icon.svelte";
  import NoteEditor from "./NoteEditor.svelte";
  import GraphView from "./GraphView.svelte";
  import KanbanView from "./KanbanView.svelte";
  import TableView from "./TableView.svelte";
  import TimelineView from "./TimelineView.svelte";
  import MatrixView from "./MatrixView.svelte";
  import ReviewView from "./ReviewView.svelte";
  import LatexView from "./LatexView.svelte";
  import DashboardView from "./DashboardView.svelte";
  import ThesisView from "./ThesisView.svelte";
  import MethodsView from "./MethodsView.svelte";
  import ArgumentView from "./ArgumentView.svelte";
  import StatsView from "./StatsView.svelte";
  import ConceptsView from "./ConceptsView.svelte";
  import TopicsView from "./TopicsView.svelte";
  import EntitiesView from "./EntitiesView.svelte";
  import TaxonomyView from "./TaxonomyView.svelte";
  import KnowledgeMapView from "./KnowledgeMapView.svelte";
  import HelpView from "./HelpView.svelte";
  import ConsoleView from "./ConsoleView.svelte";
  import ExchangeView from "./ExchangeView.svelte";
  import CitationGraphView from "./CitationGraphView.svelte";
  import ZoteroItemView from "./ZoteroItemView.svelte";
  import ImageView from "./ImageView.svelte";
  import PdfView from "./PdfView.svelte";

  let { pane, index }: { pane: Pane; index: number } = $props();
  let dropAt = $state<number | null>(null);

  const isActivePane = $derived(app.activePane === index);

  const icons: Record<string, string> = {
    note: "note",
    graph: "graph",
    kanban: "kanban",
    table: "table",
    timeline: "timeline",
    matrix: "table",
    review: "list",
    latex: "code",
    dashboard: "pulse",
    these: "flag",
    methodes: "flask",
    argument: "graph",
    statistiques: "chart",
    concepts: "bulb",
    themes: "clusters",
    entites: "person",
    taxonomie: "tree",
    atlas: "map",
    aide: "help",
    console: "code",
    echanges: "download",
    citations: "graph",
    "zotero-item": "book",
    pdf: "pdf",
    image: "image",
  };

  function tabMenu(e: MouseEvent, tb: Tab) {
    app.openMenu(e, [
      { label: t("Fermer"), icon: "x", run: () => app.closeTab(index, tb.id) },
      {
        label: t("Fermer les autres onglets"),
        run: async () => {
          for (const o of [...pane.tabs]) if (o.id !== tb.id) await app.closeTab(index, o.id);
        },
      },
      ...(tb.kind === "note" && tb.path
        ? [
            { label: t("Ouvrir à côté"), icon: "split", run: () => app.openNote(tb.path!, { otherPane: true }) },
            { label: t("Historique des versions"), icon: "history", run: () => (app.historyFor = tb.path!) },
            { label: t("Exporter…"), icon: "download", run: () => (app.exportFor = tb.path!) },
          ]
        : []),
    ]);
  }

  function onDrop(e: DragEvent, at: number) {
    e.preventDefault();
    dropAt = null;
    const raw = e.dataTransfer?.getData("text/x-eloria-tab");
    if (raw) {
      const { pane: from, id } = JSON.parse(raw);
      app.moveTab(from, id, index, at);
      return;
    }
    const path = e.dataTransfer?.getData("text/x-eloria-path");
    if (path) {
      app.activePane = index;
      app.openNote(path, { newTab: true });
    }
  }

  // The active tab stays visible in the bar (after opening or switching).
  let strip = $state<HTMLDivElement>();
  $effect(() => {
    const id = pane.active;
    if (!strip || !id) return;
    requestAnimationFrame(() => strip?.querySelector<HTMLElement>(".tab.current")?.scrollIntoView({ block: "nearest", inline: "nearest" }));
  });
</script>

<section class="pane" class:active={isActivePane} onpointerdown={() => (app.activePane = index)} aria-label={t("Panneau {n}", { n: index + 1 })}>
  <header
    class="tabs"
    role="tablist"
    tabindex="-1"
    ondragover={(e) => {
      e.preventDefault();
      dropAt = pane.tabs.length;
    }}
    ondragleave={() => (dropAt = null)}
    ondrop={(e) => onDrop(e, pane.tabs.length)}
  >
    {#if index === 0 && !app.leftOpen && !app.focusMode}
      <button class="icon-btn" title={t("Afficher le panneau")} aria-label={t("Afficher le panneau")} onclick={() => (app.leftOpen = true)}><Icon name="panel-left" /></button>
    {/if}
    <div
      class="strip"
      bind:this={strip}
      onwheel={(e) => {
        if (strip && Math.abs(e.deltaY) > Math.abs(e.deltaX)) strip.scrollLeft += e.deltaY;
      }}
    >
      {#each pane.tabs as tb, i (tb.id)}
        <div
          class="tab"
          class:current={tb.id === pane.active}
          class:drop={dropAt === i}
          role="tab"
          tabindex="0"
          aria-selected={tb.id === pane.active}
          draggable="true"
          title={tb.path ?? tb.title}
          ondragstart={(e) => e.dataTransfer?.setData("text/x-eloria-tab", JSON.stringify({ pane: index, id: tb.id }))}
          ondragover={(e) => {
            e.preventDefault();
            e.stopPropagation();
            dropAt = i;
          }}
          ondrop={(e) => {
            e.stopPropagation();
            onDrop(e, i);
          }}
          onclick={() => app.activate(index, tb.id)}
          onkeydown={(e) => e.key === "Enter" && app.activate(index, tb.id)}
          onauxclick={(e) => e.button === 1 && app.closeTab(index, tb.id)}
          oncontextmenu={(e) => tabMenu(e, tb)}
        >
          <Icon name={icons[tb.kind] ?? "note"} size={14} />
          <span class="label">{tb.title}</span>
          {#if tb.dirty}<span class="dirty" title={t("Modifications non enregistrées")}></span>{/if}
          <button
            class="close"
            aria-label={t("Fermer {title}", { title: tb.title })}
            onclick={(e) => {
              e.stopPropagation();
              app.closeTab(index, tb.id);
            }}><Icon name="x" size={13} /></button
          >
        </div>
      {/each}
      <button class="icon-btn new" title={t("Nouvelle note")} aria-label={t("Nouvelle note")} onclick={() => app.newNote()}><Icon name="plus" size={16} /></button>
    </div>
    <div class="tools">
      {#if index === app.panes.length - 1}
        <button class="icon-btn" class:active={app.panes.length > 1} title={t("Diviser l'écran (Ctrl+\\)")} aria-label={t("Diviser")} onclick={() => app.toggleSplit()}><Icon name="split" /></button>
        {#if !app.focusMode}
          <button class="icon-btn" class:active={app.rightOpen && app.roomForRight} title={t("Panneau droit")} aria-label={t("Panneau droit")} onclick={() => app.toggleRight()}><Icon name="panel-right" /></button>
        {/if}
      {/if}
    </div>
  </header>

  <div class="content">
    {#each pane.tabs as tb (tb.id)}
      <div class="view" class:shown={tb.id === pane.active}>
        {#if tb.kind === "note" && tb.path}
          <NoteEditor tab={tb} paneIndex={index} visible={tb.id === pane.active} />
        {:else if tb.kind === "graph"}
          <GraphView visible={tb.id === pane.active} />
        {:else if tb.kind === "kanban"}
          <KanbanView />
        {:else if tb.kind === "table"}
          <TableView viewName={tb.key} />
        {:else if tb.kind === "timeline"}
          <TimelineView />
        {:else if tb.kind === "matrix"}
          <MatrixView visible={tb.id === pane.active} />
        {:else if tb.kind === "citations"}
          <CitationGraphView visible={tb.id === pane.active} />
        {:else if tb.kind === "echanges"}
          <ExchangeView visible={tb.id === pane.active} />
        {:else if tb.kind === "aide"}
          <HelpView />
        {:else if tb.kind === "console"}
          <ConsoleView visible={tb.id === pane.active} />
        {:else if tb.kind === "atlas"}
          <KnowledgeMapView visible={tb.id === pane.active} />
        {:else if tb.kind === "taxonomie"}
          <TaxonomyView visible={tb.id === pane.active} />
        {:else if tb.kind === "entites"}
          <EntitiesView visible={tb.id === pane.active} />
        {:else if tb.kind === "themes"}
          <TopicsView visible={tb.id === pane.active} />
        {:else if tb.kind === "concepts"}
          <ConceptsView visible={tb.id === pane.active} />
        {:else if tb.kind === "statistiques"}
          <StatsView visible={tb.id === pane.active} />
        {:else if tb.kind === "argument" && tb.path}
          <ArgumentView path={tb.path} visible={tb.id === pane.active} />
        {:else if tb.kind === "methodes"}
          <MethodsView visible={tb.id === pane.active} />
        {:else if tb.kind === "these"}
          <ThesisView visible={tb.id === pane.active} />
        {:else if tb.kind === "dashboard"}
          <DashboardView visible={tb.id === pane.active} />
        {:else if tb.kind === "latex" && tb.path}
          <LatexView path={tb.path} visible={tb.id === pane.active} />
        {:else if tb.kind === "review" && tb.path}
          <ReviewView path={tb.path} visible={tb.id === pane.active} />
        {:else if tb.kind === "zotero-item" && tb.key}
          <ZoteroItemView itemKey={tb.key} />
        {:else if tb.kind === "pdf" && tb.path}
          <PdfView path={tb.path} page={tb.page} />
        {:else if tb.kind === "image" && tb.path}
          <ImageView path={tb.path} />
        {/if}
      </div>
    {:else}
      <div class="blank">
        <p>{t("Aucune note ouverte")}</p>
        <div class="shortcuts">
          <button class="btn ghost" onclick={() => (app.palette = "files")}>{t("Ouvrir une note")} <kbd>Ctrl O</kbd></button>
          <button class="btn ghost" onclick={() => app.newNote()}>{t("Nouvelle note")} <kbd>Ctrl N</kbd></button>
          <button class="btn ghost" onclick={() => (app.palette = "commands")}>{t("Toutes les commandes")} <kbd>Ctrl P</kbd></button>
        </div>
      </div>
    {/each}
  </div>
</section>

<style>
  .pane {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
  .tabs {
    display: flex;
    align-items: stretch;
    height: 40px;
    flex-shrink: 0;
    background: var(--bg-soft);
    border-bottom: 1px solid var(--line);
    padding: 0 6px;
    gap: 4px;
  }
  .tabs > .icon-btn {
    align-self: center;
  }
  .strip {
    flex: 1;
    display: flex;
    align-items: flex-end;
    gap: 2px;
    overflow-x: auto;
    min-width: 0;
    scrollbar-width: none;
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 32px;
    flex: 0 1 200px;
    max-width: 220px;
    min-width: 64px;
    padding: 0 6px 0 10px;
    border-radius: 7px 7px 0 0;
    color: var(--text-soft);
    cursor: default;
    position: relative;
  }
  .tab:hover {
    background: var(--bg-sunk);
  }
  .tab.current {
    background: var(--bg);
    color: var(--text);
    box-shadow: 0 1px 0 var(--bg), 1px 0 0 var(--line), -1px 0 0 var(--line), 0 -1px 0 var(--line);
  }
  .pane.active .tab.current::before {
    content: "";
    position: absolute;
    left: 8px;
    right: 8px;
    top: 0;
    height: 2px;
    border-radius: 0 0 2px 2px;
    background: var(--accent);
  }
  .tab.drop {
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dirty {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
  }
  .close {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--text-faint);
    opacity: 0;
  }
  .tab:hover .close,
  .tab.current .close {
    opacity: 1;
  }
  .close:hover {
    background: var(--bg-sunk);
    color: var(--text);
  }
  .new {
    align-self: center;
    margin-bottom: 0;
  }
  .tools {
    display: flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
  }
  .new {
    flex-shrink: 0;
  }
  .content {
    flex: 1;
    position: relative;
    min-height: 0;
  }
  .view {
    position: absolute;
    inset: 0;
    display: none;
  }
  .view.shown {
    display: flex;
    flex-direction: column;
  }
  .blank {
    height: 100%;
    display: grid;
    place-content: center;
    text-align: center;
    color: var(--text-faint);
  }
  .blank p {
    font-family: var(--font-editor);
    font-size: 18px;
    margin: 0 0 14px;
  }
  .shortcuts {
    display: flex;
    flex-direction: column;
    gap: 2px;
    align-items: stretch;
  }
  .shortcuts .btn {
    justify-content: space-between;
    gap: 30px;
    color: var(--text-soft);
  }
</style>
