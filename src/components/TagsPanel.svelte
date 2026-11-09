<script lang="ts">
  import { t } from "../lib/i18n.svelte";
  import { app } from "../lib/state.svelte";

  interface Node {
    name: string;
    full: string;
    count: number;
    children: Node[];
  }

  let filter = $state("");

  const tree = $derived.by(() => {
    const root: Node[] = [];
    const f = filter.toLowerCase();
    for (const [tag, count] of app.tags) {
      if (f && !tag.toLowerCase().includes(f)) continue;
      const parts = tag.split("/");
      let level = root;
      let acc = "";
      for (const [i, part] of parts.entries()) {
        acc = acc ? `${acc}/${part}` : part;
        let n = level.find((x) => x.name.toLowerCase() === part.toLowerCase());
        if (!n) {
          n = { name: part, full: acc, count: 0, children: [] };
          level.push(n);
        }
        if (i === parts.length - 1) n.count += count;
        level = n.children;
      }
    }
    return root;
  });

  function open(tag: string) {
    app.searchQuery = `#${tag}`;
    app.leftPanel = "search";
  }

  const max = $derived(Math.max(1, ...app.tags.map(([, c]) => c)));
</script>

<div class="panel-title"><span class="grow">{t("Tags")}</span><span class="faint">{app.tags.length}</span></div>
<div class="filter"><input class="input" placeholder={t("Filtrer les tags")} bind:value={filter} /></div>
<div class="list">
  {#if !app.tags.length}<p class="empty">{t("Aucun tag. Écris")} <code>#idée</code> {t("dans une note ou ajoute")} <code>tags:</code> {t("dans ses propriétés.")}</p>{/if}
  {@render nodes(tree, 0)}
</div>

{#snippet nodes(list: Node[], depth: number)}
  {#each list as n (n.full)}
    <button class="tag" style:padding-left="{10 + depth * 14}px" onclick={() => open(n.full)}>
      <span class="hash">#</span><span class="name">{n.name}</span>
      {#if n.count}
        <span class="bar"><span style:width="{(n.count / max) * 100}%"></span></span>
        <span class="count">{n.count}</span>
      {/if}
    </button>
    {#if n.children.length}{@render nodes(n.children, depth + 1)}{/if}
  {/each}
{/snippet}

<style>
  .filter {
    padding: 0 10px 8px;
  }
  .list {
    flex: 1;
    overflow: auto;
    padding: 0 6px 20px;
  }
  .tag {
    display: flex;
    align-items: center;
    gap: 4px;
    width: 100%;
    height: 28px;
    border: 0;
    border-radius: 5px;
    background: transparent;
    text-align: left;
    padding-right: 8px;
  }
  .tag:hover {
    background: var(--bg-sunk);
  }
  .hash {
    color: var(--accent-text);
    font-weight: 600;
  }
  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .bar {
    width: 40px;
    height: 3px;
    border-radius: 2px;
    background: var(--line);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--accent);
    opacity: 0.6;
  }
  .count {
    font-size: 11.5px;
    color: var(--text-faint);
    min-width: 18px;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
</style>
