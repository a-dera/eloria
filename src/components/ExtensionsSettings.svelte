<script lang="ts">
  // Settings › Extensions: approval per vault, creation, access to the console.
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, translateError } from "../lib/i18n.svelte";
  import { extState, loadExtension, loadExtensions, unloadExtension } from "../lib/extensions.svelte";
  import type { ExtensionInfo } from "../lib/types";
  import Icon from "./Icon.svelte";

  let list = $state<ExtensionInfo[] | null>(null);
  let busy = $state("");

  async function refresh() {
    try {
      list = await api.extensionsList();
    } catch (e) {
      list = [];
      app.toast(translateError(e), "error");
    }
  }
  refresh();

  const loaded = (id: string) => extState.loaded.find((x) => x.id === id);

  async function toggle(x: ExtensionInfo) {
    const on = !x.enabled;
    if (on) {
      const ok = await app.confirm(
        t("Activer « {name} » ?", { name: x.name }),
        t("Une extension exécute du code JavaScript avec accès à tes notes et à l'application. N'active que celles dont tu as lu le code ou dont tu connais l'auteur. Si le code change, l'extension se désactive jusqu'à ta nouvelle approbation."),
        false,
        t("Activer"),
      );
      if (!ok) return;
    }
    busy = x.id;
    try {
      list = await api.extensionEnable(x.id, on);
      // Approvals are kept by the core: the settings are read again.
      app.settings = await api.getSettings();
      if (on) {
        await loadExtension(x.id);
        const l = loaded(x.id);
        app.toast(l?.error ? t("« {name} » activée, mais son chargement a échoué : {err}", { name: x.name, err: l.error }) : t("« {name} » activée", { name: x.name }), l?.error ? "error" : "success");
      } else {
        unloadExtension(x.id);
        app.toast(t("« {name} » désactivée", { name: x.name }));
      }
    } catch (e) {
      app.toast(translateError(e), "error");
    } finally {
      busy = "";
    }
  }

  async function create() {
    const name = await app.ask(t("Nom de la nouvelle extension"), "");
    if (!name?.trim()) return;
    try {
      const dir = await api.extensionCreate(name.trim());
      await refresh();
      app.toast(t("Extension créée dans {dir}. Lis et modifie main.js, puis active-la.", { dir }), "success");
      api.revealPath(`${dir}/main.js`).catch(() => {});
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }

  async function openFolder() {
    try {
      await api.revealPath(".eloria/extensions", true);
    } catch {
      app.toast(t("Pas encore de dossier d'extensions : crée d'abord une extension."));
    }
  }

  function openConsole() {
    app.settingsOpen = false;
    app.openView("console");
  }
</script>

<div class="ex">
  <p class="hint">
    {t("Une extension est un dossier .eloria/extensions/<id>/ du coffre avec manifest.json et main.js. Elle ajoute des commandes à la palette, des éléments à la barre d'état et réagit aux notes ouvertes. Elle n'est exécutée que si tu l'actives pour ce coffre.")}
  </p>
  <div class="actions">
    <button class="btn small" onclick={create}><Icon name="plus" size={13} /> {t("Créer une extension…")}</button>
    <button class="btn small" onclick={openFolder}><Icon name="folder-open" size={13} /> {t("Ouvrir le dossier des extensions")}</button>
    <button class="btn small" onclick={openConsole}><Icon name="code" size={13} /> {t("Console développeur")}</button>
    <button class="btn small ghost" onclick={async () => { await refresh(); await loadExtensions(); }} disabled={extState.busy}><Icon name="sync" size={13} /> {t("Recharger")}</button>
  </div>
  {#if list === null}
    <p class="faint">{t("Lecture des extensions…")}</p>
  {:else if !list.length}
    <p class="faint">{t("Aucune extension dans ce coffre. La documentation (docs/extensions.md) décrit l'objet eloria et donne un exemple complet.")}</p>
  {:else}
    <ul>
      {#each list as x (x.id)}
        {@const l = loaded(x.id)}
        <li class:on={x.enabled}>
          <div class="head">
            <b>{x.name}</b>
            {#if x.version}<span class="faint">{x.version}</span>{/if}
            {#if x.author}<span class="faint">· {x.author}</span>{/if}
            <span class="grow"></span>
            {#if x.error}
              <span class="state bad">{t("Invalide")}</span>
            {:else if x.changed}
              <span class="state warn">{t("Code modifié : à réactiver")}</span>
            {:else if x.enabled && l?.error}
              <span class="state bad">{t("Erreur au chargement")}</span>
            {:else if x.enabled}
              <span class="state ok">{t("Active")}</span>
            {/if}
            <button class="icon-btn" title={t("Afficher le dossier")} aria-label={t("Afficher le dossier de {name}", { name: x.name })} onclick={() => api.revealPath(x.dir).catch((e) => app.toast(translateError(e), "error"))}><Icon name="folder" size={14} /></button>
            {#if !x.error}
              <button class="btn small" class:primary={!x.enabled} disabled={busy === x.id} onclick={() => toggle(x)}>
                {x.enabled ? t("Désactiver") : x.changed ? t("Réactiver") : t("Activer")}
              </button>
            {/if}
          </div>
          {#if x.description}<p class="desc">{x.description}</p>{/if}
          {#if x.error}<p class="err">{x.error}</p>{/if}
          {#if x.enabled && l?.error}<p class="err">{l.error}</p>{/if}
          <p class="meta faint">{x.dir}{x.hash ? ` · SHA-256 ${x.hash.slice(0, 12)}…` : ""}</p>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .ex {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .hint {
    margin: 0;
    color: var(--text-soft);
    font-size: 12.5px;
    line-height: 1.5;
  }
  .actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  li {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 10px 12px;
  }
  li.on {
    border-color: var(--accent);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }
  .grow {
    flex: 1;
  }
  .faint {
    color: var(--text-faint);
    font-size: 12px;
    margin: 0;
  }
  .desc {
    margin: 6px 0 0;
    font-size: 12.5px;
    color: var(--text-soft);
  }
  .err {
    margin: 6px 0 0;
    font-size: 12px;
    color: var(--danger);
  }
  .meta {
    margin-top: 6px;
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .state {
    font-size: 11.5px;
    padding: 1px 8px;
    border-radius: 99px;
  }
  .state.ok {
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  .state.warn {
    color: var(--warning);
    border: 1px solid currentColor;
  }
  .state.bad {
    color: var(--danger);
    border: 1px solid currentColor;
  }
</style>
