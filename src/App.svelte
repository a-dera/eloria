<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { app } from "./lib/state.svelte";
  import { handleGlobalKey } from "./lib/commands";
  import { syncCustomCommands } from "./lib/custom";
  import { loadExtensions } from "./lib/extensions.svelte";
  import { initKeyboardLayout } from "./lib/util";
  import { onCloseRequested } from "./lib/api";
  import Welcome from "./components/Welcome.svelte";
  import Ribbon from "./components/Ribbon.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import Workspace from "./components/Workspace.svelte";
  import RightPanel from "./components/RightPanel.svelte";
  import StatusBar from "./components/StatusBar.svelte";
  import Palette from "./components/Palette.svelte";
  import SettingsModal from "./components/SettingsModal.svelte";
  import HistoryModal from "./components/HistoryModal.svelte";
  import ExportModal from "./components/ExportModal.svelte";
  import ImportLibraryModal from "./components/ImportLibraryModal.svelte";
  import AddReferenceModal from "./components/AddReferenceModal.svelte";
  import IntegrityModal from "./components/IntegrityModal.svelte";
  import RewriteModal from "./components/RewriteModal.svelte";
  import SuggestModal from "./components/SuggestModal.svelte";
  import CheckModal from "./components/CheckModal.svelte";
  import FlowModal from "./components/FlowModal.svelte";
  import OutlineModal from "./components/OutlineModal.svelte";
  import ScienceModal from "./components/ScienceModal.svelte";
  import LatexToolsModal from "./components/LatexToolsModal.svelte";
  import CompareModal from "./components/CompareModal.svelte";
  import AiFicheModal from "./components/AiFicheModal.svelte";
  import Dialogs from "./components/Dialogs.svelte";
  import Resizer from "./components/Resizer.svelte";
  import { checkBuild, type BuildProblem } from "./lib/buildcheck";
  import { t } from "./lib/i18n.svelte";
  import { isTauri } from "./lib/api";

  let ready = $state(false);
  // Narrow window: the note keeps at least 480 px. The left panel shrinks,
  // the right one hides itself (the setting is kept for a wide window).
  const leftShown = $derived(app.leftShown);
  const roomForRight = $derived(app.roomForRight);
  /** Rust core older than the interface (failed build): reported, never ignored. */
  let buildProblem = $state<BuildProblem | null>(null);
  // Custom commands and saved views: present in the palette and shortcuts.
  $effect(() => syncCustomCommands(app.settings));
  // Approved extensions of the open vault: reloaded on every vault change.
  $effect(() => {
    void app.vault?.root;
    untrack(() => loadExtensions());
  });

  onMount(() => {
    void initKeyboardLayout();
    app.init().finally(() => (ready = true));
    if (isTauri)
      checkBuild()
        .then((r) => {
          app.buildInfo = r.info;
          buildProblem = r.problem;
          if (r.problem) console.error("Cœur Rust périmé, commandes absentes :", r.problem.missing);
        })
        .catch(() => {});
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && app.focusMode && !app.palette && !app.settingsOpen) {
        app.focusMode = false;
        return;
      }
      handleGlobalKey(e);
    };
    const onUnload = () => app.saveAll();
    // Saves the last keystroke before the window closes.
    let unlistenClose: (() => void) | null = null;
    onCloseRequested(() => app.saveAll()).then((u) => (unlistenClose = u));
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("beforeunload", onUnload);
    // Disables the native context menu outside text areas
    const onCtx = (e: MouseEvent) => {
      const t = e.target as HTMLElement;
      if (!t.closest(".cm-content, input, textarea")) e.preventDefault();
    };
    window.addEventListener("contextmenu", onCtx);
    const onStale = (e: Event) => {
      const cmd = (e as CustomEvent<string>).detail;
      const missing = new Set([...(buildProblem?.missing ?? []), cmd]);
      buildProblem = { missing: [...missing], info: buildProblem?.info ?? app.buildInfo };
    };
    window.addEventListener("eloria:stale-core", onStale);
    const onAssist = () => app.openRewrite("rephrase");
    window.addEventListener("eloria:assist", onAssist);
    return () => {
      window.removeEventListener("eloria:stale-core", onStale);
      window.removeEventListener("eloria:assist", onAssist);
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("beforeunload", onUnload);
      window.removeEventListener("contextmenu", onCtx);
      unlistenClose?.();
    };
  });
