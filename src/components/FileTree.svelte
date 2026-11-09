<script lang="ts">
  import { t, translateError } from "../lib/i18n.svelte";
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import type { TreeNode } from "../lib/types";
  import Icon from "./Icon.svelte";

  let renaming = $state<string | null>(null);
  let renameValue = $state("");
  let dragOver = $state<string | null>(null);

  function toggle(path: string) {
    app.expanded[path] = !app.expanded[path];
    app.saveLayout();
  }

  function startRename(n: TreeNode) {
    renaming = n.path;
    renameValue = n.is_dir ? n.name : n.name.replace(/\.md$/, "");
    setTimeout(() => {
      const el = document.querySelector<HTMLInputElement>(".tree-rename");
      el?.focus();
      el?.select();
    }, 10);
  }

  async function commitRename(n: TreeNode) {
    const v = renameValue.trim();
    renaming = null;
    if (!v || v === n.name.replace(/\.md$/, "")) return;
    try {
      await app.rename(n.path, v);
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }

  function menu(e: MouseEvent, n: TreeNode) {
    const folder = n.is_dir ? n.path : n.path.includes("/") ? n.path.slice(0, n.path.lastIndexOf("/")) : "";
    app.openMenu(e, [
      { label: t("Nouvelle note ici"), icon: "file-plus", run: () => app.newNote(folder) },
      { label: t("Nouveau dossier ici"), icon: "folder-plus", run: () => app.newFolder(folder) },
      { separator: true, label: "" },
      ...(!n.is_dir
        ? [
            { label: t("Ouvrir dans un nouvel onglet"), icon: "plus", run: () => app.openNote(n.path, { newTab: true }) },
            { label: t("Ouvrir à côté"), icon: "split", run: () => app.openNote(n.path, { otherPane: true }) },
          ]
        : []),
      { label: t("Renommer"), icon: "edit", run: () => startRename(n) },
      { label: t("Afficher dans l'explorateur"), icon: "external", run: () => api.revealPath(n.path) },
      { separator: true, label: "" },
      { label: t("Supprimer"), icon: "trash", danger: true, run: () => app.remove(n.path) },
    ]);
  }

  function onDrop(e: DragEvent, folder: string) {
    e.preventDefault();
    dragOver = null;
    const p = e.dataTransfer?.getData("text/x-eloria-path");
    if (p) app.move(p, folder);
    // PDF dropped from disk onto the file tree: added to the library.
    else if (e.dataTransfer?.files.length) void app.dropPdfs([...e.dataTransfer.files]);
  }

  const activePath = $derived(app.activeNotePath);
</script>

<div class="panel-title">
  <span class="grow">{app.vault?.name}</span>
  <button class="icon-btn" title={t("Nouvelle note (Ctrl+N)")} aria-label={t("Nouvelle note")} onclick={() => app.newNote("")}><Icon name="file-plus" /></button>
  <button class="icon-btn" title={t("Nouveau dossier")} aria-label={t("Nouveau dossier")} onclick={() => app.newFolder("")}><Icon name="folder-plus" /></button>
  <button class="icon-btn" title={t("Tout replier")} aria-label={t("Tout replier")} onclick={() => { app.expanded = {}; app.saveLayout(); }}><Icon name="list" /></button>
</div>

<div
  class="tree"
  role="tree"
  tabindex="-1"
  class:drop={dragOver === ""}
  ondragover={(e) => { e.preventDefault(); dragOver = ""; }}
  ondragleave={() => (dragOver = null)}
  ondrop={(e) => onDrop(e, "")}
  oncontextmenu={(e) => app.openMenu(e, [
    { label: t("Nouvelle note"), icon: "file-plus", run: () => app.newNote("") },
    { label: t("Nouveau dossier"), icon: "folder-plus", run: () => app.newFolder("") },
  ])}
>
  {#if app.tree}
    {@render nodes(app.tree.children, 0)}
    {#if !app.tree.children.length}
      <p class="empty">{t("Coffre vide.")}<br />{t("Crée ta première note avec")} <kbd>Ctrl</kbd>+<kbd>N</kbd>.</p>
    {/if}
  {/if}
</div>

{#snippet nodes(list: TreeNode[], depth: number)}
  {#each list as n (n.path)}
    <div
      class="item"
      class:dir={n.is_dir}
      class:active={!n.is_dir && n.path === activePath}
      class:drop={dragOver === n.path}
      style:padding-left="{10 + depth * 14}px"
      role="treeitem"
      aria-selected={!n.is_dir && n.path === activePath}
      aria-expanded={n.is_dir ? !!app.expanded[n.path] : undefined}
      tabindex="0"
      draggable={renaming !== n.path}
      ondragstart={(e) => e.dataTransfer?.setData("text/x-eloria-path", n.path)}
      ondragover={(e) => {
        if (!n.is_dir) return;
        e.preventDefault();
        e.stopPropagation();
        dragOver = n.path;
      }}
      ondrop={(e) => {
        if (!n.is_dir) return;
        e.stopPropagation();
        onDrop(e, n.path);
      }}
      onclick={(e) => (n.is_dir ? toggle(n.path) : app.openNote(n.path, { newTab: true, otherPane: e.altKey }))}
      onkeydown={(e) => {
        if (e.key === "Enter") n.is_dir ? toggle(n.path) : app.openNote(n.path);
        if (e.key === "F2") startRename(n);
        if (e.key === "Delete") app.remove(n.path);
      }}
      oncontextmenu={(e) => menu(e, n)}
      ondblclick={() => n.is_dir || startRename(n)}
    >
      {#if n.is_dir}
        <span class="chev" class:open={app.expanded[n.path]}><Icon name="chevron" size={13} stroke={2} /></span>
      {:else}
        <span class="ficon"><Icon name={n.kind === "note" ? "note" : n.kind} size={14} /></span>
      {/if}
      {#if renaming === n.path}
        <input
          class="tree-rename"
          bind:value={renameValue}
          onclick={(e) => e.stopPropagation()}
          onkeydown={(e) => {
            e.stopPropagation();
            if (e.key === "Enter") commitRename(n);
            if (e.key === "Escape") renaming = null;
          }}
          onblur={() => commitRename(n)}
        />
      {:else}
        <span class="label">{n.is_dir ? n.name : n.name.replace(/\.md$/, "")}</span>
        {#if !n.is_dir && n.kind !== "note"}<span class="ext">{n.name.split(".").pop()}</span>{/if}
      {/if}
    </div>
    {#if n.is_dir && app.expanded[n.path]}
      <div class="children" style:--guide="{16 + depth * 14}px">
        {@render nodes(n.children, depth + 1)}
      </div>
    {/if}
  {/each}
{/snippet}

<style>
  .tree {
    flex: 1;
    overflow: auto;
    padding: 2px 6px 24px;
    outline: none;
  }
  .tree.drop {
    background: var(--accent-soft);
  }
  .item {
    display: flex;
    align-items: center;
    gap: 5px;
    height: 27px;
    padding-right: 8px;
    border-radius: 5px;
    cursor: pointer;
    color: var(--text-soft);
    white-space: nowrap;
  }
  .item:hover {
    background: var(--bg-sunk);
    color: var(--text);
  }
  .item.dir {
    color: var(--text);
    font-weight: 500;
  }
  .item.active {
    background: var(--accent-soft);
    color: var(--text);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .item.drop {
    outline: 1.5px dashed var(--accent);
  }
  .label {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ext {
    font-size: 10.5px;
    color: var(--text-faint);
    border: 1px solid var(--line-strong);
    border-radius: 4px;
    padding: 0 4px;
  }
  .chev {
    display: inline-grid;
    width: 14px;
    color: var(--text-faint);
    transition: transform 0.15s var(--ease);
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .ficon {
    color: var(--text-faint);
    display: inline-grid;
    width: 14px;
  }
  .children {
    position: relative;
  }
  .children::before {
    content: "";
    position: absolute;
    left: var(--guide);
    top: 0;
    bottom: 4px;
    width: 1px;
    background: var(--line);
  }
  .tree-rename {
    flex: 1;
    min-width: 0;
    height: 22px;
    border: 1px solid var(--accent);
    border-radius: 4px;
    padding: 0 5px;
    background: var(--bg-raised);
    outline: none;
  }
</style>
