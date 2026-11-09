<script lang="ts">
  // Settings › Customization: typed properties, saved views, commands.
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, tk } from "../lib/i18n.svelte";
  import { stem } from "../lib/util";
  import type { CustomCommand, CustomKind, PropDef, PropKind, SavedView, Settings } from "../lib/types";
  import Icon from "./Icon.svelte";

  const s = $derived(app.settings!);
  const set = (patch: Partial<Settings>) => app.updateSettings(patch);
  const patchAt = <T,>(list: T[], i: number, p: Partial<T>): T[] => list.map((x, j) => (j === i ? { ...x, ...p } : x));
  const without = <T,>(list: T[], i: number): T[] => list.filter((_, j) => j !== i);
  const val = (e: Event) => (e.target as HTMLInputElement).value;

  const KINDS: [PropKind, string][] = [
    ["texte", tk("Texte")],
    ["nombre", tk("Nombre")],
    ["date", tk("Date")],
    ["case", tk("Case à cocher")],
    ["choix", tk("Choix dans une liste")],
    ["liste", tk("Plusieurs valeurs")],
    ["lien", tk("Lien vers une note")],
  ];
  const CMD_KINDS: [CustomKind, string, string][] = [
    ["modele", tk("Nouvelle note depuis un modèle"), tk("Nom du modèle")],
    ["texte", tk("Insérer un texte"), tk("Texte inséré ({{date}} et {{heure}} sont remplacés)")],
    ["vue", tk("Ouvrir une vue enregistrée"), tk("Nom de la vue")],
    ["note", tk("Ouvrir une note"), tk("Nom ou chemin de la note")],
    ["recherche", tk("Lancer une recherche"), tk("Texte recherché")],
  ];

  let templates = $state<string[]>([]);
  api.templates().then((l) => (templates = l.map(stem))).catch(() => {});

  // Typed properties
  const defs = $derived(s.property_defs ?? []);
  const setDefs = (property_defs: PropDef[]) => set({ property_defs });
  const known = $derived([...new Set(app.notes.flatMap((n) => Object.keys(n.frontmatter)))].filter((k) => !defs.some((d) => d.name === k)).sort());

  // Vues
  const views = $derived(s.saved_views ?? []);
  const viewSummary = (v: SavedView) =>
    [v.type_filter && t("type {x}", { x: v.type_filter }), v.folder && t("dossier {x}", { x: v.folder }), v.tag && `#${v.tag.replace(/^#/, "")}`, v.text && `« ${v.text} »`, v.sort_key && t("tri : {x}", { x: v.sort_key + (v.sort_dir < 0 ? " ↓" : " ↑") })]
      .filter(Boolean)
      .join(" · ") || t("toutes les notes");

  // Commandes
  const cmds = $derived(s.custom_commands ?? []);
  const setCmds = (custom_commands: CustomCommand[]) => set({ custom_commands });

</script>

