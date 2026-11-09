<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { readingTime } from "../lib/util";
  import { locale, t, tn } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";
  import { extState } from "../lib/extensions.svelte";
  import { devlog } from "../lib/devlog.svelte";

  const tab = $derived(app.activeTab);
  // Focus session timer (refreshed every second).
  let now = $state(Date.now());
  $effect(() => {
    if (!app.focus) return;
    const id = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(id);
  });
  const left = $derived(app.focus ? Math.max(0, app.focus.end - now) : 0);
  const mmss = $derived(`${Math.floor(left / 60000)}:${String(Math.floor((left % 60000) / 1000)).padStart(2, "0")}`);
</script>

<footer class="status">
  <span>{tn(app.notes.length, "{n} note", "{n} notes")}</span>
  <button
    class="zsync"
    class:busy={app.zoteroSyncing}
    title={app.zoteroStatus?.last_sync ? t("Dernière synchronisation : {when}. Cliquer pour synchroniser.", { when: app.zoteroStatus.last_sync }) : t("Synchroniser Zotero")}
    onclick={() => app.zoteroSync()}
    disabled={app.zoteroSyncing}
  >
    <Icon name="sync" size={12} />
    {app.zoteroSyncing ? t("Synchronisation…") : app.zoteroStatus?.items ? tn(app.zoteroStatus.items, "{n} référence Zotero", "{n} références Zotero") : t("Synchroniser Zotero")}
  </button>
  {#if app.focus}
    <button class="zsync focus-on" title={t("Arrêter la session de concentration")} onclick={() => app.stopFocus()}><Icon name="x" size={11} /> {t("Concentration")} {mmss}</button>
  {:else}
    <button class="zsync" title={t("Session de concentration de {m} min (Réglages › Général)", { m: app.settings?.focus_minutes ?? 25 })} onclick={() => app.startFocus()}>{t("Concentration")}</button>
  {/if}
  <span class="grow"></span>
  {#each extState.status as it (it.key)}
    {#if it.text}
      {#if it.onClick}<button class="zsync ext" title={it.title} onclick={() => it.onClick?.()}>{it.text}</button>{:else}<span class="ext" title={it.title}>{it.text}</span>{/if}
    {/if}
  {/each}
  {#if devlog.errors}
    <button class="zsync errs" title={t("Erreurs dans la console développeur")} onclick={() => app.openView("console")}>{tn(devlog.errors, "{n} erreur", "{n} erreurs")}</button>
  {/if}
  {#if tab?.kind === "note"}
    {#if app.activeStats.selection}<span class="sel">{tn(app.activeStats.selection, "{n} mot sélectionné", "{n} mots sélectionnés")}</span>{/if}
    <span>{tn(app.activeStats.words, "{n} mot", "{n} mots", { n: app.activeStats.words.toLocaleString(locale()) })}</span>
    <span>{tn(app.activeStats.chars, "{n} caractère", "{n} caractères", { n: app.activeStats.chars.toLocaleString(locale()) })}</span>
    <span>{t("{time} de lecture", { time: readingTime(app.activeStats.words) })}</span>
    <span class:dirty={tab.dirty}>{tab.dirty ? t("Modifiée") : t("Enregistrée")}</span>
  {/if}
</footer>

<style>
  .status {
    height: 26px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 0 14px;
    border-top: 1px solid var(--line);
    background: var(--bg-soft);
    color: var(--text-faint);
    font-size: 11.5px;
    font-variant-numeric: tabular-nums;
  }
  .errs {
    color: var(--danger);
  }
  .ext {
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .grow {
    flex: 1;
  }
  .zsync {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 1px 6px;
    border: 0;
    border-radius: var(--radius);
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .focus-on {
    color: var(--accent-text);
    font-variant-numeric: tabular-nums;
  }
  .zsync:hover {
    background: var(--bg-sunk);
    color: var(--text-soft);
  }
  .zsync.busy :global(svg) {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .dirty {
    color: var(--accent-text);
  }
  .sel {
    color: var(--text-soft);
  }
</style>
