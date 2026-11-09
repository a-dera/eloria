<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, translateError } from "../lib/i18n.svelte";
  import type { ArgPoint, ArgumentMap } from "../lib/types";
  import { debounce } from "../lib/util";
  import Icon from "./Icon.svelte";

  // Argument map: claim, arguments and evidence, objections and answers,
  // conclusion. Every change rewrites the note (readable Markdown); cited keys
  // missing from the library are flagged.
  let { path, visible = true }: { path: string; visible?: boolean } = $props();
  let arg = $state<ArgumentMap | null>(null);
  let unknown = $state<string[]>([]);
  let saving = $state<"" | "pending" | "saved">("");
  let error = $state("");
  let loaded = "";

  async function load() {
    try {
      const [a, u] = await api.argumentLoad(path);
      arg = a;
      unknown = u;
      loaded = JSON.stringify(a);
    } catch (e) {
      error = translateError(e);
    }
  }
  $effect(() => {
    void path;
    load();
  });

  const save = debounce(async (snapshot: string) => {
    try {
      unknown = await api.argumentSave(path, JSON.parse(snapshot));
      loaded = snapshot;
      saving = "saved";
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }, 600);
  $effect(() => {
    if (!arg) return;
    const s = JSON.stringify(arg);
    if (s === loaded) return;
    saving = "pending";
    save(s);
  });

  const blank = (): ArgPoint => ({ title: "", text: "", evidence: [], rebuttal: "" });
  const bad = (s: string) => unknown.some((k) => s.includes(`@${k}`));

  async function exportAs(format: "md" | "tex") {
    try {
      const p = await api.argumentExport(path, format);
      await app.refresh();
      if (format === "tex") app.openView("latex", { path: p, title: p.split("/").pop() });
      else await app.openNote(p, { newTab: true });
      app.toast(format === "tex" ? t("Document LaTeX créé à partir de l'argumentation") : t("Texte suivi créé à partir de l'argumentation"), "success");
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }
</script>

<div class="argv">
  {#if error}<p class="error">{error}</p>{/if}
  {#if arg}
    <div class="top">
      <input class="title" bind:value={arg.title} aria-label={t("Titre de l'argumentation")} />
      <span class="state">{saving === "pending" ? t("Enregistrement…") : saving === "saved" ? t("Enregistrée") : ""}</span>
      <button class="btn small ghost" onclick={() => app.openNote(path, { newTab: true })}>{t("Ouvrir la note")}</button>
      <button class="btn small" onclick={() => exportAs("md")}>{t("Texte suivi")}</button>
      <button class="btn small" onclick={() => exportAs("tex")}>LaTeX</button>
    </div>
    {#if unknown.length}
      <p class="warn">{t("Absentes de la bibliothèque : {keys}. Ajoute-les ou corrige la clé : Eloria n'invente aucune référence.", { keys: unknown.map((k) => `@${k}`).join(", ") })}</p>
    {/if}

    <section class="node claim">
      <span class="lbl">{t("Affirmation")}</span>
      <textarea rows="2" bind:value={arg.claim} placeholder={t("Ce que tu défends, en une ou deux phrases.")} aria-label={t("Affirmation")}></textarea>
    </section>

    <div class="branch">
      <span class="lbl side">{t("Arguments")}</span>
      <div class="row">
        {#each arg.arguments as p, i (i)}
          <section class="node pro">
            <div class="nh">
              <span class="lbl">{t("Argument {n}", { n: i + 1 })}</span>
              <button class="icon-btn" aria-label={t("Retirer l'argument {n}", { n: i + 1 })} onclick={() => arg!.arguments.splice(i, 1)}><Icon name="x" size={13} /></button>
            </div>
            <input bind:value={p.title} placeholder={t("L'idée, en une ligne")} aria-label={t("Titre de l'argument {n}", { n: i + 1 })} />
            <textarea rows="2" bind:value={p.text} placeholder={t("Explication")} aria-label={t("Explication de l'argument {n}", { n: i + 1 })}></textarea>
            <span class="sub">{t("Preuves")}</span>
            {#each p.evidence as _e, j (j)}
              <div class="ev">
                <input class:bad={bad(p.evidence[j])} bind:value={p.evidence[j]} placeholder={t("Fait, chiffre, citation… [@clé, p. 4]")} aria-label={t("Preuve {n}", { n: j + 1 })} />
                <button class="icon-btn" aria-label={t("Retirer la preuve {n}", { n: j + 1 })} onclick={() => p.evidence.splice(j, 1)}><Icon name="x" size={12} /></button>
              </div>
            {/each}
            <button class="btn small ghost" onclick={() => p.evidence.push("")}>{t("+ Preuve")}</button>
          </section>
        {/each}
        <button class="add" onclick={() => arg!.arguments.push(blank())}>{t("+ Argument")}</button>
      </div>
    </div>

    <div class="branch">
      <span class="lbl side">{t("Objections")}</span>
      <div class="row">
        {#each arg.objections as p, i (i)}
          <section class="node con">
            <div class="nh">
              <span class="lbl">{t("Objection {n}", { n: i + 1 })}</span>
              <button class="icon-btn" aria-label={t("Retirer l'objection {n}", { n: i + 1 })} onclick={() => arg!.objections.splice(i, 1)}><Icon name="x" size={13} /></button>
            </div>
            <input bind:value={p.title} placeholder={t("Ce qu'on pourrait t'opposer")} aria-label={t("Titre de l'objection {n}", { n: i + 1 })} />
            <textarea rows="2" bind:value={p.text} placeholder={t("Détail")} aria-label={t("Détail de l'objection {n}", { n: i + 1 })}></textarea>
            <span class="sub">{t("Réponse")}</span>
            <textarea rows="2" class:bad={bad(p.rebuttal)} bind:value={p.rebuttal} placeholder={t("Ta réponse, avec ses sources")} aria-label={t("Réponse à l'objection {n}", { n: i + 1 })}></textarea>
          </section>
        {/each}
        <button class="add" onclick={() => arg!.objections.push(blank())}>{t("+ Objection")}</button>
      </div>
    </div>

    <section class="node concl">
      <span class="lbl">{t("Conclusion")}</span>
      <textarea rows="2" bind:value={arg.conclusion} placeholder={t("Ce qu'il faut retenir, et pour quel chapitre.")} aria-label={t("Conclusion")}></textarea>
    </section>
  {/if}
</div>

<style>
  .argv {
    height: 100%;
    overflow: auto;
    padding: 18px 26px 30px;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 0;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 10px;
  }
  .title {
    flex: 1;
    min-width: 0;
    border: 0;
    background: none;
    font: 700 20px var(--font-ui);
    color: var(--text);
    padding: 2px 0;
  }
  .title:focus {
    outline: none;
    box-shadow: 0 1px 0 var(--accent);
  }
  .state {
    font-size: 12px;
    color: var(--text-faint);
    white-space: nowrap;
  }
  .warn {
    margin: 0 0 10px;
    font-size: 12.5px;
    color: var(--danger);
  }
  .node {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--bg);
    min-width: 0;
  }
  .claim {
    border-color: var(--accent);
    background: var(--accent-soft);
    max-width: 720px;
    width: 100%;
    align-self: center;
  }
  .concl {
    max-width: 720px;
    width: 100%;
    align-self: center;
    border-color: var(--line-strong);
    margin-top: 6px;
  }
  .con {
    border-left: 3px solid var(--warning);
  }
  .pro {
    border-left: 3px solid var(--accent);
  }
  .branch {
    position: relative;
    padding: 18px 0 8px;
  }
  /* Line connecting the claim to the branches. */
  .branch::before {
    content: "";
    position: absolute;
    left: 50%;
    top: 0;
    height: 18px;
    border-left: 2px solid var(--line-strong);
  }
  .row {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 10px;
    border-top: 2px solid var(--line-strong);
    padding-top: 12px;
  }
  .nh {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .lbl {
    font-size: 11.5px;
    font-weight: 700;
    color: var(--text-soft);
  }
  .lbl.side {
    display: block;
    margin-bottom: 6px;
    text-align: center;
  }
  .sub {
    font-size: 11.5px;
    color: var(--text-faint);
    margin-top: 2px;
  }
  input,
  textarea {
    width: 100%;
    border: 1px solid var(--line);
    border-radius: var(--radius-s);
    background: var(--bg-raised);
    color: var(--text);
    font: 13px var(--font-ui);
    padding: 5px 7px;
    resize: vertical;
    min-width: 0;
  }
  input:focus,
  textarea:focus {
    outline: none;
    border-color: var(--accent);
  }
  .bad {
    border-color: var(--danger);
  }
  .ev {
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .add {
    min-height: 80px;
    border: 1px dashed var(--line-strong);
    border-radius: var(--radius-l);
    background: none;
    color: var(--text-soft);
    font: 600 13px var(--font-ui);
    cursor: pointer;
  }
  .add:hover {
    border-color: var(--accent);
    color: var(--text);
  }
  .error {
    color: var(--danger);
  }
</style>
