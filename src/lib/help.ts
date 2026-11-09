// Content of the help page, in French and English. Light formatting in the
// text: **bold**, `code` (keys, properties, syntax). Shortcuts are read from the
// command list (always up to date); the "view" buttons open the described view.

import type { TabKind } from "./types";

export type HelpBlock = { p: string } | { ul: string[] } | { views: [string, TabKind][] } | { shortcuts: true };

export interface HelpSection {
  id: string;
  title: string;
  blocks: HelpBlock[];
}

const FR: HelpSection[] = [
  {
    id: "bienvenue",
    title: "Bienvenue",
    blocks: [
      { p: "Eloria est ton espace de recherche, d'écriture et de connaissance, pensé pour une thèse. Tes notes sont de simples fichiers Markdown (`.md`) dans un dossier, le **coffre** : tu peux les ouvrir avec n'importe quel éditeur, les sauvegarder et les synchroniser comme tu veux. L'index (`.eloria/index.db`) est jetable et se reconstruit seul." },
      { p: "Eloria lit ta bibliothèque **Zotero** sans jamais y écrire, fonctionne entièrement sur ton ordinateur, et n'utilise l'**intelligence artificielle** que si tu l'actives. L'IA n'invente jamais une référence : toute citation qu'elle propose est vérifiée contre ta bibliothèque." },
      { p: "Pour aller vite : `Ctrl+P` ouvre la **palette**, qui donne accès à toutes les commandes et à toutes les vues." },
    ],
  },
  {
    id: "prise-en-main",
    title: "Prise en main",
    blocks: [
      {
        ul: [
          "**Ouvrir un coffre** : écran d'accueil, ou « Changer de coffre » dans la palette. Un dossier existant convient ; Eloria n'y ajoute que le dossier `.eloria`.",
          "**Créer une note** : `Ctrl+N`, ou le bouton + en haut de l'arborescence. Le nom du fichier est le titre.",
          "**Relier** : tape `[[` puis le nom d'une note. Un lien vers une note qui n'existe pas encore la crée au clic.",
          "**Citer** : tape `[@` puis la clé ou le titre d'un document de ta bibliothèque. La bibliographie se construit à l'export.",
          "**Classer** : des tags `#fraude` ou hiérarchiques `#thèse/méthodo`, et des propriétés en tête de note (`statut`, `chapitres`…).",
          "**Retrouver** : recherche plein texte (`Ctrl+Maj+F`), par le sens (onglet « Par le sens »), dans les annotations Zotero et dans le texte des PDF.",
          "**Écran partagé** : `Ctrl+\\` ; glisse un onglet d'un panneau à l'autre.",
        ],
      },
    ],
  },
  {
    id: "ecrire",
    title: "Écrire",
    blocks: [
      { p: "L'éditeur affiche le Markdown mis en forme pendant que tu écris (titres, gras, listes, tableaux, encadrés, formules `$…$`, schémas Mermaid). Tape `/` en début de ligne pour insérer un bloc." },
      {
        ul: [
          "**Inclure** une note ou une section dans une autre : `![[Note]]` ou `![[Note#Section]]`.",
          "**Historique** : chaque note garde ses versions ; menu de la note › Historique, pour comparer ou restaurer.",
          "**Exporter** en Word, PDF ou HTML avec la bibliographie (Pandoc), ou convertir en document LaTeX.",
          "**Vérifier** le style académique, les citations, la **similarité avec ta bibliothèque** (passages repris mot pour mot, à citer ou reformuler) : menu de la note.",
          "**Comparer** deux notes ou deux versions, et suivre tes **statistiques de rédaction** dans le tableau de bord.",
        ],
      },
    ],
  },
  {
    id: "lire",
    title: "Lire et annoter",
    blocks: [
      { p: "Les PDF s'ouvrent dans Eloria. Sélectionne un passage pour le surligner (cinq couleurs) ou le commenter, ou ajoute une note de page. Les annotations sont enregistrées dans le coffre et recopiées dans la source du document, où elles sont cherchables." },
      { p: "Le texte des PDF est extrait à la première ouverture (ou pour tous, depuis la palette) : la recherche et Ask Eloria le lisent. Quand aucune base en ligne ne donne le résumé d'un article, Eloria le lit dans le PDF." },
    ],
  },
  {
    id: "bibliotheque",
    title: "Bibliothèque et Zotero",
    blocks: [
      { p: "Eloria lit une **copie** de la base Zotero (jamais l'original) et se resynchronise quand Zotero change. Chaque document peut devenir une **source** dans le coffre, avec ses métadonnées, son résumé, son PDF et ses annotations." },
      {
        ul: [
          "**Créer une fiche** depuis une source : la fiche de lecture contient les 13 sections de la méthode Eloria ; le bouton **Méthode Eloria** les fait remplir par l'IA à partir du PDF, à relire avant enregistrement.",
          "**Ajouter une référence** hors Zotero : par DOI, titre ou en déposant un PDF. Le résumé et les mots-clés viennent de Crossref, OpenAlex, Semantic Scholar, DataCite, Europe PMC ou de la page de l'éditeur.",
          "**Compléter les métadonnées** d'une source (résumé, mots-clés) sans rien écraser, une par une ou toutes depuis la palette.",
          "**Fiabilité** : rétractations, prépublications, revues peu citées ou absentes des index. Des faits à vérifier, pas un verdict.",
          "**Références et citations**, **articles similaires** et **réseau de citations** de ta bibliothèque (OpenAlex, Semantic Scholar).",
          "**Nouveaux documents** : la boîte de réception liste ce qui arrive dans Zotero ; « Préparer » enchaîne métadonnées, texte du PDF, fiabilité, tags et liens proposés, puis la fiche.",
        ],
      },
      { views: [["Réseau de citations", "citations"], ["Matrice de recherche", "matrix"], ["Kanban des fiches", "kanban"]] },
    ],
  },
  {
    id: "these",
    title: "Thèse",
    blocks: [
      { p: "Un chapitre est une note avec les propriétés `type: chapitre` et `chapitre: ch2` (objectif en mots : `objectif: 12000`). Rattache fiches et concepts avec `chapitres: [ch2]`." },
      {
        ul: [
          "**Espace thèse** : ordre de rédaction d'après les dépendances (`depend_de: [ch1]`), avancement, objectifs (section « ## Objectifs » à cocher), questions de recherche.",
          "**Questions de recherche** : notes `type: question` rattachées aux chapitres ; elles apparaissent aussi sur la carte des sources qu'elles citent.",
          "**Générateur de plan** : un brouillon de plan à partir des notes d'un chapitre, d'un tag ou d'un dossier, chaque point renvoyant à sa note.",
          "**Argumentation** : affirmation, arguments et preuves, objections et réponses, conclusion ; puis texte suivi ou LaTeX.",
          "**Revue de littérature** : articles inclus ou exclus, matrice, brouillon d'état de l'art (IA, citations vérifiées).",
          "**Manuscrit** : « Assembler la thèse en LaTeX » transforme les chapitres en `main.tex` et `references.bib` ; « Avant soumission » vérifie citations, figures, renvois.",
        ],
      },
      { views: [["Espace thèse", "these"], ["Tableau de bord", "dashboard"]] },
    ],
  },
  {
    id: "savoir",
    title: "Savoir",
    blocks: [
      {
        ul: [
          "**Concepts** : chaque note `type: concept`, sa définition (encadré `[!definition]`), les notes qui en parlent dans l'ordre du temps, l'évolution de sa définition, et une **fiche de connaissance** générée sans IA (ton texte autour est gardé).",
          "**Thèmes** : tes notes regroupées automatiquement par leur vocabulaire ; un tag peut être appliqué à tout un groupe.",
          "**Entités** : personnes, organisations, méthodes, technologies et lieux repérés dans tes notes.",
          "**Taxonomie** : une note « Taxonomie » en liste imbriquée devient une classification hiérarchique ; Eloria propose de classer les notes qui en mentionnent les termes.",
          "**Méthodologies** : les méthodes rencontrées et les articles qui les emploient.",
          "**Carte des connaissances** : concepts, méthodes, questions, chapitres et argumentations, reliés par leurs liens et leurs mentions.",
          "**Statistiques** : auteurs, revues et documents les plus cités, carte de chaleur des thèmes, couverture des chapitres et thèmes non couverts.",
        ],
      },
      { views: [["Concepts", "concepts"], ["Thèmes", "themes"], ["Entités", "entites"], ["Taxonomie", "taxonomie"], ["Méthodologies", "methodes"], ["Carte des connaissances", "atlas"], ["Statistiques de recherche", "statistiques"], ["Graphe", "graph"]] },
    ],
  },
  {
    id: "ia",
    title: "Intelligence artificielle",
    blocks: [
      { p: "L'IA est **désactivée par défaut**. Active-la dans Réglages › Intelligence artificielle, choisis un fournisseur (Anthropic, OpenAI, Mistral, Ollama sur ton ordinateur…) et colle ta clé : elle est rangée dans le **gestionnaire d'identifiants de Windows**, jamais dans un fichier." },
      {
        ul: [
          "**Ask Eloria** (`Ctrl+Maj+A`) : une question à tes notes, avec des réponses sourcées [S1]… et des modes (débat, contradictions, manques, synthèse, ou un agent à toi).",
          "**Réécrire** une sélection : reformuler, corriger, développer, raccourcir, ton académique, traduire, résumer. Les citations et les liens sont protégés.",
          "**Concepts** et **tags** suggérés pour une note, **méthode Eloria** pour une fiche, **brouillon d'état de l'art** pour une revue.",
        ],
      },
      { p: "Règles : l'IA **n'invente jamais une référence** (toute clé est vérifiée, une référence inconnue est retirée ou signalée « non vérifiée ») ; tout ce qu'elle produit est à relire avant d'être enregistré ; chaque appel est noté dans le journal de l'IA (date, modèle, jetons, jamais le contenu ; Réglages › Intelligence artificielle). Seuls les extraits utiles de tes notes, ou le PDF pour la méthode Eloria, sont envoyés." },
    ],
  },
  {
    id: "latex",
    title: "LaTeX",
    blocks: [
      { p: "Les fichiers `.tex` s'ouvrent dans un éditeur LaTeX avec coloration, complétion des commandes, des citations `\\cite{` et des renvois `\\ref{`. La compilation est **locale** (Tectonic, latexmk ou pdflatex), à l'ouverture, à l'enregistrement ou avec `Ctrl+Entrée` ; le PDF s'affiche à côté et les erreurs sont cliquables." },
      { p: "Le fichier `references.bib` est écrit à partir des seules références citées. Sans moteur installé, le bouton « Installer Tectonic » s'en charge." },
    ],
  },
  {
    id: "capture",
    title: "Capture et routines",
    blocks: [
      {
        ul: [
          "**Boîte de réception** : `Ctrl+Maj+V` capture le texte copié, le bouton + note une idée ; la page web en cours peut être envoyée par l'API locale (Réglages › Intelligence artificielle, « Capture web »).",
          "**Note du jour** (`Ctrl+Maj+D`), **revue hebdomadaire**, **minuteur de concentration** (barre d'état) et **objectifs** de lecture et d'écriture (tableau de bord).",
          "**Importer** un dossier Markdown (Obsidian, Notion, Logseq) depuis la palette.",
        ],
      },
    ],
  },
  {
    id: "donnees",
    title: "Fichiers, sauvegardes et confidentialité",
    blocks: [
      {
        ul: [
          "Tout reste sur ton ordinateur. Une note supprimée va dans la corbeille du coffre (`.eloria/trash`).",
          "**Sauvegardes** automatiques et **Git** (Réglages › Fichiers et dossiers).",
          "**Vérifier le coffre** : propriétés illisibles, liens cassés, PDF manquants, sources orphelines.",
          "Les recherches en ligne (Crossref, OpenAlex, Semantic Scholar…) n'envoient aucune donnée personnelle. L'**API locale** et le **serveur MCP** (pour d'autres outils d'IA) sont désactivés tant que tu ne les actives pas.",
        ],
      },
    ],
  },
  {
    id: "personnalisation",
    title: "Personnalisation et extensions",
    blocks: [
      { p: "Réglages › Personnalisation adapte Eloria à ta façon de travailler." },
      {
        ul: [
          "**Propriétés typées** : déclare qu'une propriété est une date, un nombre, une case à cocher, un choix dans une liste ou un lien vers une note ; le panneau Propriétés et la base de notes affichent alors l'éditeur adapté.",
          "**Vues enregistrées** : dans la base de notes, règle filtres, tri et colonnes puis « Enregistrer la vue ». Elle s'ouvre ensuite depuis la palette (« Vue : … »).",
          "**Commandes personnalisées** : créer une note depuis un modèle, insérer un texte (`{{date}}`, `{{heure}}`), ouvrir une vue ou une note, lancer une recherche. Elles sont dans la palette et acceptent un raccourci.",
        ],
      },
      { p: "Les **extensions** sont des dossiers `.eloria/extensions/<id>/` du coffre (manifeste et code JavaScript). Elles ajoutent des commandes, des éléments de barre d'état et réagissent aux notes ouvertes." },
      {
        ul: [
          "Une extension ne s'exécute qu'après ton accord, pour ce coffre seulement, et se désactive si son code change.",
          "Réglages › Extensions : activer, désactiver, créer une extension à partir d'un squelette commenté.",
          "**Console développeur** (`Ctrl+Alt+I`) : messages et erreurs de l'application et des extensions, évaluation de JavaScript.",
          "Guide complet pour écrire une extension : `docs/extensions.md`.",
        ],
      },
      { views: [["Console développeur", "console"]] },
    ],
  },
  { id: "raccourcis", title: "Raccourcis", blocks: [{ p: "Raccourcis de toutes les commandes qui en ont un (modifiables dans Réglages › Raccourcis). Les autres sont dans la palette (`Ctrl+P`)." }, { shortcuts: true }] },
  {
    id: "depannage",
    title: "Dépannage",
    blocks: [
      {
        ul: [
          "**Bandeau rouge « cœur plus ancien que l'interface »** : l'application n'a pas pu être recompilée ; relance-la.",
          "**Zotero introuvable** : indique le dossier de données de Zotero dans Réglages › Zotero (celui qui contient `zotero.sqlite`).",
          "**IA indisponible** : vérifie qu'elle est activée, que la clé est enregistrée (bouton « Tester »), et consulte le journal de l'IA.",
          "**Compilation LaTeX impossible** : installe Tectonic depuis l'éditeur LaTeX, ou indique le chemin de ton moteur dans les réglages.",
          "**Résultats de recherche étranges** : « Vérifier le coffre », ou supprime `.eloria/index.db` : l'index se reconstruit au prochain lancement.",
        ],
      },
    ],
  },
];

