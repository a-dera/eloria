# Extensions

An extension adds commands, status bar items and reactions to opened notes to Eloria, without changing the application. It is written in JavaScript and lives in the vault, next to the notes.

This document covers everything needed to write one: folder layout, approval, the `eloria` object, events, data, styles, the developer console and a complete example.

## 1. Layout

Each extension is a folder of `.eloria/extensions/` whose name is its identifier:

```
<vault>/
  .eloria/
    extensions/
      notes-to-read/
        manifest.json   (required)
        main.js         (required, name configurable)
        styles.css      (optional)
        data.json       (written by the extension through eloria.data)
```

The easiest start is the skeleton: Settings › Extensions › *Create an extension…*. Eloria creates the folder, a manifest and a commented `main.js`.

### manifest.json

| Field | Required | Role |
|---|---|---|
| `id` | yes | Identifier: lowercase letters, digits and hyphens, 2 to 48 characters. Must be the folder name. |
| `name` | no | Displayed name (default: the identifier). Also used as the palette group. |
| `version` | no | Extension version, e.g. `1.0.0`. |
| `description` | no | One sentence shown in the settings. |
| `author` | no | Author. |
| `main` | no | Main file, `main.js` by default. A plain file name, no folder. |
| `min_app_version` | no | Minimum Eloria version, e.g. `0.3.0`. If newer than the app, the extension is marked invalid. |

Example:

```json
{
  "id": "notes-to-read",
  "name": "Notes to read",
  "version": "1.0.0",
  "description": "Counts the reading notes still to read and opens the next one.",
  "author": "Jane Doe",
  "min_app_version": "0.3.0"
}
```

Code and stylesheet are limited to 2 MB each, data to 1 MB.

## 2. Approval and security

An extension runs code with the application's rights: it can read and write notes. Eloria therefore enforces three rules.

1. **Nothing runs without approval.** An extension found in the vault is listed in Settings › Extensions, disabled. Enabling it asks for confirmation.
2. **Approval is per vault.** It is stored in the application settings (not in the vault) together with the vault path. A vault received from someone else cannot enable its own extensions.
3. **Approval is per code.** It covers the SHA-256 fingerprint of `main.js` and `styles.css`. If either changes (update, sync, manual edit), the extension is no longer loaded and shows as *Code changed: enable again*. The Rust core checks the fingerprint before serving the code.

Only enable extensions whose code you have read or whose author you know.

While developing an extension, every change to `main.js` therefore needs a new approval: Settings › Extensions › *Enable again*, then *Reload* if needed.

## 3. Execution

When loaded, the content of `main.js` runs once, in strict mode, with an `eloria` variable in scope. The file is not a module: no `import` or `export`. It may contain asynchronous code (`await` inside an `async` function).

Extensions are loaded when the vault opens, when enabled, and by the *Reload extensions* command. Before each reload, everything an extension added is removed: commands, status bar items, listeners, stylesheet; then the functions passed to `eloria.onunload` are called.

A loading error is shown in Settings › Extensions and in the developer console; it does not prevent other extensions from loading.

## 4. The `eloria` object

Note paths are relative to the vault, with `/` as separator, e.g. `Reading notes/Doe 2024.md`.

### General

| Member | Description |
|---|---|
| `eloria.id` | Extension identifier. |
| `eloria.apiVersion` | Version of this API (1). |
| `eloria.log(...values)` | Message in the developer console. |
| `eloria.warn(...values)` | Warning in the console. |
| `eloria.error(...values)` | Error in the console (red counter in the status bar). |
| `eloria.onunload(fn)` | `fn` is called when the extension is unloaded. |

### Commands: `eloria.commands`

`add({ id, name, keys?, run })` adds a command to the palette (`Ctrl+P`).

| Parameter | Description |
|---|---|
| `id` | Identifier within the extension. The command is named `ext:<extension>:<id>`. |
| `name` | Label in the palette. |
| `keys` | Default shortcut, as `Mod+Shift+K` (`Mod` is Ctrl, or Cmd on macOS). Optional; users can change it in Settings › Shortcuts. |
| `run` | Function called, synchronous or asynchronous. Errors go to the console. |

Adding the same `id` twice replaces the command.

### Notes: `eloria.notes`

| Member | Description |
|---|---|
| `list()` | All notes: `{ path, title, tags, aliases, frontmatter, words, mtime }`. `mtime` is in milliseconds. |
| `active()` | Path of the displayed note, or `null`. |
| `read(path)` | Promise of the Markdown content. The open note is saved before reading. |
| `write(path, content)` | Replaces the content of an existing note; the open editor is updated. |
| `create(title, content?, folder?)` | Creates a note and returns its path. |
| `open(path)` | Opens the note in a new tab. |
| `search(query, limit?)` | Full-text search (same syntax as the Search panel): `[{ path, title, snippet }]`. |

### Editor: `eloria.editor`

| Member | Description |
|---|---|
| `selection()` | Selected text in the active editor, or `""`. |
| `insert(text)` | Inserts at the cursor. Throws if no note is open. |
| `replaceSelection(text)` | Replaces the selection. |

### Interface: `eloria.ui`

| Member | Description |
|---|---|
| `toast(text, kind?)` | Short message; `kind`: `"info"` (default), `"success"`, `"error"`. |
| `prompt(question, value?)` | Promise of the typed text, or `null` if cancelled. |
| `statusItem({ text, title?, onClick? })` | A status bar item. Returns `{ set(text), remove() }`. Empty text hides it. |

