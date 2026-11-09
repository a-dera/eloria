<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { locale, t, tn, translateError } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";

  // Inbox: captures (clipboard, ideas, web pages) waiting to be
  // filed. "Classer" moves the note into a vault folder.
  let items = $state<[string, string, number][]>([]);
  /** Items that arrived in Zotero, not yet in the vault. */
  let newDocs = $state<import("../lib/types").ItemSummary[]>([]);
  let folders = $state<string[]>([]);
  let moving = $state<string | null>(null);
  let target = $state("");

  async function load() {
    items = await api.inbox().catch(() => []);
    newDocs = await api.newZoteroDocuments(8).catch(() => []);
  }
  $effect(() => {
    void app.revision;
    load();
  });

  async function classify(path: string) {
    if (!target.trim()) return;
    try {
      await app.move(path, target.trim());
      moving = null;
      load();
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }

  async function startMove(path: string) {
    folders = await api.folders().catch(() => []);
    target = folders.find((f) => f === "Concepts") ?? folders[0] ?? "";
    moving = path;
  }
</script>

<div class="panel-title">
  <span class="grow">{t("Boîte de réception")}</span>
  <button class="icon-btn" title={t("Capturer le presse-papiers (Ctrl+Maj+V)")} aria-label={t("Capturer le presse-papiers")} onclick={() => app.captureClipboard()}><Icon name="download" size={15} /></button>
  <button class="icon-btn" title={t("Noter une idée")} aria-label={t("Noter une idée")} onclick={() => app.captureIdea()}><Icon name="plus" size={15} /></button>
</div>
<div class="list">
  {#if newDocs.length}
    <div class="sect">{t("Nouveaux documents Zotero")}</div>
    {#each newDocs as d (d.key)}
      <div class="item doc">
        <span class="open static">
          <span class="t">{d.title}</span>
          <span class="d">{d.authors}{d.year ? ` (${d.year})` : ""}</span>
        </span>
        <button class="btn small" onclick={() => (app.flowFor = { key: d.key, title: d.title })}>{t("Préparer")}</button>
      </div>
    {/each}
    <div class="sect">{t("Captures")}</div>
  {/if}
  {#if !items.length}
    <p class="empty">{t("Rien à classer. Capture un texte copié (Ctrl+Maj+V) ou une idée : il arrive ici avant d'être rangé.")}</p>
  {/if}
  {#each items as [path, title, mtime] (path)}
    <div class="item">
      <button class="open" onclick={() => app.openNote(path, { newTab: true })}>
        <span class="t">{title}</span>
        <span class="d">{new Date(mtime).toLocaleString(locale(), { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" })}</span>
      </button>
      {#if moving === path}
        <div class="move">
          <select class="input" bind:value={target} aria-label={t("Dossier de destination")}>
            {#each folders as f (f)}<option value={f}>{f}</option>{/each}
          </select>
          <button class="btn small primary" onclick={() => classify(path)}>{t("Ranger")}</button>
          <button class="btn small" onclick={() => (moving = null)}>{t("Annuler")}</button>
        </div>
      {:else}
        <div class="acts">
          <button class="btn small" onclick={() => startMove(path)}>{t("Classer…")}</button>
          <button class="icon-btn" title={t("Supprimer")} aria-label={t("Supprimer")} onclick={() => app.remove(path).then(load)}><Icon name="trash" size={13} /></button>
        </div>
      {/if}
    </div>
  {/each}
  {#if items.length}<p class="count">{tn(items.length, "{n} élément à classer", "{n} éléments à classer")}</p>{/if}
</div>

<style>
  .list {
    flex: 1;
    overflow: auto;
    padding: 0 8px 16px;
  }
  .empty,
  .count {
    padding: 10px 6px;
    font-size: 12.5px;
    color: var(--text-faint);
  }
  .item {
    padding: 8px 6px;
    border-bottom: 1px solid var(--line);
  }
  .sect {
    padding: 10px 6px 4px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-faint);
  }
  .item.doc {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .item.doc .open {
    flex: 1;
    min-width: 0;
    cursor: default;
  }
  .item.doc .btn {
    flex-shrink: 0;
  }
  .open {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    border: 0;
    background: none;
    padding: 0;
    text-align: left;
    color: var(--text);
    cursor: pointer;
  }
  .t {
    font-size: 13px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .open:hover .t {
    color: var(--accent-text);
  }
  .d {
    font-size: 11.5px;
    color: var(--text-faint);
  }
  .acts,
  .move {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 6px;
  }
  .move select {
    flex: 1;
    min-width: 0;
    height: 28px;
  }
</style>
