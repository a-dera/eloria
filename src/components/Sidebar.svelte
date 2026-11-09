<script lang="ts">
  import { app } from "../lib/state.svelte";
  import FileTree from "./FileTree.svelte";
  import SearchPanel from "./SearchPanel.svelte";
  import TagsPanel from "./TagsPanel.svelte";
  import ZoteroPanel from "./ZoteroPanel.svelte";
  import ThesisPanel from "./ThesisPanel.svelte";
  import InboxPanel from "./InboxPanel.svelte";
</script>

<div class="sidebar">
  {#if app.leftPanel === "files"}<FileTree />
  {:else if app.leftPanel === "inbox"}<InboxPanel />
  {:else if app.leftPanel === "search"}<SearchPanel />
  {:else if app.leftPanel === "tags"}<TagsPanel />
  {:else if app.leftPanel === "zotero"}<ZoteroPanel />
  {:else if app.leftPanel === "these"}<ThesisPanel />{/if}
</div>

<style>
  .sidebar {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
</style>
