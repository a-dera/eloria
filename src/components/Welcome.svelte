<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api, pickFolder } from "../lib/api";
  import { i18n, LANGS, t, translateError, type Lang } from "../lib/i18n.svelte";
  import { applySettings } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";

  let busy = $state(false);
  let error = $state("");

  async function open(path: string | null) {
    if (!path) return;
    busy = true;
    error = "";
    try {
      await app.openVault(path);
    } catch (e) {
      error = translateError(e);
    } finally {
      busy = false;
    }
  }

  // The language is chosen right on the welcome screen (even before opening a vault).
  async function setLang(l: Lang) {
    if (!app.settings) return;
    app.settings = { ...app.settings, language: l };
    applySettings(app.settings);
    await api.saveSettings($state.snapshot(app.settings));
  }
</script>

<div class="welcome">
  <div class="card">
    <img class="mark" src="/logo.svg" alt="" width="64" height="64" />
    <h1>ELORIA</h1>
    <p class="slogan">{t("Tes connaissances, magnifiquement reliées.")}</p>
    <p class="lede">{t("Un espace de travail personnel pour la recherche, l'écriture et le savoir.")}</p>
    <p class="intro">{t("Ouvre un dossier de notes Markdown (un « coffre ») ou crée-en un. Ta bibliothèque Zotero s'y connecte ensuite.")}</p>

    <div class="actions">
      <button class="btn primary" disabled={busy} onclick={async () => open(await pickFolder(t("Ouvrir un dossier comme coffre")))}>
        <Icon name="folder-open" /> {t("Ouvrir un dossier")}
      </button>
      <button class="btn" disabled={busy} onclick={async () => open(await pickFolder(t("Choisir où créer le coffre")))}>
        <Icon name="folder-plus" /> {t("Créer un coffre")}
      </button>
    </div>
    <p class="hint">{t("Un dossier Obsidian existant fonctionne tel quel.")}</p>

    {#if error}<p class="error">{error}</p>{/if}

    {#if app.settings?.recent_vaults.length}
      <h2>{t("Coffres récents")}</h2>
      <ul>
        {#each app.settings.recent_vaults as v (v)}
          <li>
            <button onclick={() => open(v)} disabled={busy}>
              <Icon name="vault" size={16} />
              <span class="name">{v.split(/[\\/]/).pop()}</span>
              <span class="path">{v}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}

    <div class="langs" role="group" aria-label={t("Langue")}>
      {#each LANGS as l (l.id)}
        <button class:on={i18n.lang === l.id} onclick={() => setLang(l.id)}>{l.label}</button>
      {/each}
    </div>
  </div>
</div>

<style>
  .langs {
    display: flex;
    gap: 4px;
    margin-top: 28px;
  }
  .langs button {
    border: 0;
    background: transparent;
    color: var(--text-faint);
    font-size: 12.5px;
    padding: 3px 8px;
    border-radius: var(--radius-s);
    cursor: pointer;
  }
  .langs button:hover {
    color: var(--text);
  }
  .langs button.on {
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .welcome {
    height: 100%;
    display: grid;
    place-items: center;
    background:
      radial-gradient(1200px 600px at 10% -10%, var(--accent-soft), transparent 60%),
      var(--bg);
  }
  .card {
    width: min(520px, 92vw);
  }
  .mark {
    display: block;
    width: 64px;
    height: 64px;
    margin: 0 0 22px -6px;
  }
  h1 {
    font-family: var(--font-ui);
    font-size: 40px;
    font-weight: 700;
    letter-spacing: 0.22em;
    margin: 0 0 14px;
  }
  .slogan {
    font-family: var(--font-editor);
    font-size: 24px;
    line-height: 1.3;
    margin: 0 0 6px;
    color: var(--text);
  }
  .lede {
    font-family: var(--font-editor);
    font-size: 16px;
    line-height: 1.6;
    color: var(--text-soft);
    margin: 0 0 22px;
  }
  .intro {
    color: var(--text-soft);
    margin: 0 0 18px;
    max-width: 52ch;
    line-height: 1.55;
  }
  .actions {
    display: flex;
    gap: 10px;
  }
  .actions .btn {
    height: 38px;
    padding: 0 16px;
  }
  .hint {
    color: var(--text-faint);
    font-size: 12.5px;
    margin: 12px 0 0;
  }
  .error {
    color: var(--danger);
    margin-top: 14px;
  }
  h2 {
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text-soft);
    margin: 36px 0 8px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    border-top: 1px solid var(--line);
  }
  li button {
    width: 100%;
    display: grid;
    grid-template-columns: auto auto 1fr;
    align-items: center;
    gap: 10px;
    padding: 10px 4px;
    border: 0;
    border-bottom: 1px solid var(--line);
    background: transparent;
    text-align: left;
  }
  li button:hover .name {
    color: var(--accent-text);
  }
  .name {
    font-weight: 600;
  }
  .path {
    color: var(--text-faint);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }
</style>
