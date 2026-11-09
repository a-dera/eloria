<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api, pickFile, pickFolder } from "../lib/api";
  import { LANGS, locale, t, tk, translateError } from "../lib/i18n.svelte";
  import { CUSTOM_PREFIX, THEMES, previewColors } from "../lib/themes";
  import { commands, comboFromEvent, keyConflicts, keysOf, labelOf as cmdLabel, prettyKeys } from "../lib/commands";
  import type { BackupInfo, Settings } from "../lib/types";
  import Modal from "./Modal.svelte";
  import Icon from "./Icon.svelte";
  import CustomizeSettings from "./CustomizeSettings.svelte";
  import ExtensionsSettings from "./ExtensionsSettings.svelte";

  type Section = "general" | "apparence" | "editeur" | "personnalisation" | "extensions" | "zotero" | "ia" | "fichiers" | "export" | "raccourcis" | "apropos";

  let section = $state<Section>((app.settingsSection as Section) || "general");
  let pandoc = $state<string | null | undefined>(undefined);
  /** Local API: web capture bookmarklet (the token is created by the core on activation). */
  let bookmarklet = $state<string | null>(null);
  if (app.settings?.local_api) api.localApiEnable(true).then((b) => (bookmarklet = b)).catch(() => {});
  async function toggleApi() {
    const on = !s.local_api;
    bookmarklet = await api.localApiEnable(on);
    // Settings read again: the token comes from the core, the interface must not overwrite it.
    app.settings = await api.getSettings();
  }
  /** MCP server: configuration for Claude Desktop (Settings › AI). */
  let mcp = $state<{ exe: string; exists: boolean; config: string } | null>(null);
  api.mcpInfo().then((m) => (mcp = m)).catch(() => (mcp = null));
  /** Git versions of the vault (Settings › Files). */
  let git = $state<import("../lib/types").GitStatus | null>(null);
  api.gitStatus().then((g) => (git = g)).catch(() => (git = null));
  /** LaTeX engine (Settings › Export). */
  let texEngine = $state<import("../lib/types").TexEngine | null | undefined>(undefined);
  let installingTex = $state(false);
  function loadTex() {
    texEngine = undefined;
    api.latexEngine().then((e) => (texEngine = e)).catch(() => (texEngine = null));
  }
  loadTex();
  async function installTex() {
    installingTex = true;
    try {
      texEngine = await api.latexInstallTectonic();
    } catch (e) {
      app.toast(translateError(e), "error");
    } finally {
      installingTex = false;
    }
  }
  /** Vault backups (Settings › Files). */
  let backups = $state<BackupInfo[]>([]);
  let restored = $state("");
  function loadBackups() {
    api.backupList().then((l) => (backups = l)).catch(() => (backups = []));
  }
  loadBackups();
  async function restore(archive: string) {
    const dir = await pickFolder(t("Dossier vide où restaurer la sauvegarde"));
    if (!dir) return;
    try {
      const n = await api.backupRestore(archive, dir);
      restored = t("{n} fichiers restaurés dans {dir}. Ouvre ce dossier comme coffre pour le consulter.", { n, dir });
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }

  const s = $derived(app.settings!);
  const set = (patch: Partial<Settings>) => app.updateSettings(patch);

  const SECTIONS: [Section, string][] = [
    ["general", tk("Général")],
    ["apparence", tk("Apparence")],
    ["editeur", tk("Éditeur")],
    ["personnalisation", tk("Personnalisation")],
    ["extensions", tk("Extensions")],
    ["zotero", tk("Zotero")],
    ["ia", tk("Intelligence artificielle")],
    ["fichiers", tk("Fichiers et dossiers")],
    ["export", tk("Export")],
    ["raccourcis", tk("Raccourcis")],
    ["apropos", tk("À propos")],
  ];

  /** Grouped sections, as in large editors (Obsidian, VS Code). */
  const GROUPS: { label: string; items: Section[] }[] = [
    { label: tk("Eloria"), items: ["general", "apparence", "editeur", "personnalisation", "raccourcis", "extensions"] },
    { label: tk("Bibliothèque et IA"), items: ["zotero", "ia"] },
    { label: tk("Coffre"), items: ["fichiers", "export"] },
    { label: tk("Informations"), items: ["apropos"] },
  ];
  const ICONS: Record<Section, string> = {
    general: "settings", apparence: "sun", editeur: "edit", personnalisation: "layout", extensions: "code", raccourcis: "command",
    zotero: "book", ia: "zap", fichiers: "folder", export: "download", apropos: "vault",
  };

  // Search in the settings: every section is shown, only the settings
  // whose name or explanation contains the text stay visible.
  let query = $state("");
  let paneEl = $state<HTMLDivElement>();
  let noMatch = $state(false);
  const show = (id: Section) => (query.trim() ? true : section === id);
  const fold = (x: string) => x.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
  $effect(() => {
    const q = fold(query.trim());
    void section;
    if (!paneEl) return;
    requestAnimationFrame(() => {
      if (!paneEl) return;
      let any = false;
      for (const sec of paneEl.querySelectorAll<HTMLElement>("section.sec")) {
        let visible = 0;
        for (const f of sec.querySelectorAll<HTMLElement>(":scope > .field, :scope > label.check, :scope > .row, :scope > p.hint, :scope > table, :scope > div:not(.field)")) {
          const hit = !q || fold(f.textContent ?? "").includes(q);
          f.style.display = hit ? "" : "none";
          if (hit) visible++;
        }
        sec.style.display = visible || !q ? "" : "none";
        if (visible) any = true;
      }
      noMatch = !!q && !any;
    });
  });

  /** Choosing a gallery theme also applies its accent colour (editable afterwards). */
  function pickTheme(id: string, accent?: string) {
    set(accent ? { theme: id, accent } : { theme: id });
  }

  async function newCustomTheme() {
    try {
      const rel = await api.createExampleTheme();
      await app.loadCustomThemes();
      app.toast(t("Thème d'exemple créé : {path}", { path: rel }), "success");
      api.revealPath(rel).catch(() => {});
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }

  const EDITOR_FONTS = [
    { label: tk("Literata (serif de lecture)"), value: '"Literata Variable", Literata, Georgia, serif' },
    { label: tk("Source Serif"), value: '"Source Serif 4 Variable", "Source Serif 4", Georgia, serif' },
    { label: tk("Lora"), value: '"Lora Variable", Lora, Georgia, serif' },
    { label: tk("Georgia"), value: "Georgia, serif" },
    { label: tk("Inter (sans serif)"), value: '"Inter Variable", Inter, system-ui, sans-serif' },
    { label: tk("Segoe UI / système"), value: '"Segoe UI", system-ui, sans-serif' },
    { label: tk("JetBrains Mono (chasse fixe)"), value: '"JetBrains Mono Variable", "JetBrains Mono", monospace' },
  ];
  const UI_FONTS = [
    { label: tk("Inter"), value: '"Inter Variable", Inter, "Segoe UI", system-ui, sans-serif' },
    { label: tk("Segoe UI / système"), value: '"Segoe UI", system-ui, sans-serif' },
    { label: tk("Source Serif"), value: '"Source Serif 4 Variable", Georgia, serif' },
  ];
  const ACCENTS = ["#7c5cff", "#3a7bd5", "#2c8c99", "#2f8a57", "#d4880f", "#c2413b", "#b0457e", "#5b5766"];

  $effect(() => {
    if (section === "export" && pandoc === undefined) api.pandocVersion().then((v) => (pandoc = v));
  });

  const isCustom = (list: { value: string }[], v: string) => !list.some((f) => f.value === v);

  const SHORTCUTS: [string, string][] = [
    ["Ctrl+P", tk("Palette de commandes")],
    ["Ctrl+O", tk("Ouvrir une note")],
    ["Ctrl+N / Ctrl+Maj+N", tk("Nouvelle note / depuis un modèle")],
    ["Ctrl+Maj+F", tk("Rechercher dans le coffre")],
    ["Ctrl+F", tk("Rechercher / remplacer dans la note")],
    ["Ctrl+G", tk("Graphe du coffre")],
    ["Ctrl+Maj+D", tk("Note du jour")],
    ["Ctrl+\\", tk("Diviser l'écran")],
    ["Ctrl+W, Ctrl+Tab", tk("Fermer / changer d'onglet")],
    ["Ctrl+B, Ctrl+I", tk("Gras, italique")],
    ["Ctrl+Maj+H, Ctrl+Maj+X", tk("Surligner, barrer")],
    ["Ctrl+E, Ctrl+K", tk("Code en ligne, lien")],
    ["Ctrl+Alt+1…6, Ctrl+Alt+0", tk("Titre de niveau 1 à 6, paragraphe")],
    ["Ctrl+L", tk("Case à cocher")],
    ["Ctrl+Alt+T, Ctrl+Alt+Entrée", tk("Aligner le tableau, ajouter une ligne")],
    ["Ctrl+Maj+C", tk("Citer une référence Zotero")],
    ["Ctrl+Maj+P", tk("Aperçu en direct / Markdown brut")],
    ["Ctrl+Maj+E", tk("Exporter la note")],
    ["Ctrl+clic", tk("Suivre un lien sur la ligne en cours d'édition")],
    ["Alt+clic", tk("Ouvrir à côté")],
    ["/  [[  [@  #", tk("Blocs, liens, citations, tags")],
    ["F2, F11", tk("Renommer, mode concentration")],
  ];

  /** Keys in the interface language (Maj → Shift, Entrée → Enter, clic → click). */
  const keysText = (k: string) => k.replaceAll("Maj", t("Maj")).replaceAll("Entrée", t("Entrée")).replaceAll("clic", t("clic"));

  const version = __APP_VERSION__;

  // ------------------------------------------------------------------- IA
  import type { AiLogEntry } from "../lib/types";
  let keyInput = $state("");
  let keyPresent = $state<boolean | null>(null);
  let testResult = $state("");
  let testing = $state(false);
  let journal = $state<AiLogEntry[]>([]);
  const MODELS: Record<string, string[]> = {
    anthropic: ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5"],
    openai: [],
    compatible: [],
  };
  $effect(() => {
    if (section !== "ia") return;
    const p = s.ai_provider;
    api.aiKeyPresent(p).then((v) => (keyPresent = v)).catch(() => (keyPresent = null));
    api.aiJournal().then((j) => (journal = j)).catch(() => (journal = []));
  });
  async function saveKey(value: string) {
    try {
      await api.aiSetKey(s.ai_provider, value);
      keyInput = "";
      keyPresent = !!value;
      app.toast(value ? t("Clé enregistrée dans le trousseau du système") : t("Clé effacée"), "success");
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }
  async function testAi() {
    testing = true;
    testResult = "";
    try {
      const r = await api.aiTest();
      testResult = t("Connexion réussie ({model}) : « {text} »", { model: r.model, text: r.text.trim() });
    } catch (e) {
      testResult = translateError(e);
    } finally {
      testing = false;
    }
  }

  // ---------------------------------------------------------------- raccourcis
  /** Command waiting for its new key combination. */
  let recording = $state<string | null>(null);
  const conflicts = $derived.by(() => {
    void s.keys;
    return keyConflicts();
  });
  const conflictOf = (id: string) => {
    const k = keysOf(commands.find((c) => c.id === id)!)?.toLowerCase();
    return k && conflicts.has(k) ? conflicts.get(k)!.filter((x) => x !== id) : [];
  };
  const labelOf = (id: string) => {
    const c = commands.find((x) => x.id === id);
    return c ? cmdLabel(c) : id;
  };

  function setKey(id: string, combo: string | null) {
    const keys = { ...(s.keys ?? {}) };
    const def = commands.find((c) => c.id === id)?.keys;
    if (combo === null || combo === def) delete keys[id];
    else keys[id] = combo;
    set({ keys });
  }

  function onRecordKey(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") {
      recording = null;
      return;
    }
    if (e.key === "Backspace" || e.key === "Delete") {
      setKey(recording, "");
      recording = null;
      return;
    }
    const combo = comboFromEvent(e);
    if (combo) {
      setKey(recording, combo);
      recording = null;
    }
  }
</script>

<Modal title={t("Réglages")} width={1000} onclose={() => { app.settingsOpen = false; app.settingsSection = "general"; app.saveSettingsNow(); }}>
  <div class="layout">
    <nav>
      <div class="search">
        <Icon name="search" size={14} />
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={query} placeholder={t("Rechercher un réglage")} aria-label={t("Rechercher un réglage")} autofocus />
      </div>
      {#each GROUPS as g (g.label)}
        <p class="group">{t(g.label)}</p>
        {#each g.items as id (id)}
          <button class:on={!query && section === id} onclick={() => { query = ""; section = id; }}>
            <Icon name={ICONS[id]} size={15} />
            <span>{t(SECTIONS.find((x) => x[0] === id)![1])}</span>
          </button>
        {/each}
      {/each}
    </nav>
    <div class="pane" bind:this={paneEl}>
      {#if show("general")}
      <section class="sec" data-sec="general">
        <h2 class="sec-title">{t(tk("Général"))}</h2>
        <div class="field">
          <span class="lbl">{t("Objectifs")}</span>
          <p class="hint">{t("Affichés au tableau de bord et dans la revue de la semaine. 0 : pas d'objectif.")}</p>
          <div class="inline">
            <label class="small">{t("Mots par jour")} <input class="input num" type="number" min="0" step="50" value={s.goal_words_day} onchange={(e) => set({ goal_words_day: Number((e.target as HTMLInputElement).value) || 0 })} /></label>
            <label class="small">{t("Fiches lues par semaine")} <input class="input num" type="number" min="0" value={s.goal_fiches_week} onchange={(e) => set({ goal_fiches_week: Number((e.target as HTMLInputElement).value) || 0 })} /></label>
            <label class="small">{t("Session de concentration (min)")} <input class="input num" type="number" min="5" max="120" step="5" value={s.focus_minutes} onchange={(e) => set({ focus_minutes: Math.max(5, Number((e.target as HTMLInputElement).value) || 25) })} /></label>
          </div>
        </div>
        <div class="field">
          <span class="lbl">{t("Langue de l'interface")}</span>
          <div class="seg">
            {#each LANGS as l (l.id)}
              <button class:on={s.language === l.id} onclick={() => set({ language: l.id })}>{l.label}</button>
            {/each}
          </div>
          <p class="hint">{t("Les notes créées par Eloria (fiches de lecture, note du jour) suivent aussi cette langue. Les notes existantes ne sont pas modifiées.")}</p>
        </div>
      </section>
      {/if}
      {#if show("apparence")}
      <section class="sec" data-sec="apparence">
        <h2 class="sec-title">{t(tk("Apparence"))}</h2>
        <div class="field">
          <span class="lbl">{t("Thème")}</span>
          <div class="gallery">
            <button class="th" class:on={s.theme === "system"} onclick={() => pickTheme("system", "#7c5cff")}>
              <span class="mini split"><span class="half" style:background="#fbfbfa"></span><span class="half" style:background="#1c1a24"></span></span>
              <span class="tname">{t("Système")}</span>
            </button>
            {#each THEMES as th (th.id)}
              {@const c = previewColors(th)}
              <button class="th" class:on={s.theme === th.id} onclick={() => pickTheme(th.id, th.accent)} data-theme-id={th.id}>
                <span class="mini" style:background={c.bg} style:border-color={c.line}>
                  <span class="side" style:background={c["bg-soft"]}></span>
                  <span class="lines">
                    <span style:background={c.text} style:width="70%"></span>
                    <span style:background={c["text-faint"]} style:width="90%"></span>
                    <span style:background={th.accent} style:width="45%"></span>
                    <span class="code"><i style:background={c["syn-keyword"]}></i><i style:background={c["syn-string"]}></i></span>
                  </span>
                </span>
                <span class="tname">{t(th.name)}</span>
              </button>
            {/each}
            {#each app.customThemes as ct (ct.id)}
              <button class="th" class:on={s.theme === CUSTOM_PREFIX + ct.id} onclick={() => pickTheme(CUSTOM_PREFIX + ct.id)}>
                <span class="mini custom">CSS</span>
                <span class="tname">{ct.name}</span>
              </button>
            {/each}
          </div>
          <div class="row mt">
            <button class="btn small" onclick={newCustomTheme}>{t("Créer un thème personnel…")}</button>
            <button class="btn small ghost" onclick={() => app.loadCustomThemes()}>{t("Recharger les thèmes")}</button>
          </div>
          <p class="hint">{t("Thèmes personnels : fichiers .css dans le dossier .eloria/themes du coffre. Le thème d'exemple liste toutes les variables utiles.")}</p>
        </div>
        <div class="field">
          <span class="lbl">{t("Couleur d'accent")}</span>
          <div class="swatches">
            {#each ACCENTS as c (c)}
              <button class="sw" class:on={s.accent === c} style:background={c} aria-label={t("Accent {c}", { c })} onclick={() => set({ accent: c })}></button>
            {/each}
            <input type="color" value={s.accent} oninput={(e) => set({ accent: (e.target as HTMLInputElement).value })} aria-label={t("Couleur personnalisée")} />
          </div>
        </div>
        <div class="field">
          <label class="lbl" for="f-ed">{t("Police du texte")}</label>
          <select id="f-ed" class="input" value={isCustom(EDITOR_FONTS, s.font_editor) ? "__custom" : s.font_editor} onchange={(e) => { const v = (e.target as HTMLSelectElement).value; if (v !== "__custom") set({ font_editor: v }); }}>
            {#each EDITOR_FONTS as f (f.value)}<option value={f.value}>{t(f.label)}</option>{/each}
            <option value="__custom">{t("Personnalisée…")}</option>
          </select>
          <input class="input mt" value={s.font_editor} onchange={(e) => set({ font_editor: (e.target as HTMLInputElement).value })} placeholder={t("Nom de police installée, ex. Garamond")} />
          <p class="preview" style:font-family={s.font_editor} style:font-size="{s.font_size}px" style:line-height={s.line_height}>
            {t("La détection de fraude dans les paiements mobiles exige des modèles à la fois précis et explicables.")}
          </p>
        </div>
        <div class="field">
          <label class="lbl" for="f-ui">{t("Police de l'interface")}</label>
          <select id="f-ui" class="input" value={s.font_ui} onchange={(e) => set({ font_ui: (e.target as HTMLSelectElement).value })}>
            {#each UI_FONTS as f (f.value)}<option value={f.value}>{t(f.label)}</option>{/each}
            {#if isCustom(UI_FONTS, s.font_ui)}<option value={s.font_ui}>{t("Personnalisée")}</option>{/if}
          </select>
        </div>
        <div class="grid2">
          <div class="field">
            <label class="lbl" for="f-size">{t("Taille du texte : {n} px", { n: s.font_size })}</label>
            <input id="f-size" type="range" min="12" max="28" step="0.5" value={s.font_size} oninput={(e) => set({ font_size: +(e.target as HTMLInputElement).value })} />
          </div>
          <div class="field">
            <label class="lbl" for="f-lh">{t("Interligne : {n}", { n: s.line_height.toFixed(2) })}</label>
            <input id="f-lh" type="range" min="1.2" max="2.2" step="0.05" value={s.line_height} oninput={(e) => set({ line_height: +(e.target as HTMLInputElement).value })} />
          </div>
          <div class="field">
            <label class="lbl" for="f-w">{t("Largeur de la colonne : {n} px", { n: s.editor_width })}</label>
            <input id="f-w" type="range" min="520" max="1400" step="10" value={s.editor_width} oninput={(e) => set({ editor_width: +(e.target as HTMLInputElement).value })} />
          </div>
          <div class="field">
            <label class="lbl" for="f-h">{t("Taille des titres : {n} %", { n: Math.round(s.heading_scale * 100) })}</label>
            <input id="f-h" type="range" min="0.8" max="1.4" step="0.05" value={s.heading_scale} oninput={(e) => set({ heading_scale: +(e.target as HTMLInputElement).value })} />
          </div>
        </div>
        <label class="check"><input type="checkbox" checked={s.readable_line_length} onchange={() => set({ readable_line_length: !s.readable_line_length })} /> {t("Limiter la largeur de la colonne d'écriture")}</label>
      </section>
      {/if}
      {#if show("editeur")}
      <section class="sec" data-sec="editeur">
        <h2 class="sec-title">{t(tk("Éditeur"))}</h2>
        <label class="check"><input type="checkbox" checked={s.live_preview} onchange={() => set({ live_preview: !s.live_preview })} /> {t("Aperçu en direct (la syntaxe se masque hors de la ligne en cours)")}</label>
        <label class="check"><input type="checkbox" checked={s.spellcheck} onchange={() => set({ spellcheck: !s.spellcheck })} /> {t("Correcteur orthographique")}</label>
        <div class="field">
          <label class="lbl" for="f-lang">{t("Langue du correcteur")}</label>
          <select id="f-lang" class="input" value={s.spellcheck_lang} onchange={(e) => set({ spellcheck_lang: (e.target as HTMLSelectElement).value })}>
            <option value="fr">{t("Français")}</option>
            <option value="en-US">{t("Anglais (États-Unis)")}</option>
            <option value="en-GB">{t("Anglais (Royaume-Uni)")}</option>
          </select>
          <p class="hint">{t("Le correcteur est celui de Windows (WebView2). Ajoute la langue dans Paramètres Windows › Heure et langue si elle manque.")}</p>
        </div>
        <label class="check"><input type="checkbox" checked={s.show_line_numbers} onchange={() => set({ show_line_numbers: !s.show_line_numbers })} /> {t("Numéros de ligne")}</label>
        <div class="grid2">
          <div class="field">
            <label class="lbl" for="f-as">{t("Enregistrement automatique après {n} ms", { n: s.autosave_ms })}</label>
            <input id="f-as" type="range" min="300" max="5000" step="100" value={s.autosave_ms} oninput={(e) => set({ autosave_ms: +(e.target as HTMLInputElement).value })} />
          </div>
          <div class="field">
            <label class="lbl" for="f-hi">{t("Une version d'historique toutes les {n} min", { n: Math.round(s.history_interval / 60) })}</label>
            <input id="f-hi" type="range" min="60" max="3600" step="60" value={s.history_interval} oninput={(e) => set({ history_interval: +(e.target as HTMLInputElement).value })} />
          </div>
        </div>
      </section>
      {/if}
      {#if show("zotero")}
      <section class="sec" data-sec="zotero">
        <h2 class="sec-title">{t(tk("Zotero"))}</h2>
        <div class="field">
          <span class="lbl">{t("Dossier de données Zotero")}</span>
          <div class="inline">
            <input class="input" value={s.zotero_data_dir} placeholder={t("Détection automatique (~/Zotero)")} onchange={(e) => set({ zotero_data_dir: (e.target as HTMLInputElement).value })} />
            <button class="btn" onclick={async () => { const p = await pickFolder(t("Dossier contenant zotero.sqlite")); if (p) set({ zotero_data_dir: p }); }}>{t("Parcourir")}</button>
          </div>
          <p class="hint">{t("Dans Zotero : Édition › Paramètres › Avancé › Fichiers et dossiers › Répertoire de données. Le dossier contient")} <code>zotero.sqlite</code>.</p>
        </div>
        <div class="field">
          <label class="lbl" for="f-port">{t("Port de l'API locale (repli si la base est inaccessible)")}</label>
          <input id="f-port" class="input short" type="number" value={s.zotero_port} onchange={(e) => set({ zotero_port: +(e.target as HTMLInputElement).value })} />
        </div>
        <div class="field">
          <label class="lbl" for="f-fiches">{t("Dossier des fiches de lecture")}</label>
          <input id="f-fiches" class="input" value={s.fiches_folder} onchange={(e) => set({ fiches_folder: (e.target as HTMLInputElement).value })} />
          <p class="hint">{t("Modèle personnalisable : crée")} <code>{s.templates_folder}/Fiche de lecture.md</code> ({t("variables :")} <code>{"{{ title }}"}</code>, <code>{"{{ citekey }}"}</code>, <code>{"{{ authors_short }}"}</code>, <code>{"{{ year }}"}</code>, <code>{"{{ abstract }}"}</code>, <code>{"{{ annotations_block }}"}</code>…).</p>
        </div>
        <label class="check"><input type="checkbox" checked={s.auto_reading_status} onchange={() => set({ auto_reading_status: !s.auto_reading_status })} /> {t("Chaîne de lecture : le statut des fiches avance seul (en cours, lue, exploitée)")}</label>
        <label class="check"><input type="checkbox" checked={s.zotero_auto_sync} onchange={() => set({ zotero_auto_sync: !s.zotero_auto_sync })} /> {t("Synchroniser automatiquement quand Zotero change")}</label>
        <label class="check"><input type="checkbox" checked={s.copy_pdfs} onchange={() => set({ copy_pdfs: !s.copy_pdfs })} /> {t("Copier les PDF dans le coffre (fiches et import)")}</label>
        <div class="field">
          <label class="lbl" for="f-pdf">{t("Dossier des PDF copiés")}</label>
          <input id="f-pdf" class="input" value={s.pdf_folder} onchange={(e) => set({ pdf_folder: (e.target as HTMLInputElement).value })} />
          <p class="hint">{t("Le coffre reste complet même si Zotero ou son dossier de fichiers disparaît.")}</p>
        </div>
        <div class="row">
          <button class="btn primary" onclick={() => app.zoteroSync("auto")}>{t("Synchroniser maintenant")}</button>
          <button class="btn" onclick={() => app.zoteroSync("sqlite")}>{t("Forcer la base locale")}</button>
          <button class="btn" onclick={() => app.zoteroSync("api")}>{t("Forcer l'API locale")}</button>
        </div>
      </section>
      {/if}
      {#if show("ia")}
      <section class="sec" data-sec="ia">
        <h2 class="sec-title">{t(tk("Intelligence artificielle"))}</h2>
        <label class="check"><input type="checkbox" checked={s.ai_enabled} onchange={() => set({ ai_enabled: !s.ai_enabled })} /> <b>{t("Activer l'intelligence artificielle")}</b></label>
        <p class="hint">{t("Désactivée, Eloria fonctionne entièrement sans IA. Activée, rien n'est envoyé sans une action de ta part (par exemple « Analyse IA » sur un document).")}</p>
        <div class="field">
          <span class="lbl">{t("Fournisseur")}</span>
          <div class="seg">
            {#each [["anthropic", "Anthropic (Claude)"], ["openai", "OpenAI"], ["compatible", t("Compatible OpenAI")]] as [v, l] (v)}
              <button class:on={s.ai_provider === v} onclick={() => set({ ai_provider: v as typeof s.ai_provider, ai_model: "" })}>{l}</button>
            {/each}
          </div>
          {#if s.ai_provider === "compatible"}<p class="hint">{t("Ollama en local, Mistral, OpenRouter… : indique l'adresse de l'API ci-dessous.")}</p>{/if}
        </div>
        {#if s.ai_provider === "compatible"}
          <div class="field">
            <label class="lbl" for="f-url">{t("Adresse de l'API")}</label>
            <input id="f-url" class="input" value={s.ai_base_url} placeholder="http://localhost:11434/v1" onchange={(e) => set({ ai_base_url: (e.target as HTMLInputElement).value })} />
          </div>
        {/if}
        <div class="field">
          <label class="lbl" for="f-model">{t("Modèle")}</label>
          <input id="f-model" class="input" list="ai-models" value={s.ai_model} placeholder={s.ai_provider === "anthropic" ? "claude-opus-5-5" : t("nom du modèle")} onchange={(e) => set({ ai_model: (e.target as HTMLInputElement).value.trim() })} />
          <datalist id="ai-models">{#each MODELS[s.ai_provider] ?? [] as m (m)}<option value={m}></option>{/each}</datalist>
          {#if s.ai_provider === "anthropic"}<p class="hint">{t("Vide : Claude Opus 5.5, le plus capable pour l'analyse d'articles. Sonnet 5.5 et Haiku 5.5 coûtent moins cher.")}</p>{/if}
        </div>
        <div class="field">
          <span class="lbl">{t("Clé API")}</span>
          <div class="inline">
            <input class="input" type="password" autocomplete="off" bind:value={keyInput} placeholder={keyPresent ? t("Clé enregistrée (••••)") : t("Colle ta clé ici")} />
            <button class="btn" disabled={!keyInput.trim()} onclick={() => saveKey(keyInput.trim())}>{t("Enregistrer")}</button>
            {#if keyPresent}<button class="btn ghost" onclick={() => saveKey("")}>{t("Effacer")}</button>{/if}
          </div>
          <p class="hint">
            {#if keyPresent}{t("Clé enregistrée dans le Gestionnaire d'identification Windows, jamais dans les fichiers d'Eloria.")}{:else if s.ai_provider === "compatible"}{t("Facultative pour un service local comme Ollama.")}{:else}{t("Aucune clé enregistrée pour ce fournisseur.")}{/if}
          </p>
        </div>
        {#if s.ai_provider === "anthropic"}
          <div class="field">
            <label class="lbl" for="f-effort">{t("Profondeur de réflexion")}</label>
            <select id="f-effort" class="input short-select" value={s.ai_effort} onchange={(e) => set({ ai_effort: (e.target as HTMLSelectElement).value as typeof s.ai_effort })}>
              <option value="low">{t("Faible : rapide, économique")}</option>
              <option value="medium">{t("Moyenne")}</option>
              <option value="high">{t("Élevée : recommandée pour les fiches")}</option>
              <option value="xhigh">{t("Très élevée")}</option>
              <option value="max">{t("Maximale : la plus coûteuse")}</option>
            </select>
          </div>
        {/if}
        <div class="field">
          <label class="lbl" for="f-topic">{t("Sujet de ta recherche")}</label>
          <textarea id="f-topic" class="input" rows="2" value={s.research_topic} placeholder={t("Ex. détection de la fraude dans le mobile money en Afrique de l'Ouest")} onchange={(e) => set({ research_topic: (e.target as HTMLTextAreaElement).value })}></textarea>
          <p class="hint">{t("Donné à l'IA pour que son commentaire juge l'intérêt de chaque article pour ta recherche.")}</p>
        </div>
        <div class="field">
          <span class="lbl">{t("Serveur MCP (Claude Desktop, autres assistants)")}</span>
          <p class="hint">{t("Un assistant extérieur peut interroger ce coffre : recherche, notes, sources, extraits sourcés. Lecture seule ; ajoute « --ecriture » aux arguments pour l'autoriser à créer des notes. Colle ce bloc dans claude_desktop_config.json (Claude Desktop › Réglages › Développeur), puis redémarre Claude Desktop.")}</p>
          {#if mcp}
            <pre class="mono cfg">{mcp.config}</pre>
            <div class="row">
              <button class="btn small" onclick={() => { navigator.clipboard.writeText(mcp!.config); app.toast(t("Configuration copiée"), "success"); }}>{t("Copier la configuration")}</button>
              {#if !mcp.exists}<span class="hint">{t("eloria-mcp.exe introuvable à côté d'Eloria : compile-le avec « cargo build -p eloria-mcp --release ».")}</span>{/if}
            </div>
          {/if}
        </div>
        <label class="check"><input type="checkbox" checked={s.local_api} onchange={toggleApi} /> {t("Capture web : recevoir des pages du navigateur (API locale 127.0.0.1, protégée par un jeton)")}</label>
        {#if s.local_api && bookmarklet}
          <div class="field">
            <span class="lbl">{t("Signet « Envoyer à Eloria »")}</span>
            <p class="hint">{t("Glisse ce lien dans la barre de favoris du navigateur. Sur une page, un clic l'envoie (titre, adresse, texte sélectionné) dans la boîte de réception. Eloria doit être ouvert.")}</p>
            <div class="row">
              <a class="btn small primary" href={bookmarklet} onclick={(e) => e.preventDefault()}>{t("Envoyer à Eloria")}</a>
              <button class="btn small" onclick={() => { navigator.clipboard.writeText(bookmarklet!); app.toast(t("Signet copié"), "success"); }}>{t("Copier le signet")}</button>
              <span class="hint mono">127.0.0.1:{s.local_api_port}</span>
            </div>
          </div>
        {/if}
        <div class="field">
          <span class="lbl">{t("Assistants personnalisés")}</span>
          <p class="hint">{t("Un nom et des consignes : ils apparaissent comme modes d'Ask Eloria, avec les mêmes garanties (extraits sourcés, références vérifiées).")}</p>
          <div class="agents">
            {#each s.ai_agents ?? [] as ag, i (i)}
              <div class="agent">
                <input class="input" value={ag.name} placeholder={t("Nom (ex. Relecteur de chapitre)")} aria-label={t("Nom de l'assistant")} onchange={(e) => set({ ai_agents: s.ai_agents.map((x, j) => (j === i ? { ...x, name: (e.target as HTMLInputElement).value } : x)) })} />
                <textarea class="input" rows="2" value={ag.instructions} placeholder={t("Consignes")} aria-label={t("Consignes de l'assistant")} onchange={(e) => set({ ai_agents: s.ai_agents.map((x, j) => (j === i ? { ...x, instructions: (e.target as HTMLTextAreaElement).value } : x)) })}></textarea>
                <button class="btn small ghost" onclick={() => set({ ai_agents: s.ai_agents.filter((_, j) => j !== i) })}>{t("Retirer")}</button>
              </div>
            {/each}
            <button class="btn small" onclick={() => set({ ai_agents: [...(s.ai_agents ?? []), { name: "", instructions: "" }] })}>{t("Ajouter un assistant")}</button>
          </div>
        </div>
        <div class="field">
          <span class="lbl">{t("Recherche par le sens (vecteurs)")}</span>
          <div class="seg">
            {#each [["", t("Désactivée")], ["ollama", t("Ollama (sur l'ordinateur)")], ["voyage", "Voyage AI"], ["openai", "OpenAI"]] as [v, l] (v)}
              <button class:on={s.embed_provider === v} onclick={() => set({ embed_provider: v, embed_model: "" })}>{l}</button>
            {/each}
          </div>
          {#if s.embed_provider}
            <div class="inline">
              <input class="input" value={s.embed_model} placeholder={s.embed_provider === "ollama" ? "nomic-embed-text" : s.embed_provider === "voyage" ? "voyage-3.5" : "text-embedding-3-small"} aria-label={t("Modèle de vecteurs")} onchange={(e) => set({ embed_model: (e.target as HTMLInputElement).value })} />
              <input class="input" value={s.embed_url} placeholder={t("Adresse (vide = celle du fournisseur)")} aria-label={t("Adresse du service de vecteurs")} onchange={(e) => set({ embed_url: (e.target as HTMLInputElement).value })} />
            </div>
            {#if s.embed_provider !== "ollama"}
              <div class="inline">
                <input class="input" type="password" placeholder={s.embed_provider === "openai" ? t("Vide = clé OpenAI de l'assistant") : t("Clé API Voyage")} aria-label={t("Clé API des vecteurs")} onchange={async (e) => { await api.semanticSetKey(s.embed_provider, (e.target as HTMLInputElement).value); app.toast(t("Clé enregistrée dans le Gestionnaire d'identification Windows"), "success"); }} />
              </div>
            {/if}
            <p class="hint">{t("Chaque note est envoyée une fois au fournisseur pour calculer son vecteur (Ollama : rien ne quitte l'ordinateur). Seules les notes modifiées sont recalculées. Lance le calcul depuis Recherche › Par le sens.")}</p>
          {/if}
        </div>
        <div class="row">
          <button class="btn" disabled={testing || !s.ai_enabled} onclick={testAi}>{testing ? t("Test en cours…") : t("Tester la connexion")}</button>
        </div>
        {#if testResult}<p class="hint">{testResult}</p>{/if}
        <p class="hint privacy">{t("Confidentialité : quand tu lances une analyse, le contenu de l'article (PDF ou texte) et tes annotations sont envoyés au fournisseur choisi, qui facture selon les jetons utilisés. Le journal ci-dessous garde la date, le modèle et le nombre de jetons, jamais le contenu.")}</p>
        {#if journal.length}
          <table class="keys journal">
            <tbody>
              {#each journal.slice(0, 8) as j, i (i)}
                <tr><td>{new Date(j.date).toLocaleString(locale())}</td><td>{j.model}</td><td>{t("{inp} / {out} jetons", { inp: j.input_tokens.toLocaleString(locale()), out: j.output_tokens.toLocaleString(locale()) })}</td><td>{j.ok ? "" : t("échec")}</td></tr>
              {/each}
            </tbody>
          </table>
        {/if}
      </section>
      {/if}
      {#if show("fichiers")}
      <section class="sec" data-sec="fichiers">
        <h2 class="sec-title">{t(tk("Fichiers et dossiers"))}</h2>
        <div class="field">
          <label class="lbl" for="f-att">{t("Dossier des images collées")}</label>
          <input id="f-att" class="input" value={s.attachments_folder} onchange={(e) => set({ attachments_folder: (e.target as HTMLInputElement).value })} />
        </div>
        <div class="field">
          <label class="lbl" for="f-tpl">{t("Dossier des modèles")}</label>
          <input id="f-tpl" class="input" value={s.templates_folder} onchange={(e) => set({ templates_folder: (e.target as HTMLInputElement).value })} />
          <p class="hint">{t("Variables :")} <code>{"{{title}}"}</code>, <code>{"{{date}}"}</code>, <code>{"{{time}}"}</code>. {t("Un modèle Journal.md sert à la note du jour.")}</p>
        </div>
        <div class="field">
          <label class="lbl" for="f-daily">{t("Dossier du journal")}</label>
          <input id="f-daily" class="input" value={s.daily_folder} onchange={(e) => set({ daily_folder: (e.target as HTMLInputElement).value })} />
        </div>
        <div class="field">
          <span class="lbl">{t("Sauvegardes automatiques")}</span>
          <div class="inline">
            <input class="input" value={s.backup_dir} placeholder={t("Dossier hors du coffre (disque externe, OneDrive…)")} onchange={(e) => set({ backup_dir: (e.target as HTMLInputElement).value })} />
            <button class="btn" onclick={async () => { const p = await pickFolder(t("Dossier des sauvegardes")); if (p) { set({ backup_dir: p }); loadBackups(); } }}>{t("Choisir")}</button>
          </div>
          <div class="inline">
            <label class="small">{t("Toutes les")} <input class="input num" type="number" min="0" value={s.backup_interval_hours} onchange={(e) => set({ backup_interval_hours: Number((e.target as HTMLInputElement).value) || 0 })} /> {t("heures")}</label>
            <label class="small">{t("Garder")} <input class="input num" type="number" min="1" value={s.backup_keep} onchange={(e) => set({ backup_keep: Math.max(1, Number((e.target as HTMLInputElement).value) || 1) })} /> {t("archives")}</label>
            <button class="btn" disabled={!s.backup_dir} onclick={async () => { await app.backupNow(); loadBackups(); }}>{t("Sauvegarder maintenant")}</button>
          </div>
          <p class="hint">{t("Archive .zip de tout le coffre (notes, PDF, images, historique), sans l'index qui se reconstruit. 0 heure : sauvegarde manuelle seulement.")}</p>
          {#each backups.slice(0, 5) as b (b.path)}
            <div class="backup-row">
              <span class="mono">{b.created}</span>
              <span class="faint">{(b.bytes / 1_048_576).toFixed(1)} Mo</span>
              <button class="btn small ghost" onclick={() => restore(b.path)}>{t("Restaurer…")}</button>
            </div>
          {/each}
          {#if restored}<p class="hint ok">{restored}</p>{/if}
        </div>
        <div class="field">
          <span class="lbl">{t("Versions Git")}</span>
          <p class="hint">
            {#if !git}{t("Vérification…")}{:else if !git.available}{t("Git n'est pas installé : winget install Git.Git")}{:else if !git.repo}{t("Le coffre n'est pas encore un dépôt Git. Chaque version garde tout le coffre (sans l'index ni la corbeille).")}{:else}{t("Branche {b} · {n} fichiers modifiés depuis la dernière version", { b: git.branch || "?", n: git.changes })}{git.remote ? ` · ${git.remote}` : ""}{/if}
          </p>
          <div class="row">
            {#if git?.available}
              <button class="btn" onclick={async () => { git = (await app.gitSave()) ?? git; }}>{git.repo ? t("Enregistrer une version") : t("Activer et enregistrer une version")}</button>
              {#if git.repo && git.remote}<button class="btn" onclick={async () => { try { git = await api.gitSync(); app.toast(t("Dépôt distant à jour"), "success"); } catch (e) { app.toast(translateError(e), "error"); } }}>{t("Envoyer et récupérer")}</button>{/if}
            {/if}
          </div>
          {#each git?.log.slice(0, 4) ?? [] as [h, d, m] (h)}<div class="backup-row"><span class="mono">{h}</span><span class="faint">{d}</span><span>{m}</span></div>{/each}
        </div>
        <div class="field">
          <span class="lbl">{t("Importer des notes")}</span>
          <p class="hint">{t("Un dossier Markdown d'Obsidian, Notion (export Markdown), Logseq ou Evernote : notes et pièces jointes recopiées, liens convertis en [[liens]], rien n'est écrasé.")}</p>
          <div class="row"><button class="btn" onclick={() => app.importMarkdownDir()}>{t("Choisir le dossier…")}</button></div>
        </div>
        <div class="field">
          <span class="lbl">{t("Vérification du coffre")}</span>
          <p class="hint">{t("Liens cassés, fichiers introuvables, propriétés illisibles, fiches sans source.")}</p>
          <div class="row"><button class="btn" onclick={() => { app.settingsOpen = false; app.integrityOpen = true; }}>{t("Vérifier le coffre")}</button></div>
        </div>
        <div class="field">
          <span class="lbl">{t("Coffre ouvert")}</span>
          <p class="hint mono">{app.vault?.root}</p>
          <div class="row">
            <button class="btn" onclick={() => { app.settingsOpen = false; app.closeVault(); }}>{t("Changer de coffre")}</button>
          </div>
        </div>
      </section>
      {/if}
      {#if show("export")}
      <section class="sec" data-sec="export">
        <h2 class="sec-title">{t(tk("Export"))}</h2>
        <div class="field">
          <span class="lbl">Pandoc</span>
          <!-- A single help block: two .hint elements would share the same grid cell. -->
          <div class="hint">
            <p>{#if pandoc === undefined}{t("Vérification…")}{:else if pandoc}<span class="ok">{pandoc}</span>{:else}{t("Non trouvé. Installe-le avec")} <code>winget install JohnMacFarlane.Pandoc</code> {t("pour exporter en Word, ODT, LaTeX et PDF. L'export HTML fonctionne sans.")}{/if}</p>
            <p>{t("PDF : installe aussi Typst, plus léger que LaTeX :")} <code>winget install Typst.Typst</code></p>
          </div>
          <input class="input" value={s.pandoc_path} placeholder={t("Chemin de pandoc.exe (vide = PATH)")} onchange={(e) => { set({ pandoc_path: (e.target as HTMLInputElement).value }); pandoc = undefined; }} />
        </div>
        <div class="field">
          <span class="lbl">{t("Moteur LaTeX")}</span>
          <p class="hint">
            {#if texEngine === undefined}{t("Vérification…")}{:else if texEngine}<span class="ok">{texEngine.kind}</span> · {texEngine.version} <span class="mono">{texEngine.path}</span>{:else}{t("Aucun moteur trouvé. Tectonic compile sur ton ordinateur, sans service en ligne.")}{/if}
          </p>
          <div class="inline">
            <input class="input" value={s.latex_path} placeholder={t("Chemin de tectonic, latexmk ou pdflatex (vide = recherche automatique)")} onchange={(e) => { set({ latex_path: (e.target as HTMLInputElement).value }); loadTex(); }} />
            {#if !texEngine}<button class="btn" disabled={installingTex} onclick={installTex}>{installingTex ? t("Téléchargement…") : t("Installer Tectonic")}</button>{/if}
          </div>
        </div>
        <div class="field">
          <span class="lbl">{t("Style de citation (CSL)")}</span>
          <div class="inline">
            <input class="input" value={s.csl_path} placeholder={t("Style par défaut de Pandoc (Chicago auteur-date)")} onchange={(e) => set({ csl_path: (e.target as HTMLInputElement).value })} />
            <button class="btn" onclick={async () => { const p = await pickFile(["csl"], t("Style CSL")); if (p) set({ csl_path: p }); }}>{t("Choisir")}</button>
          </div>
          <p class="hint">{t("Les styles APA, IEEE, ISO 690 se téléchargent sur zotero.org/styles.")}</p>
        </div>
      </section>
      {/if}
      {#if show("personnalisation")}
      <section class="sec" data-sec="personnalisation">
        <h2 class="sec-title">{t(tk("Personnalisation"))}</h2>
        <CustomizeSettings />
      </section>
      {/if}
      {#if show("extensions")}
      <section class="sec" data-sec="extensions">
        <h2 class="sec-title">{t(tk("Extensions"))}</h2>
        <ExtensionsSettings />
      </section>
      {/if}
      {#if show("raccourcis")}
      <section class="sec" data-sec="raccourcis">
        <h2 class="sec-title">{t(tk("Raccourcis"))}</h2>
        <p class="hint top">{t("Clique sur un raccourci puis appuie sur la nouvelle combinaison. Retour arrière : désactiver. Échap : annuler.")}</p>
        <table class="keys editable">
          <tbody>
            {#each commands.filter((c) => c.keys || s.keys?.[c.id] || c.id.startsWith("custom:")) as c (c.id)}
              {@const k = keysOf(c)}
              {@const clash = conflictOf(c.id)}
              <tr class:custom={s.keys?.[c.id] !== undefined}>
                <td>{cmdLabel(c)}<span class="grp">{t(c.group)}</span></td>
                <td>
                  <button class="kbtn" class:rec={recording === c.id} class:clash={clash.length > 0} data-cmd={c.id} data-capture-keys={recording === c.id ? "" : undefined}
                    onclick={() => (recording = recording === c.id ? null : c.id)} onkeydown={onRecordKey}>
                    {#if recording === c.id}{t("Appuie sur les touches…")}{:else if k}<kbd>{prettyKeys(k)}</kbd>{:else}<span class="faint">{t("aucun")}</span>{/if}
                  </button>
                  {#if s.keys?.[c.id] !== undefined}<button class="btn small ghost" onclick={() => setKey(c.id, null)}>{t("Rétablir")}</button>{/if}
                  {#if clash.length}<div class="clash-msg">{t("Aussi utilisé par : {list}", { list: clash.map(labelOf).join(", ") })}</div>{/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
        <h4 class="sub">{t("Autres raccourcis")}</h4>
        <table class="keys">
          <tbody>
            {#each SHORTCUTS as [k, d] (k)}
              <tr><td><kbd>{keysText(k)}</kbd></td><td>{t(d)}</td></tr>
            {/each}
          </tbody>
        </table>
      </section>
      {/if}
      {#if show("apropos")}
      <section class="sec" data-sec="apropos">
        <h2 class="sec-title">{t(tk("À propos"))}</h2>
        <div class="about">
          <img src="/logo.svg" alt="" width="72" height="72" />
          <h3>ELORIA</h3>
          <p class="ver">{t("Version {v}", { v: version })}</p>
          {#if app.buildInfo}<p class="hint mono">{t("Compilation {commit} du {built}", { commit: app.buildInfo.commit, built: app.buildInfo.built })}</p>{/if}
          <p class="slogan">{t("Tes connaissances, magnifiquement reliées.")}</p>
          <p class="hint">{t("Un espace de travail personnel pour la recherche, l'écriture et le savoir.")}</p>
          <dl>
            <dt>{t("Auteur")}</dt>
            <dd><strong>Amédée DERA</strong> <span class="faint">(Amédée Wend-Bénédo DERA)</span></dd>
            <dt>Copyright</dt>
            <dd>© 2026 Amédée DERA</dd>
            <dt>{t("Licence")}</dt>
            <dd>MIT</dd>
            <dt>{t("Construit avec")}</dt>
            <dd>Rust, Tauri 2, Svelte 5, CodeMirror 6, SQLite</dd>
          </dl>
        </div>
      </section>
      {/if}
      {#if query && noMatch}<p class="nomatch">{t("Aucun réglage ne correspond à « {q} ».", { q: query })}</p>{/if}
    </div>
  </div>
</Modal>

<style>
  .cfg {
    margin: 0;
    padding: 10px 12px;
    background: var(--bg-sunk);
    border-radius: var(--radius);
    font-size: 12px;
    white-space: pre-wrap;
    user-select: text;
  }
  .agents {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: stretch;
  }
  .agent {
    display: grid;
    grid-template-columns: 200px minmax(0, 1fr) auto;
    gap: 8px;
    align-items: start;
  }
  .agent textarea {
    resize: vertical;
    font: inherit;
  }
  .backup-row {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 12.5px;
    padding: 2px 0;
  }
  .num {
    width: 64px;
  }
  .small {
    font-size: 12.5px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .layout {
    display: grid;
    grid-template-columns: 230px minmax(0, 1fr);
    height: min(78vh, 760px);
  }
  nav {
    border-right: 1px solid var(--line);
    padding: 12px 10px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    background: var(--bg-soft);
    overflow-y: auto;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 9px;
    margin-bottom: 8px;
    height: 32px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg-raised);
    color: var(--text-faint);
  }
  .search:focus-within {
    border-color: var(--accent);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: none;
    background: transparent;
    color: var(--text);
    font: 13px var(--font-ui);
  }
  .group {
    margin: 12px 10px 4px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-faint);
  }
  nav button {
    display: flex;
    align-items: center;
    gap: 9px;
    border: 0;
    background: transparent;
    text-align: left;
    padding: 6px 10px;
    border-radius: 6px;
    color: var(--text-soft);
    font-size: 13px;
  }
  nav button:hover {
    background: var(--bg-sunk);
    color: var(--text);
  }
  nav button.on {
    background: var(--accent-soft);
    color: var(--text);
    font-weight: 600;
  }
  .pane {
    padding: 22px 30px 34px;
    overflow: auto;
    user-select: text;
  }
  .sec + .sec {
    margin-top: 28px;
  }
  .sec-title {
    margin: 0 0 6px;
    padding-bottom: 10px;
    font-size: 18px;
    font-weight: 600;
    border-bottom: 1px solid var(--line);
  }
  /* One row per setting: name and explanation on the left, control on the right. */
  .field {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    grid-template-areas: "name ctrl" "desc ctrl";
    column-gap: 28px;
    align-items: center;
    padding: 14px 0;
    border-bottom: 1px solid var(--line);
  }
  .field > :global(*) {
    grid-area: ctrl;
    justify-self: end;
    max-width: 340px;
  }
  .field > .lbl {
    grid-area: name;
    justify-self: start;
    max-width: none;
  }
  .field > .hint {
    grid-area: desc;
    justify-self: start;
    max-width: none;
  }
  /* Wide settings (galleries, lists, text areas): control below the name. */
  .field:has(> .gallery, > textarea, > .inline, > table, > .grid2, > .swatches, > dl, > .preview, > .backup-row, > .row, > .agents, > .cfg) {
    grid-template-columns: minmax(0, 1fr);
    grid-template-areas: none;
  }
  .field:has(> .gallery, > textarea, > .inline, > table, > .grid2, > .swatches, > dl, > .preview, > .backup-row, > .row, > .agents, > .cfg) > :global(*) {
    grid-area: auto;
    justify-self: stretch;
    max-width: none;
  }
  .lbl {
    display: block;
    font-weight: 600;
    margin: 0 0 3px;
    font-size: 13.5px;
    color: var(--text);
  }
  .hint {
    font-size: 12.5px;
    color: var(--text-soft);
    margin: 2px 0 0;
    line-height: 1.5;
  }
  .hint p {
    margin: 0;
  }
  .hint p + p {
    margin-top: 4px;
  }
  .nomatch {
    color: var(--text-soft);
    padding: 30px 0;
    text-align: center;
  }
  .mono {
    font-family: var(--font-mono);
    word-break: break-all;
  }
  .mt {
    margin-top: 6px;
  }
  .short {
    width: 120px;
  }
  .preview {
    margin: 10px 0 0;
    padding: 12px 14px;
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: var(--radius);
  }
  .seg {
    display: inline-flex;
    background: var(--bg-sunk);
    border-radius: var(--radius);
    padding: 2px;
  }
  .seg button {
    border: 0;
    background: transparent;
    padding: 5px 12px;
    border-radius: 5px;
    color: var(--text-soft);
  }
  .seg button.on {
    background: var(--bg-raised);
    color: var(--text);
    box-shadow: 0 1px 2px rgb(0 0 0 / 10%);
  }
  .swatches {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .sw {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 2px solid transparent;
    padding: 0;
  }
  .sw.on {
    box-shadow: 0 0 0 2px var(--bg-raised), 0 0 0 4px currentColor;
    color: var(--text);
  }
  input[type="color"] {
    width: 30px;
    height: 26px;
    border: 0;
    background: transparent;
    padding: 0;
  }
  input[type="range"] {
    width: 100%;
    margin: 4px 0;
    box-sizing: border-box;
    accent-color: var(--accent);
  }
  .grid2 {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 0 24px;
  }
  /* Explanation of a switch: below it, in the same row. */
  .check:has(+ .hint) {
    border-bottom: 0;
    padding-bottom: 2px;
  }
  .check + .hint {
    margin: 0;
    padding: 0 60px 14px 0;
    border-bottom: 1px solid var(--line);
  }
  /* Button choices: never split over two lines. */
  .field > .seg {
    max-width: none;
  }
  .seg button {
    white-space: nowrap;
  }
  /* Two sliders side by side: each stacked (name, then slider). */
  .grid2 {
    border-bottom: 1px solid var(--line);
  }
  .grid2 .field {
    grid-template-columns: minmax(0, 1fr);
    grid-template-areas: none;
    border-bottom: 0;
  }
  .grid2 .field > :global(*) {
    grid-area: auto;
    justify-self: stretch;
    max-width: none;
  }
  /* Checkboxes shown as switches, aligned right. */
  .check {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 14px 0;
    border-bottom: 1px solid var(--line);
    font-size: 13.5px;
    cursor: pointer;
  }
  .check input[type="checkbox"] {
    order: 2;
    margin-left: auto;
    appearance: none;
    flex-shrink: 0;
    width: 36px;
    height: 20px;
    border-radius: 999px;
    background: var(--line-strong);
    position: relative;
    cursor: pointer;
    transition: background 0.15s;
  }
  .check input[type="checkbox"]::after {
    content: "";
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 2px rgb(0 0 0 / 25%);
    transition: transform 0.15s;
  }
  .check input[type="checkbox"]:checked {
    background: var(--accent);
  }
  .check input[type="checkbox"]:checked::after {
    transform: translateX(16px);
  }
  .check input[type="checkbox"]:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .inline {
    display: flex;
    gap: 6px;
  }
  .row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .ok {
    color: var(--success);
  }
  .keys {
    border-collapse: collapse;
    width: 100%;
  }
  .keys td {
    padding: 6px 4px;
    border-bottom: 1px solid var(--line);
  }
  .keys td:first-child {
    width: 46%;
  }
  .short-select {
    max-width: 340px;
  }
  textarea.input {
    width: 100%;
    resize: vertical;
    font: inherit;
  }
  .privacy {
    margin-top: 16px;
    padding: 10px 12px;
    border-left: 3px solid var(--accent);
    background: var(--bg-soft);
  }
  .journal td {
    font-size: 12px;
  }
  .hint.top {
    margin: 0 0 12px;
  }
  .sub {
    margin: 22px 0 6px;
    font-size: 13px;
  }
  .keys.editable td:first-child {
    width: 58%;
  }
  .grp {
    margin-left: 8px;
    color: var(--text-faint);
    font-size: 11.5px;
  }
  tr.custom td:first-child {
    font-weight: 600;
  }
  .kbtn {
    min-width: 120px;
    padding: 3px 8px;
    border: 1px dashed transparent;
    border-radius: var(--radius-s);
    background: transparent;
    text-align: left;
  }
  .kbtn:hover {
    border-color: var(--line-strong);
  }
  .kbtn.rec {
    border-color: var(--accent);
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .kbtn.clash kbd {
    border-color: var(--danger);
    color: var(--danger);
  }
  .clash-msg {
    color: var(--danger);
    font-size: 11.5px;
    margin-top: 3px;
  }
  .gallery {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(112px, 1fr));
    gap: 10px;
  }
  .th {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 6px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
    color: var(--text-soft);
    cursor: pointer;
    text-align: left;
  }
  .th:hover {
    border-color: var(--line-strong);
  }
  .th.on {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-soft);
    color: var(--text);
  }
  .mini {
    display: flex;
    height: 58px;
    border-radius: 5px;
    border: 1px solid transparent;
    overflow: hidden;
  }
  .mini.split .half {
    flex: 1;
  }
  .mini.custom {
    align-items: center;
    justify-content: center;
    font: 600 12px var(--font-mono);
    color: var(--text-faint);
    background: repeating-linear-gradient(45deg, var(--bg-soft), var(--bg-soft) 6px, var(--bg-sunk) 6px, var(--bg-sunk) 12px);
  }
  .side {
    width: 26%;
  }
  .lines {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 9px 8px;
  }
  .lines > span {
    height: 4px;
    border-radius: 2px;
  }
  .lines .code {
    display: flex;
    gap: 4px;
    height: 4px;
  }
  .lines .code i {
    width: 30%;
    border-radius: 2px;
  }
  .tname {
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .about {
    text-align: center;
    padding-top: 10px;
  }
  .about h3 {
    margin: 10px 0 0;
    font-size: 22px;
    letter-spacing: 0.18em;
  }
  .about .ver {
    margin: 4px 0 12px;
    color: var(--text-faint);
    font-size: 12.5px;
  }
  .about .slogan {
    font-family: var(--font-editor);
    font-size: 17px;
    font-style: italic;
    margin: 0;
  }
  .about dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 8px 16px;
    max-width: 440px;
    margin: 26px auto 0;
    text-align: left;
    font-size: 13px;
  }
  .about dt {
    color: var(--text-faint);
  }
  .about dd {
    margin: 0;
  }
  .faint {
    color: var(--text-faint);
  }
  code {
    font-family: var(--font-mono);
    font-size: 11.5px;
    background: var(--bg-sunk);
    padding: 1px 4px;
    border-radius: 3px;
  }
</style>
