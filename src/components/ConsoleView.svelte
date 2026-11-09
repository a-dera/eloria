<script lang="ts">
  // Developer console: log (application and extensions) and code evaluation.
  import { t, tk } from "../lib/i18n.svelte";
  import { app } from "../lib/state.svelte";
  import { clearLog, devlog, fmt, pushLog, type LogLevel } from "../lib/devlog.svelte";
  import { evaluate, extState, loadExtensions } from "../lib/extensions.svelte";
  import Icon from "./Icon.svelte";

  let { visible = true }: { visible?: boolean } = $props();

  let level = $state<"all" | LogLevel>("all");
  let source = $state("");
  let filter = $state("");
  let code = $state("");
  let history = $state<string[]>([]);
  let hpos = -1;
  let list = $state<HTMLDivElement>();
  let follow = $state(true);

  const LEVELS: ["all" | LogLevel, string][] = [
    ["all", tk("Tout")],
    ["error", tk("Erreurs")],
    ["warn", tk("Avertissements")],
    ["info", tk("Infos")],
    ["log", tk("Messages")],
  ];
  const sources = $derived([...new Set(devlog.entries.map((e) => e.source))].sort());
  const shown = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    return devlog.entries.filter((e) => (level === "all" || e.level === level) && (!source || e.source === source) && (!q || e.text.toLowerCase().includes(q)));
  });

  // Automatic scrolling while at the bottom.
  $effect(() => {
    void shown.length;
    if (visible && follow && list) queueMicrotask(() => list && (list.scrollTop = list.scrollHeight));
  });
  // Errors count as seen as soon as the console is displayed.
  $effect(() => {
    if (visible && devlog.errors) devlog.errors = 0;
  });

  function onScroll() {
    if (list) follow = list.scrollHeight - list.scrollTop - list.clientHeight < 30;
  }

  const time = (ms: number) => new Date(ms).toLocaleTimeString(undefined, { hour12: false });

  async function run() {
    const src = code.trim();
    if (!src) return;
    history = [...history.filter((h) => h !== src), src].slice(-50);
    hpos = -1;
    code = "";
    follow = true;
    pushLog("log", "console", [`› ${src}`]);
    try {
      const r = await evaluate(src);
      pushLog("info", "console", [r === undefined ? "undefined" : fmt(r)]);
    } catch (e) {
      pushLog("error", "console", [e]);
    }
  }

  function key(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      run();
    } else if (e.key === "ArrowUp" && !code.includes("\n") && history.length) {
      e.preventDefault();
      hpos = hpos < 0 ? history.length - 1 : Math.max(0, hpos - 1);
      code = history[hpos];
    } else if (e.key === "ArrowDown" && hpos >= 0) {
      e.preventDefault();
      hpos = hpos + 1 < history.length ? hpos + 1 : -1;
      code = hpos < 0 ? "" : history[hpos];
    }
  }

  async function copy() {
    const text = shown.map((e) => `${time(e.time)} [${e.level}] ${e.source} : ${e.text}`).join("\n");
    await navigator.clipboard.writeText(text);
    app.toast(t("{n} lignes copiées", { n: shown.length }), "success");
  }
</script>

