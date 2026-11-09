<script lang="ts">
  import { onDestroy, tick } from "svelte";
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { editors } from "../lib/editor/registry";
  import { t, tn, translateError } from "../lib/i18n.svelte";
  import { escapeHtml } from "../lib/util";
  import type { AskAnswer, Passage } from "../lib/types";
  import Icon from "./Icon.svelte";

  // Ask Eloria, as a conversation: each question is searched in the notes, the
  // sources and the library; the answer only relies on the [S1]… excerpts
  // shown below it, and any unchecked reference is flagged.
  type Turn = { q: string; text: string; answer: AskAnswer | null; error: string; running: boolean; mode?: string; modeLabel?: string };
  const MODES = $derived([
    { id: "", label: t("Question"), hint: t("Ta question… (Entrée pour envoyer)") },
    { id: "synthese", label: t("Synthèse"), hint: t("Sujet à synthétiser à partir de tes lectures…") },
    { id: "contradictions", label: t("Contradictions"), hint: t("Sujet sur lequel chercher des divergences entre sources…") },
    { id: "debat", label: t("Débat"), hint: t("Position A contre position B (théories, auteurs, articles)…") },
    { id: "manques", label: t("Manques"), hint: t("Chapitre ou question de recherche à vérifier…") },
    ...(app.settings?.ai_agents ?? []).filter((a) => a.name.trim()).map((a) => ({ id: `custom:${a.instructions}`, label: a.name, hint: t("Ta demande à « {name} »…", { name: a.name }) })),
  ]);
  let mode = $state("");
  const current = $derived(MODES.find((m) => m.id === mode) ?? MODES[0]);
  const KEY = () => `eloria:ask:${app.vault?.root ?? ""}`;
  function load(): Turn[] {
    try {
      return (JSON.parse(localStorage.getItem(KEY()) ?? "[]") as Turn[]).map((x) => ({ ...x, running: false }));
    } catch {
      return [];
    }
  }
  let turns = $state<Turn[]>(load());
  let question = $state("");
  let list = $state<HTMLDivElement>();
  let input = $state<HTMLTextAreaElement>();

  function persist() {
    try {
      localStorage.setItem(KEY(), JSON.stringify($state.snapshot(turns).slice(-30)));
    } catch {
      /* stockage indisponible */
    }
  }

  async function scrollEnd() {
    await tick();
    if (list) list.scrollTop = list.scrollHeight;
  }

  const running = $derived(turns.some((x) => x.running));

  async function ask(q?: string) {
    const text = (q ?? question).trim();
    if (!text || running) return;
    question = "";
    autosize();
    turns.push({ q: text, text: "", answer: null, error: "", running: true, mode, modeLabel: mode ? current.label : "" });
    const turn = turns[turns.length - 1];
    scrollEnd();
    try {
      turn.answer = await api.aiAsk(
        text,
        (d) => {
          turn.text += d;
          if (list && list.scrollHeight - list.scrollTop - list.clientHeight < 120) scrollEnd();
        },
        turn.mode ?? "",
      );
    } catch (e) {
      turn.error = translateError(e);
    } finally {
      turn.running = false;
      persist();
      scrollEnd();
    }
  }

  function autosize() {
    if (!input) return;
    input.style.height = "auto";
    input.style.height = `${Math.min(160, input.scrollHeight)}px`;
  }

  /** Answer as safe HTML: bold, italics, headings, lists, clickable [S2] references. */
  function render(turn: Turn): string {
    const src = turn.answer?.text ?? turn.text;
    const lines = escapeHtml(src).split("\n");
    const out: string[] = [];
    let inList = false;
    for (const raw of lines) {
      let l = raw
        .replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>")
        .replace(/(?<![*\w])\*([^*\n]+)\*(?!\*)/g, "<i>$1</i>")
        .replace(/\[S(\d+)\]/g, (m, n) => {
          if (!turn.answer) return `<span class="ref">${m}</span>`;
          const ok = turn.answer.passages.some((p) => p.id === Number(n));
          return ok ? `<button class="ref" data-s="${n}">${m}</button>` : `<span class="ref bad" title="${escapeHtml(t("Extrait inexistant"))}">${m}</span>`;
        });
      const li = /^\s*[-*]\s+(.*)$/.exec(l);
      if (li) {
        if (!inList) out.push("<ul>");
        inList = true;
        out.push(`<li>${li[1]}</li>`);
        continue;
      }
      if (inList) {
        out.push("</ul>");
        inList = false;
      }
      const h = /^#{1,4}\s+(.*)$/.exec(l);
      if (h) l = `<p class="h">${h[1]}</p>`;
      else if (!l.trim()) l = "";
      else l = `<p>${l}</p>`;
      out.push(l);
    }
    if (inList) out.push("</ul>");
    return out.join("");
  }

  function onAnswerClick(e: MouseEvent, turn: Turn) {
    const b = (e.target as HTMLElement).closest("button.ref") as HTMLElement | null;
    if (!b || !turn.answer) return;
    const p = turn.answer.passages.find((x) => x.id === Number(b.dataset.s));
    if (p) openPassage(p);
  }

  function openPassage(p: Passage) {
    if (p.path) app.openNote(p.path, { newTab: true });
    else if (p.zotero) app.openView("zotero-item", { key: p.zotero, title: `@${p.citekey}` });
  }

  async function asMarkdown(turn: Turn) {
    if (!turn.answer) return "";
    const body = await api.aiAnswerMarkdown(turn.answer.text, turn.answer.passages);
    const used = turn.answer.passages.filter((p) => turn.answer!.text.includes(`[S${p.id}]`));
    const srcs = used.map((p) => `- ${p.citekey ? `[@${p.citekey}]` : `[[${p.path.split("/").pop()!.replace(/\.md$/, "")}]]`} ${p.title}`).join("\n");
    return `> [!question] ${turn.q}\n\n${body.trim()}\n\n**${t("Sources")}**\n${srcs}\n`;
  }

  async function insert(turn: Turn) {
    const ed = editors.active();
    if (!ed) return app.toast(t("Ouvre une note pour y insérer la réponse"), "info");
    ed.insert(`\n${await asMarkdown(turn)}\n`);
    app.toast(t("Réponse insérée dans la note"), "success");
  }

  async function saveNote(turn: Turn) {
    await app.newNote(undefined, turn.q.slice(0, 80).replace(/[\\/:*?"<>|]/g, " "), await asMarkdown(turn));
  }

  function clear() {
    turns = [];
    persist();
  }

  onDestroy(() => {
    if (running) api.aiCancel();
  });

  // Question sent from elsewhere (palette, selection).
  $effect(() => {
    if (app.askSeed) {
      const q = app.askSeed;
      app.askSeed = "";
      ask(q);
    }
  });
  $effect(() => {
    void turns.length;
    scrollEnd();
  });

  const SUGGESTIONS = [
    "Que disent mes lectures sur la fraude transactionnelle ?",
    "Compare les méthodes de détection de mes sources.",
    "Quels concepts approfondir pour le chapitre 2 ?",
  ];
</script>

<div class="chat">
  <div class="head">
    <span class="title">Ask Eloria</span>
    <span class="grow"></span>
    {#if turns.length}<button class="icon-btn" title={t("Effacer la conversation")} aria-label={t("Effacer la conversation")} onclick={clear}><Icon name="trash" size={14} /></button>{/if}
  </div>

  <div class="msgs" bind:this={list}>
    {#if !app.settings?.ai_enabled}
      <div class="intro">
        <p>{t("L'IA est désactivée. Active-la et enregistre ta clé API dans Réglages › Intelligence artificielle.")}</p>
        <button class="btn small" onclick={() => app.openSettings("ia")}>{t("Ouvrir les réglages")}</button>
      </div>
    {:else if !turns.length}
      <div class="intro">
        <p>{t("Pose une question à tes notes, tes sources et ta bibliothèque. Chaque affirmation renvoie à l'extrait d'où elle vient ; rien n'est inventé.")}</p>
        {#each SUGGESTIONS as s (s)}
          <button class="sugg" onclick={() => ask(t(s))}>{t(s)}</button>
        {/each}
      </div>
    {/if}
    {#each turns as turn, i (i)}
      <div class="q">{#if turn.modeLabel}<span class="qmode">{turn.modeLabel}</span>{/if}{turn.q}</div>
      <div class="a">
        {#if turn.running && !turn.text}<span class="typing">{t("Recherche dans tes notes…")}</span>{/if}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div class="md" onclick={(e) => onAnswerClick(e, turn)}>{@html render(turn)}</div>
        {#if turn.error}<p class="err">{turn.error}</p>{/if}
        {#if turn.answer}
          {#if turn.answer.unverified.length}
            <p class="warn">{t("À vérifier, absent de tes extraits et de ta bibliothèque : {list}", { list: turn.answer.unverified.join(", ") })}</p>
          {/if}
          <details>
            <summary>{tn(turn.answer.passages.length, "{n} extrait consulté", "{n} extraits consultés")}</summary>
            <ol class="srcs">
              {#each turn.answer.passages as p (p.id)}
                <li class:used={turn.answer.text.includes(`[S${p.id}]`)}>
                  <button onclick={() => openPassage(p)}><b>S{p.id}</b> {p.title}</button>
                </li>
              {/each}
            </ol>
          </details>
          <div class="acts">
            <button class="btn small ghost" onclick={() => insert(turn)}>{t("Insérer dans la note")}</button>
            <button class="btn small ghost" onclick={() => saveNote(turn)}>{t("Enregistrer en note")}</button>
          </div>
        {/if}
      </div>
    {/each}
  </div>

  <div class="modes" role="radiogroup" aria-label={t("Mode")}>
    {#each MODES as m (m.id)}
      <button role="radio" aria-checked={mode === m.id} class:on={mode === m.id} onclick={() => (mode = m.id)}>{m.label}</button>
    {/each}
  </div>
  <form
    class="compose"
    onsubmit={(e) => {
      e.preventDefault();
      ask();
    }}
  >
    <textarea
      bind:this={input}
      bind:value={question}
      rows="2"
      placeholder={current.hint}
      aria-label={t("Ta question")}
      disabled={!app.settings?.ai_enabled}
      oninput={autosize}
      onkeydown={(e) => {
        if (e.key === "Enter" && !e.shiftKey) {
          e.preventDefault();
          ask();
        }
      }}
    ></textarea>
    {#if running}
      <button type="button" class="send" title={t("Arrêter")} aria-label={t("Arrêter")} onclick={() => api.aiCancel()}><Icon name="x" size={15} /></button>
    {:else}
      <button type="submit" class="send" disabled={!question.trim()} title={t("Envoyer")} aria-label={t("Envoyer")}><Icon name="chevron" size={15} /></button>
    {/if}
  </form>
</div>

<style>
  .chat {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .head {
    display: flex;
    align-items: center;
    padding: 8px 12px 4px;
  }
  .title {
    font-weight: 600;
    font-size: 13px;
  }
  .grow {
    flex: 1;
  }
  .msgs {
    flex: 1;
    overflow-y: auto;
    padding: 6px 12px 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .intro {
    color: var(--text-soft);
    font-size: 12.5px;
    line-height: 1.5;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .intro p {
    margin: 0 0 4px;
  }
  .intro .btn {
    align-self: flex-start;
  }
  .sugg {
    text-align: left;
    padding: 7px 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg-raised);
    color: var(--text);
    font: 12.5px var(--font-ui);
    cursor: pointer;
  }
  .sugg:hover {
    border-color: var(--accent);
  }
  .q {
    align-self: flex-end;
    max-width: 88%;
    padding: 7px 11px;
    border-radius: 12px 12px 3px 12px;
    background: var(--accent-soft);
    color: var(--text);
    font-size: 13px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .a {
    font-size: 13px;
    line-height: 1.55;
    overflow-wrap: anywhere;
  }
  .md :global(p) {
    margin: 0 0 7px;
  }
  .md :global(p.h) {
    font-weight: 700;
    margin-top: 10px;
  }
  .md :global(ul) {
    margin: 0 0 7px;
    padding-left: 18px;
  }
  .md :global(.ref) {
    display: inline-block;
    padding: 0 4px;
    margin: 0 1px;
    border: 0;
    border-radius: 4px;
    background: var(--accent-soft);
    color: var(--accent-text);
    font: 600 11px var(--font-ui);
    cursor: pointer;
    vertical-align: 1px;
  }
  .md :global(.ref.bad) {
    background: none;
    color: var(--danger);
    text-decoration: line-through;
    cursor: help;
  }
  .typing {
    color: var(--text-faint);
    font-style: italic;
  }
  .err,
  .warn {
    color: var(--danger);
    font-size: 12px;
    margin: 4px 0;
  }
  details {
    margin-top: 4px;
    font-size: 12px;
  }
  summary {
    cursor: pointer;
    color: var(--text-soft);
  }
  .srcs {
    margin: 4px 0;
    padding-left: 18px;
    color: var(--text-faint);
  }
  .srcs li.used {
    color: var(--text);
  }
  .srcs button {
    border: 0;
    background: none;
    padding: 1px 0;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .srcs button:hover {
    color: var(--accent-text);
    text-decoration: underline;
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 4px;
  }
  .modes {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 8px 10px 0;
    border-top: 1px solid var(--line);
  }
  .modes button {
    padding: 2px 9px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: transparent;
    color: var(--text-soft);
    font: 11.5px var(--font-ui);
    white-space: nowrap;
    cursor: pointer;
  }
  .modes button.on {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  .qmode {
    display: block;
    font-size: 10.5px;
    font-weight: 600;
    color: var(--accent-text);
    margin-bottom: 2px;
  }
  .compose {
    display: flex;
    align-items: flex-end;
    gap: 6px;
    padding: 8px 10px 10px;
  }
  textarea {
    flex: 1;
    resize: none;
    max-height: 160px;
    padding: 8px 10px;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--bg-raised);
    color: var(--text);
    font: 13px/1.45 var(--font-ui);
    outline: none;
  }
  textarea:focus {
    border-color: var(--accent);
  }
  .send {
    width: 34px;
    height: 34px;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: 50%;
    background: var(--accent);
    color: #fff;
    cursor: pointer;
  }
  .send:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .send :global(svg) {
    transform: rotate(-90deg);
  }
</style>
