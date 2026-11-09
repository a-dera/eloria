# User guide

This guide covers the basics. The **built-in help** (`F1`, or the question mark in the left bar) describes every feature, with buttons that open each view, in the interface language.

Shortcuts are written for Windows and Linux; on macOS, `Ctrl` is `Cmd`.

## The vault

A vault is an ordinary folder. Every note is a `.md` file. You can open an existing Obsidian vault as is, and use Git or any sync service (OneDrive, Syncthing, Dropbox): Eloria notices files changed outside the app and updates its index.

Deleted notes go to an internal trash (`.eloria/trash/`), never straight to oblivion.

## The window

```
┌──┬───────────────┬──────────────────────────────┬───────────────┐
│  │ Files         │ Tabs ─────────────────────── ▢ │ Links · Outline│
│L │ Search        │                              │ Local graph   │
│e │ Tags          │   Note (live preview)        │ Properties    │
│f │ Zotero        │                              │ Ask Eloria    │
│t │ Thesis, Inbox │                              │               │
├──┴───────────────┴──────────────────────────────┴───────────────┤
│ Status bar: notes, references, words, characters, save state      │
└──────────────────────────────────────────────────────────────────┘
```

- **Left bar**: panels (Files, Search, Tags, Zotero, Thesis, Inbox), then views (dashboard, graph, Kanban, notes database, research matrix, timeline, daily note, thesis workspace, concepts, topics, entities, taxonomy, methodologies, knowledge map, statistics), then tools. Help, the command palette and settings stay at the bottom.
- **Tabs**: drag to reorder or to move to the other pane; middle-click to close; `Ctrl+\` splits the screen.
- **Right panel**: backlinks with their context, references cited in the note, unlinked mentions, outline, local graph, editable properties, Ask Eloria.
- **Focus mode** (`F11`): everything but the text disappears.

## Writing

**Live preview** hides Markdown syntax except on the line you are editing. `Ctrl+Shift+P` switches to raw Markdown.

| You type | Result |
|---|---|
| `/` at the start of a word | Insert menu: headings, lists, tasks, callouts, toggles, tables, code, diagrams, math, date, template |
| `[[` | Link to a note (completion on titles and aliases) |
| `[[Note#Section]]`, `[[Note\|label]]` | Link to a section, link with a label |
| `![[image.png]]`, `![[image.png\|300]]` | Image from the vault (optional width) |
| `[@` | Zotero citation (completion on author, year, title, key) |
| `#idea`, `#thesis/ch2` | Tags (hierarchical with `/`) |
| `==text==` | Highlight |
| `$x^2$`, `$$ … $$` | Inline math, display math (KaTeX) |
| ` ```mermaid ` | Diagram: flowchart, sequence, Gantt, mind map, class diagram… |
| `> [!tip] Title` | Coloured callout (note, info, tip, warning, danger, question, quote, summary, definition, example) |
| `> [!note]- Title` | Callout folded by default (a Notion-style toggle) |
| `- [ ] task` | Clickable checkbox |

Pasting (`Ctrl+V`) or dropping an image saves it in the attachments folder and inserts the link.

On the line being edited, follow a link with `Ctrl+click`; elsewhere a plain click is enough. `Alt+click` opens it in the other pane. Clicking a link to a note that does not exist creates it.

### Tables

Outside editing, a table is rendered and editable cell by cell: click a cell, `Tab` / `Shift+Tab` to move (at the end of the table, `Tab` adds a row), `Enter` to confirm, `Esc` to cancel. A toolbar on hover adds or removes rows and columns; *Source* shows the Markdown. `Ctrl+Alt+T` aligns the columns.

### Embeds

`![[Note]]` or `![[Note#Section]]`, alone on its line, shows the content of the note (or section) read-only, with an *Open* button.

### Properties

The YAML block at the top of a note (`---`) is shown as a property panel, editable in place or from the Properties tab on the right. Properties feed the notes database, the Kanban board and the thesis views:

| Property | Role |
|---|---|
| `titre` (title) | Displayed title (otherwise the first `# Heading`, otherwise the file name) |
| `aliases` | Other names for `[[…]]` links |
| `tags` | Tags |
| `type` | `fiche` (reading note), `concept`, `chapitre` (chapter), `journal`… (view filters) |
| `statut` (status) | `à lire`, `en cours`, `lue`, `exploitée` (Kanban columns) |
| `chapitres` | Thesis chapters the note belongs to, e.g. `[ch2, ch4]` |
| `chapitre`, `objectif` | On a chapter note: identifier and word-count goal |

Property names are French because they are part of the note format and stay stable across interface languages.

In Settings › Customization, a property can be given a **type** (number, date, checkbox, choice from a list, several values, link to a note): the Properties panel and the notes database then show a suitable editor.

## Views

- **Graph** (`Ctrl+G`): notes and links, with tags, orphan notes and notes to create as options, filters by folder and text. Hover highlights neighbours; scroll to zoom; drag to move; click to open.
- **Local graph** (right panel): the neighbourhood of the note, depth 1 to 3.
- **Kanban**: cards are notes of a type, columns are values of a property. Dragging a card changes the property in the file.
- **Notes database**: a filterable table (folder, type, tag, text), sortable, with editable cells and CSV export. *Save view* keeps filters, sort order and columns; the view then opens from the palette (*View: …*).
- **Timeline**: reading notes by publication year or reading year.
- **Thesis workspace**: chapters, word counts against goals, objectives, research questions, writing order from chapter dependencies.
- **Knowledge views**: concepts (with the timeline of each concept), topics (automatic clustering), entities (people, organisations, methods, technologies, places), taxonomy, methodologies, knowledge map, research statistics.

## Templates and daily note

Notes in the templates folder are templates. Variables: `{{title}}`, `{{date}}`, `{{time}}`, `{{datetime}}`. `Ctrl+Shift+N` creates a note from a template; in the template palette, `Shift+Enter` inserts the template into the open note.

The daily note (`Ctrl+Shift+D`) is created in the journal folder as `YYYY-MM-DD.md`, from the `Journal` template if it exists.

## History

A version is saved at most every 5 minutes while you write (configurable), and 100 versions are kept per note. Note menu `…` › History: line-by-line comparison with the current text, and restore.

## Export

Note menu `…` › Export (`Ctrl+Shift+E`): Word, PDF, ODT, LaTeX (through Pandoc) or HTML (no dependency). `[@key]` citations are resolved and the bibliography is appended; the citation style is set in Settings › Export (a `.csl` file). Internal links become plain text and the Zotero annotation block can be left out.

## Shortcuts

| Shortcut | Action |
|---|---|
| `Ctrl+P` | Command palette |
| `Ctrl+O` | Open a note or any vault file (`.tex`, `.bib`, PDF, image); creates the note if missing |
| `Ctrl+N` / `Ctrl+Shift+N` | New note / from a template |
| `Ctrl+Shift+F` | Search the vault (`"phrase"`, `#tag`, `-exclude`) |
| `Ctrl+F` | Find and replace in the note |
| `Ctrl+G` | Graph |
| `Ctrl+Shift+D` | Daily note |
| `Ctrl+Shift+A` | Ask Eloria (AI) |
| `Ctrl+Shift+J` | Writing assistant on the selection (AI) |
| `Ctrl+Shift+V` | Capture the clipboard into the inbox |
| `Ctrl+Shift+C` | Insert a Zotero citation |
| `Ctrl+Shift+E` | Export |
| `Ctrl+\` | Split the screen |
| `Ctrl+W`, `Ctrl+Tab` | Close, switch tab |
| `Ctrl+B`, `Ctrl+I`, `Ctrl+E`, `Ctrl+K` | Bold, italic, code, link |
| `Ctrl+Shift+H`, `Ctrl+Shift+X` | Highlight, strikethrough |
| `Ctrl+Alt+1…6`, `Ctrl+Alt+0` | Headings, paragraph |
| `Ctrl+L` | Checkbox |
| `Ctrl+=`, `Ctrl+-` | Larger, smaller text |
| `Ctrl+,` | Settings |
| `Ctrl+Alt+I` | Developer console |
| `F1`, `F2`, `F11` | Help, rename, focus mode |

Every shortcut can be changed in Settings › Shortcuts.

## Customizing

Settings › Appearance: theme (a gallery of 14 themes, including One Dark Pro, Dracula, Nord, Solarized, Gruvbox, Catppuccin and GitHub, plus your own CSS files), accent colour, text font (Literata, Source Serif, Lora, Inter, JetBrains Mono, or any installed font), interface font, text size, line height, column width, heading size. The bundled fonts work offline.

Settings › Customization: typed properties, saved views, and **custom commands** (create a note from a template in a chosen folder, insert text with `{{date}}` and `{{heure}}` replaced, open a view or a note, run a search). Custom commands appear in the palette and can have a shortcut.

Settings › Extensions: extensions are JavaScript code stored in the vault; none runs without your approval. See [extensions.md](extensions.md).

## Going further

The built-in help (`F1`) covers the rest: library and Zotero ([zotero.md](zotero.md)), the PDF reader and annotations, AI, LaTeX, the thesis workspace, knowledge views, capture and routines, backups, Git and privacy.
