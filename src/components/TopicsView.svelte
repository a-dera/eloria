<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, tn, translateError } from "../lib/i18n.svelte";
  import type { Topic } from "../lib/types";

  // Topic clustering: notes grouped by their vocabulary (TF-IDF,
  // k-means), each group named after its most characteristic words. No AI.
  let { visible = true }: { visible?: boolean } = $props();
  let k = $state(0);
  let topics = $state<Topic[] | null>(null);
  let busy = $state(false);
  let error = $state("");
  let open = $state<Record<number, boolean>>({});

  async function load() {
    busy = true;
    try {
      topics = await api.topics(k);
      error = "";
    } catch (e) {
      error = translateError(e);
    } finally {
      busy = false;
    }
  }
  $effect(() => {
    void k;
    if (visible) load();
  });

  async function tagAll(tp: Topic) {
    const tag = await app.ask(t("Tag à ajouter aux {n} notes de ce thème", { n: tp.notes.length }), tp.tag);
    if (!tag?.trim()) return;
    let changed = 0;
    for (const [p] of tp.notes) {
      try {
        if (await api.addTags(p, [tag.trim().replace(/^#/, "")])) changed++;
      } catch {
        /* unreadable note: skipped */
      }
    }
    await app.refresh();
    app.toast(tn(changed, "#{tag} ajouté à {n} note", "#{tag} ajouté à {n} notes", { tag: tag.trim().replace(/^#/, "") }), "success");
  }
</script>

<div class="tv">
  <div class="top">
    <h2>{t("Thèmes")}</h2>
    <span class="grow"></span>
    <label class="kl">{t("Nombre de thèmes")}
      <select class="input" bind:value={k} aria-label={t("Nombre de thèmes")}>
        <option value={0}>{t("automatique")}</option>
        {#each [2, 3, 4, 5, 6, 8, 10, 12] as x (x)}<option value={x}>{x}</option>{/each}
      </select>
    </label>
  </div>
  <p class="hint">{t("Tes notes regroupées par leur vocabulaire (TF-IDF et k-moyennes, sur ton ordinateur, sans IA). Chaque thème est nommé par ses mots les plus caractéristiques ; les notes les plus centrales viennent d'abord.")}</p>
  {#if error}<p class="error">{error}</p>{/if}
  {#if busy && !topics}<p class="loading" role="status">{t("Analyse de tes notes…")}</p>{/if}
  {#if topics}
    {#if !topics.length}<p class="empty">{t("Pas assez de notes rédigées pour dégager des thèmes.")}</p>{/if}
    <div class="grid">
      {#each topics as tp, i (i)}
        <section class="card">
          <div class="th">
            <h3>{tp.terms.join(" · ")}</h3>
            <span class="faint">{tn(tp.notes.length, "{n} note", "{n} notes")}</span>
          </div>
          <ul>
            {#each open[i] ? tp.notes : tp.notes.slice(0, 8) as [p, title, score] (p)}
              <li>
                <button class="n" onclick={() => app.openNote(p, { newTab: true })} title={p}>
                  <span class="nt">{title}</span>
                  <span class="sc" style:width="{Math.round(score * 40)}px" title={t("Proximité au centre du thème : {n} %", { n: Math.round(score * 100) })}></span>
                </button>
              </li>
            {/each}
          </ul>
          {#if tp.notes.length > 8 && !open[i]}<button class="btn small ghost" onclick={() => (open[i] = true)}>{t("Voir les {n} autres", { n: tp.notes.length - 8 })}</button>{/if}
          <button class="btn small" onclick={() => tagAll(tp)}>{t("Taguer ces notes")}</button>
        </section>
      {/each}
    </div>
  {/if}
</div>

<style>
  .tv {
    height: 100%;
    overflow: auto;
    padding: 20px 26px 30px;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .grow {
    flex: 1;
  }
  h2 {
    margin: 0;
    font-size: 20px;
  }
  .kl {
    display: flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
    font-size: 12.5px;
    color: var(--text-soft);
  }
  .hint {
    margin: 6px 0 14px;
    font-size: 12.5px;
    color: var(--text-soft);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 12px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--bg);
    min-width: 0;
  }
  .th {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  h3 {
    margin: 0;
    font-size: 14px;
    flex: 1;
    min-width: 0;
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .n {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    border: 0;
    background: none;
    padding: 3px 0;
    text-align: left;
    font: 13px var(--font-ui);
    color: var(--text);
    cursor: pointer;
    min-width: 0;
  }
  .n:hover .nt {
    color: var(--accent-text);
  }
  .nt {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sc {
    height: 4px;
    border-radius: 2px;
    background: var(--accent);
    opacity: 0.7;
    flex-shrink: 0;
  }
  .card > .btn {
    align-self: flex-start;
  }
  .faint {
    color: var(--text-faint);
    font-size: 12px;
  }
  .empty {
    color: var(--text-soft);
  }
  .error {
    color: var(--danger);
  }
</style>
