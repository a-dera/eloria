<script lang="ts">
  import { t, tk } from "../lib/i18n.svelte";
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { commands, keysOf, labelOf, prettyKeys } from "../lib/commands";
  import { editors } from "../lib/editor/registry";
  import { fuzzyScore, stem } from "../lib/util";
  import type { TreeNode } from "../lib/types";

  let query = $state(app.paletteSeed);
  app.paletteSeed = "";
  let index = $state(0);
  let input = $state<HTMLInputElement>();
  let templates = $state<string[]>([]);
  let list = $state<HTMLDivElement>();

  interface Row {
    id: string;
    label: string;
    detail?: string;
    keys?: string;
    run: (e?: KeyboardEvent | MouseEvent) => void;
  }

  $effect(() => {
    if (app.palette) {
      query = "";
      index = 0;
      if (app.palette === "templates") api.templates().then((list) => (templates = list));
    }
  });

  const rows = $derived.by((): Row[] => {
    const q = query.trim();
    if (app.palette === "commands") {
      return commands
        .filter((c) => !c.needsNote || app.activeNotePath)
        .map((c) => ({ c, s: fuzzyScore(q, `${labelOf(c)} ${t(c.group)} ${c.label}`) }))
        .filter((x) => x.s >= 0)
        .sort((a, b) => (q ? b.s - a.s : 0))
        .map(({ c }) => ({ id: c.id, label: labelOf(c), detail: t(c.group), keys: keysOf(c), run: () => c.run() }));
    }
    if (app.palette === "files") {
      const scored = app.notes
        .map((n) => ({ n, s: Math.max(fuzzyScore(q, n.title), fuzzyScore(q, n.path), ...n.aliases.map((a) => fuzzyScore(q, a))) }))
        .filter((x) => x.s >= 0);
      if (!q) scored.sort((a, b) => b.n.mtime - a.n.mtime);
      else scored.sort((a, b) => b.s - a.s);
      const out: Row[] = scored.slice(0, 80).map(({ n }) => ({
        id: n.path,
        label: n.title,
        detail: n.path.replace(/\.md$/, ""),
        run: (e) => app.openNote(n.path, { newTab: true, otherPane: !!e?.altKey }),
      }));
      // The other vault files (LaTeX, bibliographies, PDFs, images) open from here too.
      const others: { path: string; name: string }[] = [];
      const walk = (node: TreeNode | null) => {
        for (const c of node?.children ?? []) {
          if (c.is_dir) walk(c);
          else if (!/\.md$/i.test(c.path)) others.push({ path: c.path, name: c.name });
        }
      };
      walk(app.tree);
      const files = others
        .map((f) => ({ f, s: Math.max(fuzzyScore(q, f.name), fuzzyScore(q, f.path)) }))
        .filter((x) => q && x.s >= 0)
        .sort((a, b) => b.s - a.s)
        .slice(0, 30)
        .map(({ f }) => ({
          id: f.path,
          label: f.name,
          detail: f.path,
          run: (e?: KeyboardEvent | MouseEvent) =>
            /\.(tex|bib)$/i.test(f.path) ? app.openView("latex", { path: f.path, title: f.name, otherPane: !!e?.altKey }) : app.openNote(f.path, { newTab: true, otherPane: !!e?.altKey }),
        }));
      // A file that matches exactly goes before the notes.
      if (files.some((x) => x.label.toLowerCase() === q.toLowerCase())) out.unshift(...files);
      else out.push(...files);
      // "Create" only for a note name (not "test.tex" or "article.pdf").
      const ext = /\.([a-z0-9]{1,6})$/i.exec(q)?.[1]?.toLowerCase();
      if (q && (!ext || ext === "md") && !app.notes.some((n) => n.stem.toLowerCase() === q.toLowerCase())) {
        out.push({ id: "__new", label: t("Créer « {name} »", { name: q }), detail: t("nouvelle note"), run: () => app.newNote(undefined, q, `# ${q}\n\n`) });
      }
      return out;
    }
    if (app.palette === "templates") {
      return templates
        .map((tp) => ({ tp, s: fuzzyScore(q, tp) }))
        .filter((x) => x.s >= 0)
        .map(({ tp }) => ({
          id: tp,
          label: stem(tp),
          detail: t("Entrée : nouvelle note · Maj+Entrée : insérer ici"),
          run: async (e) => {
            if (e?.shiftKey && editors.active()) {
              const txt = await api.renderTemplate(tp, stem(app.activeNotePath ?? ""));
              editors.active()!.insert(txt.replace(/^---\n[\s\S]*?\n---\n/, ""));
            } else app.newNoteFromTemplate(tp);
          },
        }));
    }
    return [];
  });

  $effect(() => {
    void query;
    index = 0;
  });

  $effect(() => {
    list?.querySelector(`[data-i="${index}"]`)?.scrollIntoView({ block: "nearest" });
  });

  /** Immediate focus on opening, so that no keystroke is lost. */
  function focusNow(el: HTMLInputElement) {
    el.focus();
  }

  function close() {
    app.palette = null;
    setTimeout(() => editors.active()?.focus(), 10);
  }

  function choose(r: Row | undefined, e?: KeyboardEvent | MouseEvent) {
    if (!r) return;
    app.palette = null;
    r.run(e);
  }

  function key(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Escape") close();
    else if (e.key === "ArrowDown") {
      e.preventDefault();
      index = Math.min(rows.length - 1, index + 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      index = Math.max(0, index - 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      choose(rows[index], e);
    }
  }

  const placeholders = {
    commands: tk("Tape une commande…"),
    files: tk("Ouvrir une note (Ctrl+Entrée : nouvel onglet, Alt+Entrée : à côté)"),
    templates: tk("Choisir un modèle"),
  };
</script>

{#if app.palette}
  <div class="backdrop" role="presentation" onmousedown={close}>
    <div class="palette" role="dialog" aria-modal="true" aria-label={t("Palette")} tabindex="-1" onmousedown={(e) => e.stopPropagation()}>
      <input bind:this={input} use:focusNow bind:value={query} placeholder={t(placeholders[app.palette])} onkeydown={key} />
      <div class="list" bind:this={list} role="listbox">
        {#each rows as r, i (r.id)}
          <button class="row" class:on={i === index} data-i={i} role="option" aria-selected={i === index} onmousemove={() => (index = i)} onclick={(e) => choose(r, e)}>
            <span class="label">{r.label}</span>
            {#if r.detail}<span class="detail">{r.detail}</span>{/if}
            {#if r.keys}<kbd>{prettyKeys(r.keys)}</kbd>{/if}
          </button>
        {:else}
          <p class="empty">{t("Aucun résultat")}</p>
        {/each}
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
    background: rgb(20 15 30 / 18%);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 12vh;
  }
  .palette {
    width: min(640px, 92vw);
    background: var(--bg-raised);
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    box-shadow: var(--shadow);
    overflow: hidden;
    display: flex;
    flex-direction: column;
    max-height: 64vh;
  }
  input {
    border: 0;
    border-bottom: 1px solid var(--line);
    padding: 15px 18px;
    font-size: 15px;
    outline: none;
    background: transparent;
    user-select: text;
  }
  .list {
    overflow: auto;
    padding: 6px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    border: 0;
    background: transparent;
    text-align: left;
    padding: 8px 12px;
    border-radius: 6px;
  }
  .row.on {
    background: var(--accent-soft);
  }
  .label {
    flex-shrink: 0;
    max-width: 60%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .detail {
    flex: 1;
    color: var(--text-faint);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  kbd {
    margin-left: auto;
  }
</style>
