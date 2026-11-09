<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, tn, translateError } from "../lib/i18n.svelte";
  import Modal from "./Modal.svelte";

  // Outline generator: choose the scope (chapter, tag, folder), then a draft
  // outline is written in "Thèse/Plans" from the notes, without AI.
  const init = app.outlineFor!;
  let scope = $state<"chapter" | "tag" | "folder">(init.scope);
  let value = $state(init.value);
  let folders = $state<string[]>([]);
  let busy = $state(false);
  api.folders().then((f) => (folders = f)).catch(() => {});

  const chapters = $derived(
    app.notes
      .filter((n) => n.frontmatter.type === "chapitre" && n.frontmatter.chapitre)
      .map((n) => ({ id: String(n.frontmatter.chapitre).toLowerCase(), title: n.title }))
      .sort((a, b) => a.id.localeCompare(b.id, "fr", { numeric: true })),
  );
  const tags = $derived.by(() => {
    const m = new Map<string, number>();
    for (const n of app.notes) for (const tg of n.tags) m.set(tg, (m.get(tg) ?? 0) + 1);
    return [...m.entries()].filter(([tg]) => !["source", "fiche"].includes(tg)).sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
  });
  $effect(() => {
    // Default value when the scope changes.
    const s = scope;
    if (s === "chapter" && !chapters.some((c) => c.id === value)) value = chapters[0]?.id ?? "";
    if (s === "tag" && !tags.some(([tg]) => tg === value)) value = tags[0]?.[0] ?? "";
    if (s === "folder" && !folders.includes(value)) value = folders[0] ?? "";
  });
  const count = $derived.by(() => {
    const v = value.toLowerCase();
    if (!v) return 0;
    return app.notes.filter((n) => {
      if (n.frontmatter.type === "chapitre") return false;
      if (scope === "chapter") return (Array.isArray(n.frontmatter.chapitres) ? n.frontmatter.chapitres : []).map((x) => String(x).toLowerCase()).includes(v);
      if (scope === "tag") return n.tags.some((tg) => tg.toLowerCase() === v || tg.toLowerCase().startsWith(`${v}/`));
      return n.path.toLowerCase().startsWith(`${v}/`);
    }).length;
  });

  async function generate() {
    busy = true;
    try {
      const p = await api.generateOutline(scope, value);
      await app.refresh();
      app.outlineFor = null;
      await app.openNote(p, { newTab: true });
      app.toast(t("Brouillon de plan créé : réordonne et renomme les sections"), "success");
    } catch (e) {
      app.toast(translateError(e), "error");
    } finally {
      busy = false;
    }
  }
</script>

<Modal title={t("Générer un plan")} width={520} onclose={() => (app.outlineFor = null)}>
  <div class="body">
    <p class="hint">{t("Les notes du périmètre sont regroupées par leurs tags les plus partagés ; chaque point renvoie à sa note, avec sa phrase clé et sa citation. Rien n'est inventé.")}</p>
    <div class="scopes" role="radiogroup" aria-label={t("Périmètre")}>
      <label class:on={scope === "chapter"}><input type="radio" bind:group={scope} value="chapter" /> {t("Chapitre")}</label>
      <label class:on={scope === "tag"}><input type="radio" bind:group={scope} value="tag" /> {t("Tag")}</label>
      <label class:on={scope === "folder"}><input type="radio" bind:group={scope} value="folder" /> {t("Dossier")}</label>
    </div>
    <select class="input" bind:value aria-label={t("Valeur du périmètre")}>
      {#if scope === "chapter"}
        {#each chapters as c (c.id)}<option value={c.id}>{c.id.toUpperCase()} · {c.title}</option>{/each}
      {:else if scope === "tag"}
        {#each tags as [tg, n] (tg)}<option value={tg}>#{tg} ({n})</option>{/each}
      {:else}
        {#each folders as f (f)}<option value={f}>{f}</option>{/each}
      {/if}
    </select>
    <p class="count">{tn(count, "{n} note dans ce périmètre", "{n} notes dans ce périmètre")}</p>
  </div>
  {#snippet footer()}
    <button class="btn" onclick={() => (app.outlineFor = null)}>{t("Annuler")}</button>
    <button class="btn primary" disabled={busy || !value} onclick={generate}>{t("Générer le plan")}</button>
  {/snippet}
</Modal>

<style>
  .body {
    padding: 14px 22px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .hint {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-soft);
  }
  .scopes {
    display: flex;
    gap: 6px;
  }
  .scopes label {
    padding: 3px 12px;
    border: 1px solid var(--line);
    border-radius: 999px;
    font-size: 13px;
    color: var(--text-soft);
    cursor: pointer;
  }
  .scopes label.on {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--text);
  }
  .scopes input {
    display: none;
  }
  .count {
    margin: 0;
    font-size: 12px;
    color: var(--text-faint);
  }
</style>
