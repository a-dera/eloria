<script lang="ts">
  import { locale, t, tk, translateError } from "../lib/i18n.svelte";
  import { app, type RightPanel } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { editors } from "../lib/editor/registry";
  import type { ItemSummary, LinkRef, SearchHit } from "../lib/types";
  import { escapeHtml, parsePropInput, propText, snippetHtml } from "../lib/util";
  import { initialValue, linkTarget, propDef, typedValue } from "../lib/custom";
  import Icon from "./Icon.svelte";
  import GraphView from "./GraphView.svelte";
  import AskChat from "./AskChat.svelte";

  const tabs: { id: RightPanel; icon: string; label: string }[] = [
    { id: "backlinks", icon: "link", label: tk("Liens") },
    { id: "outline", icon: "outline", label: tk("Plan") },
    { id: "graph", icon: "graph", label: tk("Graphe local") },
    { id: "properties", icon: "props", label: tk("Propriétés") },
    { id: "ask", icon: "quote", label: tk("Ask Eloria") },
  ];

  const path = $derived(app.contextNotePath);
  let backlinks = $state<LinkRef[]>([]);
  let unlinked = $state<SearchHit[] | null>(null);
  let cites = $state<ItemSummary[]>([]);
  let depth = $state(1);
  let withTags = $state(false);

  $effect(() => {
    const p = path;
    void app.revision;
    unlinked = null;
    if (!p) return;
    api.backlinks(p).then((b) => (backlinks = b)).catch(() => (backlinks = []));
  });

  // References cited in the note
  $effect(() => {
    void app.activeStats.chars;
    const v = editors.active()?.view;
    if (!v) return;
    const text = v.state.doc.toString();
    const keys = new Set<string>();
    for (const g of text.matchAll(/\[[^\[\]\n]*@[^\[\]\n]*\]/g)) for (const k of g[0].matchAll(/@([\p{L}\p{N}_][\p{L}\p{N}_:.\-]*[\p{L}\p{N}_])/gu)) keys.add(k[1].toLowerCase());
    cites = app.zoteroItems.filter((i) => keys.has(i.citekey.toLowerCase()));
  });

  const grouped = $derived.by(() => {
    const m = new Map<string, { title: string; refs: LinkRef[] }>();
    for (const b of backlinks) {
      if (!m.has(b.path)) m.set(b.path, { title: b.title, refs: [] });
      m.get(b.path)!.refs.push(b);
    }
    return [...m.entries()];
  });

  function ctxHtml(s: string): string {
    // The context of a link is a Markdown line: the text is kept, without markup.
    const plain = s
      .replace(/^\s*(?:[-*+]|\d+\.|>+|#{1,6})\s+/, "")
      .replace(/\*\*([^*]+)\*\*/g, "$1")
      .replace(/(?<![*\w])\*([^*]+)\*(?!\*)/g, "$1")
      .replace(/==([^=]+)==/g, "$1")
      .replace(/`([^`]+)`/g, "$1")
      .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
      .replace(/^(\w+):\s*["']?/, "$1 : ")
      .replace(/["']$/, "");
    return escapeHtml(plain).replace(/\[\[([^\]|`\[]+)(?:\|([^\]`]+))?\]\]/g, (_m, tb, a) => `<b>${a ?? tb}</b>`);
  }

  // Outline: headings of the active note, recomputed while typing
  const headings = $derived.by(() => {
    void app.activeStats.chars;
    void path;
    const v = editors.active()?.view;
    if (!v) return [] as { level: number; text: string; line: number }[];
    const out: { level: number; text: string; line: number }[] = [];
    let inCode = false;
    const doc = v.state.doc;
    for (let i = 1; i <= doc.lines; i++) {
      const tb = doc.line(i).text;
      if (/^(```|~~~)/.test(tb)) inCode = !inCode;
      if (inCode) continue;
      const m = /^(#{1,6})\s+(.+?)\s*#*$/.exec(tb);
      if (m) out.push({ level: m[1].length, text: m[2], line: i - 1 });
    }
    return out;
  });
  const minLevel = $derived(Math.min(6, ...headings.map((h) => h.level)));
  const currentHeading = $derived.by(() => {
    let cur = -1;
    for (const [i, h] of headings.entries()) if (h.line <= app.cursorLine) cur = i;
    return cur;
  });

  // Properties
  const meta = $derived(app.noteByPath(path));
  let newKey = $state("");

  async function setProp(key: string, value: unknown) {
    if (!path) return;
    await editors.savePath(path);
    try {
      const content = await api.setProperty(path, key, value);
      editors.pushContent(path, content);
      await app.refresh();
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }

  async function addProp() {
    const k = newKey.trim().replace(/\s+/g, "_");
    if (!k) return;
    newKey = "";
    await setProp(k, initialValue(propDef(k)));
  }
</script>

<div class="rp">
  <div class="tabs" role="tablist">
    {#each tabs as tb (tb.id)}
      <button class="icon-btn" class:active={app.rightPanel === tb.id} title={t(tb.label)} aria-label={t(tb.label)} role="tab" aria-selected={app.rightPanel === tb.id} onclick={() => (app.rightPanel = tb.id)}>
        <Icon name={tb.icon} />
      </button>
    {/each}
  </div>

  {#if app.rightPanel === "ask"}
    <AskChat />
  {:else}
  <div class="body">
    {#if !path}
      <p class="empty">{t("Ouvre une note.")}</p>
    {:else if app.rightPanel === "backlinks"}
      <h4>{t("Liens entrants")} <span class="faint">{backlinks.length}</span></h4>
      {#each grouped as [p, g] (p)}
        <div class="group">
          <button class="src" onclick={(e) => app.openNote(p, { newTab: true })}>{g.title}</button>
          {#each g.refs as r, i (i)}
            <button class="ctx" onclick={() => app.openNote(p, { line: r.line })}>{@html ctxHtml(r.context)}</button>
          {/each}
        </div>
      {:else}
        <p class="faint small">{t("Aucune note ne renvoie ici pour l'instant.")}</p>
      {/each}

      {#if cites.length}
        <h4>{t("Références citées")} <span class="faint">{cites.length}</span></h4>
        {#each cites as c (c.key)}
          {@const fiche = app.fiches[c.citekey.toLowerCase()]}
          <button class="ref" onclick={() => (fiche ? app.openNote(fiche) : app.openView("zotero-item", { key: c.key, title: `@${c.citekey}` }))}>
            <span class="rt">{c.authors} ({c.year ?? t("s.d.")})</span>
            <span class="faint">{c.title}</span>
          </button>
        {/each}
      {/if}

      <h4>{t("Mentions non liées")}</h4>
      {#if unlinked === null}
        <button class="btn small" onclick={async () => (unlinked = await api.unlinkedMentions(path!))}>{t("Chercher")}</button>
      {:else}
        {#each unlinked as u (u.path)}
          <button class="ctx" onclick={() => app.openNote(u.path)}><b>{u.title}</b><br />{@html snippetHtml(u.snippet)}</button>
        {:else}
          <p class="faint small">{t("Aucune mention du nom de cette note sans lien.")}</p>
        {/each}
      {/if}
    {:else if app.rightPanel === "outline"}
      <h4>{t("Plan")}</h4>
      {#each headings as h, i (i)}
        <button class="h" class:cur={i === currentHeading} style:padding-left="{8 + (h.level - minLevel) * 14}px" onclick={() => editors.active()?.reveal(h.line)}>
          {h.text.replace(/\[\[([^\]|]+)\|?([^\]]*)\]\]/g, (_m, a, b) => b || a).replace(/[*_=~`]/g, "")}
        </button>
      {:else}
        <p class="faint small">{t("Pas de titre dans cette note.")}</p>
      {/each}
    {:else if app.rightPanel === "graph"}
      <div class="lg-tools">
        <label>{t("Profondeur")} <input type="range" min="1" max="3" bind:value={depth} /> {depth}</label>
        <label><input type="checkbox" bind:checked={withTags} /> {t("Tags")}</label>
      </div>
      <div class="lg"><GraphView visible={true} local={path} {depth} includeTags={withTags} /></div>
    {:else if app.rightPanel === "properties"}
      <h4>{t("Propriétés")}</h4>
      {#if meta}
        <div class="props">
          {#each Object.entries(meta.frontmatter) as [k, v] (k)}
            {@const d = propDef(k)}
            <label class="pk" for="prop-{k}">{k}</label>
            <div class="pv" data-kind={d?.kind ?? "texte"}>
              {#if d?.kind === "case"}
                <input id="prop-{k}" type="checkbox" checked={v === true || v === "true"} onchange={(e) => setProp(k, (e.target as HTMLInputElement).checked)} />
                <span class="grow"></span>
              {:else if d?.kind === "choix"}
                <select id="prop-{k}" class="input" value={propText(v)} onchange={(e) => setProp(k, (e.target as HTMLSelectElement).value)}>
                  {#if !d.options.includes(propText(v))}<option value={propText(v)}>{propText(v) || t("(vide)")}</option>{/if}
                  {#each d.options as o (o)}<option value={o}>{o}</option>{/each}
                </select>
              {:else if d && (d.kind === "nombre" || d.kind === "date")}
                <input id="prop-{k}" class="input" type={d.kind === "nombre" ? "number" : "date"} step="any" value={propText(v)} onchange={(e) => setProp(k, typedValue(d, (e.target as HTMLInputElement).value))} />
              {:else if d}
                <input
                  id="prop-{k}"
                  class="input"
                  value={propText(v)}
                  placeholder={d.kind === "lien" ? t("Nom de la note") : d.kind === "liste" ? t("valeur, valeur") : ""}
                  onchange={(e) => setProp(k, typedValue(d, (e.target as HTMLInputElement).value))}
                />
                {#if d.kind === "lien" && linkTarget(v)}
                  <button class="icon-btn" title={t("Ouvrir la note liée")} aria-label={t("Ouvrir la note liée")} onclick={() => path && app.followLink(linkTarget(v), path, { newTab: true })}><Icon name="link" size={14} /></button>
                {/if}
              {:else}
                <input
                  id="prop-{k}"
                  class="input"
                  value={propText(v)}
                  title={Array.isArray(v) ? t("Liste : sépare les valeurs par des virgules") : ""}
                  onchange={(e) => setProp(k, parsePropInput((e.target as HTMLInputElement).value, v))}
                />
              {/if}
              <button class="icon-btn" title={t("Supprimer")} aria-label={t("Supprimer {name}", { name: k })} onclick={() => setProp(k, null)}><Icon name="x" size={14} /></button>
            </div>
          {/each}
        </div>
        <div class="add">
          <input class="input" list="prop-defs" placeholder={t("Nouvelle propriété (ex. statut)")} bind:value={newKey} onkeydown={(e) => e.key === "Enter" && addProp()} />
          <datalist id="prop-defs">
            {#each (app.settings?.property_defs ?? []).filter((d) => !(d.name in meta.frontmatter)) as d (d.name)}<option value={d.name}></option>{/each}
          </datalist>
          <button class="btn small" onclick={addProp}>{t("Ajouter")}</button>
        </div>
        <p class="faint small">{t("Mots : {n}", { n: meta.words.toLocaleString(locale()) })} · {t("Tags : {list}", { list: meta.tags.map((tag) => `#${tag}`).join(" ") || t("aucun") })}</p>
      {/if}
    {/if}
  </div>
  {/if}
</div>

<style>
  .rp {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .tabs {
    display: flex;
    gap: 2px;
    padding: 5px 8px;
    height: 40px;
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
    align-items: center;
  }
  .body {
    flex: 1;
    overflow: auto;
    padding: 4px 12px 24px;
    display: flex;
    flex-direction: column;
  }
  h4 {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-soft);
    margin: 14px 0 6px;
  }
  .small {
    font-size: 12px;
  }
  .body > :global(.btn) {
    align-self: flex-start;
  }
  .group {
    margin-bottom: 10px;
  }
  .src,
  .ctx,
  .ref,
  .h {
    display: block;
    width: 100%;
    text-align: left;
    border: 0;
    background: transparent;
    border-radius: 5px;
  }
  .src {
    font-weight: 600;
    padding: 3px 6px;
  }
  .src:hover {
    color: var(--accent-text);
  }
  .ctx {
    padding: 6px 8px;
    margin: 2px 0;
    font-size: 12.5px;
    color: var(--text-soft);
    line-height: 1.5;
    background: var(--bg);
    border: 1px solid var(--line);
  }
  .ctx:hover {
    border-color: var(--line-strong);
  }
  .ctx :global(b) {
    color: var(--accent-text);
    font-weight: 600;
  }
  .ref {
    padding: 5px 6px;
    font-size: 12.5px;
  }
  .ref:hover {
    background: var(--bg-sunk);
  }
  .rt {
    display: block;
    font-weight: 600;
  }
  .ref .faint {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .h {
    padding: 4px 8px;
    font-size: 12.5px;
    color: var(--text-soft);
    position: relative;
  }
  .h:hover {
    background: var(--bg-sunk);
    color: var(--text);
  }
  .h.cur {
    color: var(--accent-text);
    font-weight: 600;
  }
  .h.cur::before {
    content: "";
    position: absolute;
    left: 0;
    top: 6px;
    bottom: 6px;
    width: 2px;
    border-radius: 2px;
    background: var(--accent);
  }
  .lg-tools {
    display: flex;
    gap: 14px;
    font-size: 12px;
    color: var(--text-soft);
    padding: 8px 0;
    align-items: center;
  }
  .lg-tools label {
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .lg-tools input[type="range"] {
    width: 60px;
    accent-color: var(--accent);
  }
  .lg {
    flex: 1;
    min-height: 320px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
    display: flex;
    background: var(--bg);
  }
  .props {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 10px;
    align-items: center;
  }
  .pk {
    font-size: 12px;
    color: var(--text-soft);
  }
  .pv {
    display: flex;
    gap: 2px;
    align-items: center;
  }
  .pv .input {
    height: 28px;
    font-size: 12.5px;
    min-width: 0;
    flex: 1;
  }
  .pv input[type="checkbox"] {
    accent-color: var(--accent);
    margin: 0 4px;
  }
  .pv .grow {
    flex: 1;
  }
  .add {
    display: flex;
    gap: 6px;
    margin: 12px 0;
  }
  .add .input {
    height: 28px;
    font-size: 12.5px;
  }
</style>