### Events: `eloria.events`

`on(name, fn)` subscribes `fn` and returns an unsubscribe function.

| Event | Argument | When |
|---|---|---|
| `note-open` | path or `null` | On every change of displayed note, and once on subscription. |
| `notes-changed` | none | After every update of the note list (creation, edit, rename, sync). |

### Data: `eloria.data`

| Member | Description |
|---|---|
| `load()` | Promise of the saved value (`null` at first). |
| `save(value)` | Saves a JSON value in `data.json`, in the extension folder. |

`data.json` is not part of the fingerprint: saving data does not disable the extension.

## 5. Styles

A `styles.css` file is added to the page while the extension is loaded. Use the theme variables rather than fixed colours, to follow light and dark themes: `--text`, `--text-soft`, `--text-faint`, `--bg`, `--bg-soft`, `--line`, `--accent`, `--accent-soft`, `--danger`, `--warning`, `--success`, `--font-ui`, `--font-editor`, `--font-mono`.

Prefix your classes with the extension identifier (`.notes-to-read-…`) to avoid collisions.

## 6. Developer console

Palette › *Developer console* (`Ctrl+Alt+I`), or Settings › Extensions.

- The log gathers messages from the application, from extensions (`eloria.log`, uncaught errors) and from the console. Filters by level, source and text.
- The bottom line evaluates JavaScript with `eloria` (an API instance on behalf of the console), `app` (the interface state) and `api` (calls to the core) in scope. A single expression returns its value; for a block, end with `return`. Up and down arrows browse the history.
- *Reload extensions* unloads and reloads the enabled extensions.
- An error counter appears in the status bar; clicking it opens the console.

Examples:

```js
eloria.notes.list().filter((n) => n.frontmatter.statut === "à lire").length
await eloria.notes.read(eloria.notes.active())
const n = eloria.notes.list(); return n.sort((a, b) => b.words - a.words).slice(0, 5).map((x) => x.title)
```

## 7. Complete example

`manifest.json` as in section 1, then `main.js`:

```js
// Notes to read: a status bar counter, a command to open the oldest one,
// and a command that marks the open reading note as read.

const toRead = () =>
  eloria.notes
    .list()
    .filter((n) => n.frontmatter.type === "fiche" && n.frontmatter.statut === "à lire")
    .sort((a, b) => a.mtime - b.mtime);

const item = eloria.ui.statusItem({
  text: "",
  title: "Notes to read (click: open the oldest)",
  onClick: () => {
    const next = toRead()[0];
    if (next) eloria.notes.open(next.path);
  },
});

const refresh = () => {
  const n = toRead().length;
  item.set(n ? `${n} to read` : "");
};
eloria.events.on("notes-changed", refresh);
refresh();

eloria.commands.add({
  id: "next",
  name: "Notes to read: open the oldest",
  run: () => {
    const next = toRead()[0];
    if (!next) return eloria.ui.toast("Nothing to read.");
    eloria.notes.open(next.path);
  },
});

eloria.commands.add({
  id: "mark-read",
  name: "Notes to read: mark the open note as read",
  keys: "Mod+Alt+L",
  run: async () => {
    const path = eloria.notes.active();
    if (!path) return eloria.ui.toast("Open a reading note first.");
    const text = await eloria.notes.read(path);
    const updated = text.replace(/^statut:\s*à lire\s*$/m, "statut: lue");
    if (updated === text) return eloria.ui.toast("This is not a note to read.");
    await eloria.notes.write(path, updated);
    const data = (await eloria.data.load()) ?? { read: 0 };
    data.read += 1;
    await eloria.data.save(data);
    eloria.ui.toast(`Marked as read (${data.read} since install).`, "success");
  },
});

eloria.onunload(() => eloria.log("Notes to read unloaded"));
```

## 8. Good practice

- **Do not block the interface.** Avoid long loops over every note on each event; `notes-changed` can be frequent. Keep a cache if needed.
- **Write as little as possible.** `notes.write` replaces the whole file: read the note right before, change the text, write immediately. Never touch the block between `<!-- zotero:debut -->` and `<!-- zotero:fin -->`: Eloria regenerates it.
- **No network without saying so.** An extension can call `fetch`; say so in its description and never send note content without explicit consent.
- **Clean up after yourself.** What Eloria created for you is removed automatically; what you create yourself (timers, elements added to the page) is removed in `eloria.onunload`.

## 9. Compatibility

- `eloria.apiVersion` is 1. An incompatible API change will increase this number; check it if your extension relies on a member added later.
- `min_app_version` prevents loading on a version of Eloria that is too old.
- Command identifiers (`ext:<extension>:<id>`) are used by custom shortcuts: do not change them between versions.

## 10. Inside Eloria

| Role | File |
|---|---|
| Manifests, fingerprints, approvals, data, skeleton | `crates/eloria-core/src/extensions.rs` |
| Tauri commands (`extensions_list`, `extension_enable`, `extension_load`, `extension_data_get`, `extension_data_set`, `extension_create`) | `src-tauri/src/commands.rs` |
| Execution, the `eloria` object, events, unloading | `src/lib/extensions.svelte.ts` |
| Console log | `src/lib/devlog.svelte.ts` |
| Screens | `src/components/ExtensionsSettings.svelte`, `src/components/ConsoleView.svelte` |

Approvals live in `Settings::extension_grants`; `save_settings` keeps them, and only the `extension_enable` command changes them.