<div class="console">
  <div class="toolbar">
    <div class="seg" role="group" aria-label={t("Niveau")}>
      {#each LEVELS as [id, label] (id)}
        <button class:on={level === id} onclick={() => (level = id)}>{t(label)}</button>
      {/each}
    </div>
    <select class="input src" aria-label={t("Origine")} bind:value={source}>
      <option value="">{t("Toutes les origines")}</option>
      {#each sources as s (s)}<option value={s}>{s === "app" ? t("application") : s}</option>{/each}
    </select>
    <input class="input q" placeholder={t("Filtrer")} aria-label={t("Filtrer le journal")} bind:value={filter} />
    <span class="grow"></span>
    <button class="btn small" onclick={() => loadExtensions()} disabled={extState.busy} title={t("Décharger puis recharger les extensions activées")}><Icon name="sync" size={13} /> {t("Recharger les extensions")}</button>
    <button class="btn small ghost" onclick={copy}>{t("Copier")}</button>
    <button class="btn small ghost" onclick={clearLog}>{t("Effacer")}</button>
  </div>
  {#if extState.loaded.length}
    <div class="exts">
      {#each extState.loaded as x (x.id)}
        <span class="chip" class:bad={x.error} title={x.error ?? `${x.name} ${x.version}`}>{x.name}{x.error ? ` : ${t("erreur")}` : ""}</span>
      {/each}
    </div>
  {/if}
  <div class="log" bind:this={list} onscroll={onScroll} role="log" aria-live="polite">
    {#each shown as e (e.id)}
      <div class="entry {e.level}">
        <span class="time">{time(e.time)}</span>
        <span class="from">{e.source === "app" ? t("application") : e.source}</span>
        <pre>{e.text}</pre>
      </div>
    {:else}
      <p class="empty">{t("Rien pour l'instant. Les messages de l'application et des extensions s'affichent ici.")}</p>
    {/each}
  </div>
  <div class="prompt">
    <span class="caret">›</span>
    <textarea
      rows={Math.min(8, code.split("\n").length)}
      bind:value={code}
      onkeydown={key}
      spellcheck="false"
      aria-label={t("Code JavaScript à évaluer")}
      placeholder={t("JavaScript : eloria.notes.list().length, app.activeNotePath… (Entrée : évaluer, Maj+Entrée : nouvelle ligne)")}
    ></textarea>
  </div>
</div>

<style>
  .console {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
    font-size: 12.5px;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--line);
    flex-wrap: wrap;
  }
  .seg {
    display: flex;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .seg button {
    border: 0;
    background: transparent;
    padding: 4px 10px;
    font-size: 12px;
    color: var(--text-soft);
  }
  .seg button + button {
    border-left: 1px solid var(--line);
  }
  .seg button.on {
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  .src {
    width: 150px;
    height: 28px;
  }
  .q {
    width: 130px;
    height: 28px;
  }
  .grow {
    flex: 1;
  }
  .exts {
    display: flex;
    gap: 6px;
    padding: 8px 14px;
    border-bottom: 1px solid var(--line);
    flex-wrap: wrap;
  }
  .chip {
    font-size: 11.5px;
    padding: 2px 8px;
    border-radius: 99px;
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  .chip.bad {
    background: var(--bg-sunk);
    color: var(--danger);
  }
  .log {
    flex: 1;
    overflow: auto;
    font-family: var(--font-mono);
    font-size: 12px;
    background: var(--bg-soft);
  }
  .entry {
    display: grid;
    grid-template-columns: 64px 110px 1fr;
    gap: 10px;
    padding: 4px 14px;
    border-bottom: 1px solid var(--line);
    align-items: baseline;
  }
  .entry pre {
    margin: 0;
    white-space: pre-wrap;
    word-break: break-word;
    font: inherit;
    color: var(--text);
  }
  .time,
  .from {
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .entry.error {
    background: color-mix(in srgb, var(--danger) 8%, transparent);
  }
  .entry.error pre {
    color: var(--danger);
  }
  .entry.warn {
    background: color-mix(in srgb, var(--warning) 9%, transparent);
  }
  .entry.warn pre {
    color: var(--warning);
  }
  .entry.info pre {
    color: var(--text-soft);
  }
  .empty {
    color: var(--text-faint);
    padding: 20px 14px;
    font-family: var(--font-ui);
  }
  .prompt {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    padding: 8px 14px;
    border-top: 1px solid var(--line);
    background: var(--bg);
  }
  .caret {
    color: var(--accent);
    font-family: var(--font-mono);
    line-height: 22px;
  }
  textarea {
    flex: 1;
    border: 0;
    outline: none;
    resize: none;
    background: transparent;
    font-family: var(--font-mono);
    font-size: 12.5px;
    line-height: 22px;
    color: var(--text);
    user-select: text;
  }
</style>