const EN: HelpSection[] = [
  {
    id: "bienvenue",
    title: "Welcome",
    blocks: [
      { p: "Eloria is your workspace for research, writing and knowledge, designed for a thesis. Your notes are plain Markdown files (`.md`) in a folder, the **vault**: open them with any editor, back them up and sync them as you like. The index (`.eloria/index.db`) is disposable and rebuilds itself." },
      { p: "Eloria reads your **Zotero** library without ever writing to it, runs entirely on your computer, and uses **artificial intelligence** only if you turn it on. The AI never makes up a reference: every citation it suggests is checked against your library." },
      { p: "Quick start: `Ctrl+P` opens the **palette**, which reaches every command and every view." },
    ],
  },
  {
    id: "prise-en-main",
    title: "Getting started",
    blocks: [
      {
        ul: [
          "**Open a vault**: welcome screen, or “Change vault” in the palette. Any existing folder works; Eloria only adds a `.eloria` folder.",
          "**Create a note**: `Ctrl+N`, or the + button above the file tree. The file name is the title.",
          "**Link**: type `[[` then a note name. A link to a note that does not exist yet creates it on click.",
          "**Cite**: type `[@` then the key or title of a document in your library. The bibliography is built on export.",
          "**Organise**: tags `#fraud` or hierarchical `#thesis/method`, and properties at the top of a note (`statut`, `chapitres`…).",
          "**Find**: full-text search (`Ctrl+Shift+F`), by meaning (“By meaning” tab), in Zotero annotations and in PDF text.",
          "**Split screen**: `Ctrl+\\`; drag a tab from one pane to the other.",
        ],
      },
    ],
  },
  {
    id: "ecrire",
    title: "Writing",
    blocks: [
      { p: "The editor shows formatted Markdown as you type (headings, bold, lists, tables, callouts, `$…$` formulas, Mermaid diagrams). Type `/` at the start of a line to insert a block." },
      {
        ul: [
          "**Embed** a note or a section in another: `![[Note]]` or `![[Note#Section]]`.",
          "**History**: each note keeps its versions; note menu › History, to compare or restore.",
          "**Export** to Word, PDF or HTML with the bibliography (Pandoc), or convert to a LaTeX document.",
          "**Check** academic style, citations, and **similarity with your library** (passages taken word for word, to cite or rephrase): note menu.",
          "**Compare** two notes or two versions, and follow your **writing statistics** on the dashboard.",
        ],
      },
    ],
  },
  {
    id: "lire",
    title: "Reading and annotating",
    blocks: [
      { p: "PDFs open inside Eloria. Select a passage to highlight it (five colours) or comment on it, or add a page note. Annotations are saved in the vault and copied into the document's source, where they are searchable." },
      { p: "PDF text is extracted the first time a PDF is opened (or for all of them, from the palette): search and Ask Eloria read it. When no online database gives an article's abstract, Eloria reads it from the PDF." },
    ],
  },
  {
    id: "bibliotheque",
    title: "Library and Zotero",
    blocks: [
      { p: "Eloria reads a **copy** of the Zotero database (never the original) and resyncs when Zotero changes. Each document can become a **source** in the vault, with its metadata, abstract, PDF and annotations." },
      {
        ul: [
          "**Create a reading note** from a source: it holds the 13 sections of the Eloria method; the **Eloria method** button has the AI fill them from the PDF, to review before saving.",
          "**Add a reference** outside Zotero: by DOI, title, or by dropping a PDF. Abstract and keywords come from Crossref, OpenAlex, Semantic Scholar, DataCite, Europe PMC or the publisher's page.",
          "**Complete the metadata** of a source (abstract, keywords) without overwriting anything, one at a time or all from the palette.",
          "**Reliability**: retractions, preprints, little-cited or unindexed journals. Facts to check, not a verdict.",
          "**References and citations**, **similar articles** and the **citation network** of your library (OpenAlex, Semantic Scholar).",
          "**New documents**: the inbox lists what arrives in Zotero; “Prepare” chains metadata, PDF text, reliability, suggested tags and links, then the reading note.",
        ],
      },
      { views: [["Citation network", "citations"], ["Research matrix", "matrix"], ["Reading-note Kanban", "kanban"]] },
    ],
  },
  {
    id: "these",
    title: "Thesis",
    blocks: [
      { p: "A chapter is a note with the properties `type: chapitre` and `chapitre: ch2` (word target: `objectif: 12000`). Attach reading notes and concepts with `chapitres: [ch2]`." },
      {
        ul: [
          "**Thesis space**: writing order from dependencies (`depend_de: [ch1]`), progress, objectives (a “## Objectifs” checklist), research questions.",
          "**Research questions**: `type: question` notes attached to chapters; they also show on the card of the sources they cite.",
          "**Outline generator**: a draft outline from the notes of a chapter, a tag or a folder, each point linking to its note.",
          "**Argument builder**: claim, arguments and evidence, objections and responses, conclusion; then running text or LaTeX.",
          "**Literature review**: included or excluded articles, matrix, state-of-the-art draft (AI, citations checked).",
          "**Manuscript**: “Assemble the thesis in LaTeX” turns chapters into `main.tex` and `references.bib`; “Before submission” checks citations, figures, references.",
        ],
      },
      { views: [["Thesis space", "these"], ["Dashboard", "dashboard"]] },
    ],
  },
  {
    id: "savoir",
    title: "Knowledge",
    blocks: [
      {
        ul: [
          "**Concepts**: each `type: concept` note, its definition (`[!definition]` callout), the notes that mention it over time, how its definition evolved, and a **knowledge card** generated without AI (your text around it is kept).",
          "**Topics**: your notes grouped automatically by vocabulary; a tag can be applied to a whole group.",
          "**Entities**: people, organisations, methods, technologies and places found in your notes.",
          "**Taxonomy**: a “Taxonomie” note with a nested list becomes a hierarchical classification; Eloria suggests classifying notes that mention its terms.",
          "**Methodologies**: the methods you have met and the articles that use them.",
          "**Knowledge map**: concepts, methods, questions, chapters and arguments, connected by links and mentions.",
          "**Statistics**: most cited authors, journals and documents, topic heatmap, chapter coverage and uncovered topics.",
        ],
      },
      { views: [["Concepts", "concepts"], ["Topics", "themes"], ["Entities", "entites"], ["Taxonomy", "taxonomie"], ["Methodologies", "methodes"], ["Knowledge map", "atlas"], ["Research statistics", "statistiques"], ["Graph", "graph"]] },
    ],
  },
  {
    id: "ia",
    title: "Artificial intelligence",
    blocks: [
      { p: "AI is **off by default**. Turn it on in Settings › Artificial intelligence, pick a provider (Anthropic, OpenAI, Mistral, Ollama on your computer…) and paste your key: it is stored in the **Windows Credential Manager**, never in a file." },
      {
        ul: [
          "**Ask Eloria** (`Ctrl+Shift+A`): a question to your notes, with sourced answers [S1]… and modes (debate, contradictions, gaps, synthesis, or your own agent).",
          "**Rewrite** a selection: rephrase, fix, expand, shorten, academic tone, translate, summarise. Citations and links are protected.",
          "Suggested **concepts** and **tags** for a note, the **Eloria method** for a reading note, a **state-of-the-art draft** for a review.",
        ],
      },
      { p: "Rules: the AI **never makes up a reference** (every key is checked; an unknown reference is removed or marked “unverified”); everything it produces is to be reviewed before saving; every call is logged in the AI log (date, model, tokens, never the content; Settings › Artificial intelligence). Only the useful excerpts of your notes, or the PDF for the Eloria method, are sent." },
    ],
  },
  {
    id: "latex",
    title: "LaTeX",
    blocks: [
      { p: "`.tex` files open in a LaTeX editor with highlighting, completion of commands, `\\cite{` citations and `\\ref{` references. Compilation is **local** (Tectonic, latexmk or pdflatex), on open, on save or with `Ctrl+Enter`; the PDF shows alongside and errors are clickable." },
      { p: "`references.bib` is written from the cited references only. Without an engine installed, the “Install Tectonic” button takes care of it." },
    ],
  },
  {
    id: "capture",
    title: "Capture and routines",
    blocks: [
      {
        ul: [
          "**Inbox**: `Ctrl+Shift+V` captures copied text, the + button notes an idea; the current web page can be sent through the local API (Settings › Artificial intelligence, “Web capture”).",
          "**Daily note** (`Ctrl+Shift+D`), **weekly review**, **focus timer** (status bar) and reading and writing **goals** (dashboard).",
          "**Import** a Markdown folder (Obsidian, Notion, Logseq) from the palette.",
        ],
      },
    ],
  },
  {
    id: "donnees",
    title: "Files, backups and privacy",
    blocks: [
      {
        ul: [
          "Everything stays on your computer. A deleted note goes to the vault's trash (`.eloria/trash`).",
          "Automatic **backups** and **Git** (Settings › Files and folders).",
          "**Check the vault**: unreadable properties, broken links, missing PDFs, orphan sources.",
          "Online lookups (Crossref, OpenAlex, Semantic Scholar…) send no personal data. The **local API** and the **MCP server** (for other AI tools) stay off until you turn them on.",
        ],
      },
    ],
  },
  {
    id: "personnalisation",
    title: "Customization and extensions",
    blocks: [
      { p: "Settings › Customization adapts Eloria to the way you work." },
      {
        ul: [
          "**Typed properties**: declare that a property is a date, a number, a checkbox, a choice from a list or a link to a note; the Properties panel and the notes database then show the matching editor.",
          "**Saved views**: in the notes database, set filters, sort order and columns, then “Save view”. It then opens from the palette (“View: …”).",
          "**Custom commands**: create a note from a template, insert text (`{{date}}`, `{{heure}}`), open a view or a note, run a search. They appear in the palette and accept a shortcut.",
        ],
      },
      { p: "**Extensions** are `.eloria/extensions/<id>/` folders in the vault (manifest and JavaScript code). They add commands and status bar items, and react to opened notes." },
      {
        ul: [
          "An extension only runs after you approve it, for this vault only, and is disabled if its code changes.",
          "Settings › Extensions: enable, disable, create an extension from a commented skeleton.",
          "**Developer console** (`Ctrl+Alt+I`): messages and errors from the app and extensions, JavaScript evaluation.",
          "Full guide to writing an extension: `docs/extensions.md`.",
        ],
      },
      { views: [["Developer console", "console"]] },
    ],
  },
  { id: "raccourcis", title: "Shortcuts", blocks: [{ p: "Shortcuts of every command that has one (editable in Settings › Shortcuts). The others are in the palette (`Ctrl+P`)." }, { shortcuts: true }] },
  {
    id: "depannage",
    title: "Troubleshooting",
    blocks: [
      {
        ul: [
          "**Red banner “core older than the interface”**: the app could not be rebuilt; restart it.",
          "**Zotero not found**: set Zotero's data folder in Settings › Zotero (the one containing `zotero.sqlite`).",
          "**AI unavailable**: check it is on, that the key is saved (“Test” button), and look at the AI log.",
          "**LaTeX cannot compile**: install Tectonic from the LaTeX editor, or set your engine's path in the settings.",
          "**Odd search results**: “Check the vault”, or delete `.eloria/index.db`: the index rebuilds on next launch.",
        ],
      },
    ],
  },
];

export function helpSections(lang: string): HelpSection[] {
  return lang === "en" ? EN : FR;
}

/** Text of a section (for search). */
export function sectionText(s: HelpSection): string {
  return [s.title, ...s.blocks.flatMap((b) => ("p" in b ? [b.p] : "ul" in b ? b.ul : "views" in b ? b.views.map((v) => v[0]) : []))].join(" ");
}

/** Light formatting, after escaping: **bold**, `code`. */
export function inline(s: string): string {
  const esc = s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  return esc.replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>").replace(/`([^`]+)`/g, "<code>$1</code>");
}