<div class="cz">
  <div class="block" data-block="proprietes">
    <h3>{t("Propriétés typées")}</h3>
    <p class="hint">{t("Une propriété typée a un éditeur adapté dans le panneau Propriétés et dans la base de notes : case à cocher, date, liste de choix, lien vers une note.")}</p>
    {#each defs as d, i (i)}
      <div class="line">
        <input class="input name" list="cz-known" value={d.name} placeholder={t("Nom (ex. statut)")} aria-label={t("Nom de la propriété")} onchange={(e) => setDefs(patchAt(defs, i, { name: val(e).trim().replace(/\s+/g, "_") }))} />
        <select class="input" value={d.kind} aria-label={t("Type de la propriété")} onchange={(e) => setDefs(patchAt(defs, i, { kind: val(e) as PropKind }))}>
          {#each KINDS as [k, l] (k)}<option value={k}>{t(l)}</option>{/each}
        </select>
        {#if d.kind === "choix"}
          <input class="input grow" value={d.options.join(", ")} placeholder={t("Valeurs séparées par des virgules")} aria-label={t("Valeurs proposées")} onchange={(e) => setDefs(patchAt(defs, i, { options: val(e).split(",").map((x) => x.trim()).filter(Boolean) }))} />
        {:else}<span class="grow"></span>{/if}
        <button class="icon-btn" title={t("Supprimer")} aria-label={t("Supprimer {name}", { name: d.name || t("propriété") })} onclick={() => setDefs(without(defs, i))}><Icon name="x" size={14} /></button>
      </div>
    {/each}
    <datalist id="cz-known">{#each known as k (k)}<option value={k}></option>{/each}</datalist>
    <button class="btn small" onclick={() => setDefs([...defs, { name: "", kind: "texte", options: [] }])}><Icon name="plus" size={13} /> {t("Ajouter une propriété")}</button>
  </div>

  <div class="block" data-block="vues">
    <h3>{t("Vues enregistrées")}</h3>
    <p class="hint">{t("Une vue garde les filtres, le tri et les colonnes de la base de notes. Crée-la depuis la base de notes avec « Enregistrer la vue » ; elle s'ouvre ensuite depuis la palette.")}</p>
    {#each views as v, i (i)}
      <div class="line">
        <b class="vname">{v.name}</b>
        <span class="faint grow ellipsis">{viewSummary(v)}</span>
        <button class="btn small ghost" onclick={() => { app.settingsOpen = false; app.openView("table", { key: v.name, title: v.name }); }}>{t("Ouvrir")}</button>
        <button class="icon-btn" title={t("Supprimer")} aria-label={t("Supprimer {name}", { name: v.name })} onclick={() => set({ saved_views: without(views, i) })}><Icon name="x" size={14} /></button>
      </div>
    {:else}
      <p class="faint small">{t("Aucune vue pour l'instant.")}</p>
    {/each}
    <button class="btn small" onclick={() => { app.settingsOpen = false; app.openView("table"); }}><Icon name="table" size={13} /> {t("Ouvrir la base de notes")}</button>
  </div>

  <div class="block" data-block="commandes">
    <h3>{t("Commandes personnalisées")}</h3>
    <p class="hint">{t("Elles apparaissent dans la palette (Ctrl+P), groupe « Personnalisées ». Un raccourci se choisit ensuite dans Réglages › Raccourcis.")}</p>
    {#each cmds as c, i (i)}
      {@const kind = CMD_KINDS.find((x) => x[0] === c.kind) ?? CMD_KINDS[0]}
      <div class="line wrap">
        <input class="input name" value={c.name} placeholder={t("Nom dans la palette")} aria-label={t("Nom de la commande")} onchange={(e) => setCmds(patchAt(cmds, i, { name: val(e).trim() }))} />
        <select class="input" value={c.kind} aria-label={t("Action")} onchange={(e) => setCmds(patchAt(cmds, i, { kind: val(e) as CustomKind }))}>
          {#each CMD_KINDS as [k, l] (k)}<option value={k}>{t(l)}</option>{/each}
        </select>
        <input class="input grow" list={c.kind === "modele" ? "cz-templates" : c.kind === "vue" ? "cz-views" : undefined} value={c.arg} placeholder={t(kind[2])} aria-label={t(kind[2])} onchange={(e) => setCmds(patchAt(cmds, i, { arg: val(e) }))} />
        {#if c.kind === "modele"}
          <input class="input folder" value={c.folder} placeholder={t("Dossier (facultatif)")} aria-label={t("Dossier de la note créée")} onchange={(e) => setCmds(patchAt(cmds, i, { folder: val(e).trim() }))} />
        {/if}
        <button class="icon-btn" title={t("Supprimer")} aria-label={t("Supprimer {name}", { name: c.name || t("commande") })} onclick={() => setCmds(without(cmds, i))}><Icon name="x" size={14} /></button>
      </div>
    {/each}
    <datalist id="cz-templates">{#each templates as tp (tp)}<option value={tp}></option>{/each}</datalist>
    <datalist id="cz-views">{#each views as v (v.name)}<option value={v.name}></option>{/each}</datalist>
    <button class="btn small" onclick={() => setCmds([...cmds, { name: "", kind: "modele", arg: "", folder: "" }])}><Icon name="plus" size={13} /> {t("Ajouter une commande")}</button>
  </div>

</div>

<style>
  .cz {
    display: flex;
    flex-direction: column;
    gap: 26px;
  }
  .block {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-start;
  }
  .block > .line {
    align-self: stretch;
  }
  h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }
  .hint {
    margin: 0 0 4px;
    color: var(--text-soft);
    font-size: 12.5px;
    line-height: 1.5;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .line.wrap {
    flex-wrap: wrap;
  }
  .cz .input {
    height: 30px;
    font-size: 12.5px;
    width: auto;
    flex: 0 0 auto;
  }
  .cz select.input {
    width: 200px;
  }
  .cz .name {
    width: 180px;
  }
  .cz .folder {
    width: 150px;
  }
  .cz .grow {
    flex: 1 1 140px;
    min-width: 0;
  }
  .ellipsis {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .vname {
    font-weight: 600;
    font-size: 13px;
  }
  .faint {
    color: var(--text-faint);
  }
  .small {
    font-size: 12px;
    margin: 0;
  }
</style>