</script>

<svelte:window onresize={() => (app.winW = window.innerWidth)} />

{#if buildProblem}
  <div class="build-alert" role="alert">
    <b>{t("Le cœur de l'application est plus ancien que l'interface.")}</b>
    {t("Une compilation Rust a échoué et l'ancien binaire tourne encore : certains écrans seront vides. Ferme Eloria et relance « Lancer Eloria.cmd », qui compile d'abord et s'arrête en cas d'erreur.")}
    <span class="mono">{t("Commandes absentes : {list}", { list: buildProblem.missing.join(", ") })}{buildProblem.info ? ` · ${buildProblem.info.commit} · ${buildProblem.info.built}` : ""}</span>
  </div>
{/if}
{#if ready}
  {#if !app.vault}
    <Welcome />
  {:else}
    <div class="app" class:focus={app.focusMode}>
      <div class="main">
        {#if !app.focusMode}
          <Ribbon />
          {#if app.leftOpen}
            <div class="left" style:width="{leftShown}px"><Sidebar /></div>
            <Resizer value={app.leftWidth} min={200} max={560} defaultValue={280} onchange={(v) => (app.leftWidth = v)} oncollapse={() => (app.leftOpen = false)} collapseBelow={130} onend={() => app.saveLayout()} label={t("Largeur du panneau de gauche")} />
          {/if}
        {/if}
        <div class="center"><Workspace /></div>
        {#if !app.focusMode && app.rightOpen && (app.contextNotePath || app.rightPanel === "ask") && roomForRight}
          <Resizer value={app.rightWidth} min={220} max={600} defaultValue={300} invert onchange={(v) => (app.rightWidth = v)} oncollapse={() => (app.rightOpen = false)} collapseBelow={150} onend={() => app.saveLayout()} label={t("Largeur du panneau de droite")} />
          <div class="right" style:width="{app.rightWidth}px"><RightPanel /></div>
        {/if}
      </div>
      {#if !app.focusMode}<StatusBar />{/if}
    </div>
  {/if}
  <Palette />
  {#if app.settingsOpen}<SettingsModal />{/if}
  {#if app.historyFor}<HistoryModal />{/if}
  {#if app.exportFor}<ExportModal />{/if}
  {#if app.importOpen}<ImportLibraryModal />{/if}
  {#if app.addRefOpen}<AddReferenceModal />{/if}
  {#if app.integrityOpen}<IntegrityModal />{/if}
  {#if app.rewriteFor}<RewriteModal />{/if}
  {#if app.suggestFor}<SuggestModal />{/if}
  {#if app.checkFor}<CheckModal />{/if}
  {#if app.flowFor}<FlowModal />{/if}
  {#if app.outlineFor}<OutlineModal />{/if}
  {#if app.scienceFor}<ScienceModal />{/if}
  {#if app.latexToolsFor}<LatexToolsModal />{/if}
  {#if app.compareFor}<CompareModal />{/if}
  {#if app.aiFicheFor}<AiFicheModal />{/if}
  <Dialogs />
{/if}

<style>
  .build-alert {
    position: fixed;
    z-index: 1000;
    top: 0;
    left: 0;
    right: 0;
    padding: 10px 16px;
    background: var(--danger);
    color: #fff;
    font-size: 13px;
    line-height: 1.5;
  }
  .build-alert .mono {
    display: block;
    font-family: var(--font-mono);
    font-size: 11.5px;
    opacity: 0.9;
  }
  .app {
    display: flex;
    flex-direction: column;
    height: 100%;
  }
  .main {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .left,
  .right {
    flex-shrink: 0;
    background: var(--bg-soft);
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .center {
    flex: 1;
    min-width: 0;
    display: flex;
  }
</style>
