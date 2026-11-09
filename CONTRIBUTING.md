# Contributing to Eloria

Thanks for your interest! Bug reports, ideas and pull requests are welcome. For anything larger than a small fix, please open an issue first so we can agree on the approach.

## Getting started

```bash
npm install
npm run dev        # interface only, in the browser, mock backend + demo vault: http://localhost:1420
npm run app        # full application (Rust core + interface, hot reload)
```

`npm run dev` needs no Rust build: `src/lib/api.ts` routes every call to `src/lib/mock.ts`, which loads `examples/demo-vault/` in memory with a small fake Zotero library. Add `?accueil` to the URL to see the welcome screen.

See [docs/installation.md](docs/installation.md) for the system prerequisites.

## Checks

Everything must pass before a pull request is merged (the CI runs the same checks):

```bash
cargo test -p eloria-core       # core tests
npm run check                   # svelte-check: 0 errors
npm run check:i18n              # every interface string has an English translation
npm run check:commands          # every command called by the interface is defined, registered and mocked
```

## Architecture

```
crates/eloria-core/   all the logic, in Rust, independent of Tauri (testable alone)
crates/eloria-mcp/    MCP server (stdio) exposing the vault to AI assistants
src-tauri/            thin Tauri layer: commands.rs wraps Workspace, watcher.rs watches the vault
src/                  interface: Svelte 5, CodeMirror 6
  lib/api.ts          calls to the core (or to the mock backend)
  lib/mock.ts         mock backend for npm run dev
  lib/state.svelte.ts global state (tabs, panels, dialogs)
  lib/commands.ts     palette and shortcuts
  lib/editor/         CodeMirror: live preview, widgets, completion
  components/         views, panels and dialogs
examples/demo-vault/  demo vault used by the mock backend
```

Key ideas:

- **Notes are files.** A vault is a folder of `.md` (and `.tex`) files. The index `.eloria/index.db` (SQLite, FTS5) is disposable and rebuilt from the files.
- **Zotero is read-only.** `zotero.sqlite` is copied and read; Eloria never writes to it.
- **The core owns the logic.** New behaviour goes into `eloria-core` with a test; `src-tauri` only exposes it.
- **AI never invents references.** Any citation key produced by a model must exist in the vault or the library mirror; otherwise it is removed or flagged.
- **Secrets stay out of files.** API keys live in the OS credential store.

Main core modules: `workspace.rs` (facade used by the commands), `vault.rs` (safe paths, atomic writes, trash), `markdown.rs` (links, tags, citations, front matter), `index.rs` (SQLite index, search, graph), `zotero.rs` (library mirror), `fiche.rs` (sources and reading notes), `ai.rs` / `assist.rs` (AI client and safeguards), `latex.rs` / `texplus.rs` (LaTeX, SyncTeX), `extensions.rs` (vault extensions).

## Adding a feature

1. **Core**: the logic in the right `eloria-core` module, with a test.
2. **Facade**: a `Workspace` method if it combines vault, index or library.
3. **Command**: a `#[tauri::command]` in `src-tauri/src/commands.rs`, registered in `generate_handler!` in `src-tauri/src/lib.rs`.
4. **Interface**: the type in `src/lib/types.ts`, the call in `src/lib/api.ts`, a case in `src/lib/mock.ts`, then the component.
5. **Palette command** (optional): an entry in `src/lib/commands.ts`.
6. **Texts**: through `t("…")` with an English translation in `src/lib/locales/en.json`.

## Conventions

- **Interface language.** French is the source language of the interface: strings are written in French in the code (`t("Ouvrir une note")`) and translated in `src/lib/locales/en.json`. `npm run check:i18n` lists missing and unused translations. Code, comments, docs and commit messages are in English.
- **Paths** exchanged with the interface are relative to the vault, with `/` separators, and always go through `Vault::resolve` (which rejects `..` and absolute paths). Writes use `Vault::write` (atomic); deletions use `Vault::trash`.
- **Styles** use the CSS variables of `src/app.css` (`--accent`, `--bg`, `--text-soft`…), never hard-coded colours, so that every theme works.
- **File-format markers** such as `<!-- zotero:debut -->` / `<!-- zotero:fin -->` and front-matter keys (`type`, `statut`, `chapitres`…) are part of users' notes: never rename them.
- **CodeMirror**: block decorations come from the `StateField` in `livePreview.ts`, inline decorations from the `ViewPlugin`; use `pushMark` (it refuses empty ranges).

## Releases

Versions follow [Semantic Versioning](https://semver.org). To publish a release:

1. Update the version in `package.json`, `Cargo.toml` (`workspace.package`) and `src-tauri/tauri.conf.json`, and add a section to `CHANGELOG.md`.
2. Commit, then tag and push: `git tag v0.3.0 && git push origin v0.3.0`.
3. The **Release** workflow builds the installers for Windows (x64, ARM64), macOS (Apple Silicon, Intel) and Linux (deb, rpm, AppImage), and attaches them with `SHA256SUMS.txt` to a draft GitHub release. Review it, then publish.

Locally, `npm run release` builds the installers for the current platform and gathers them in `release/<version>/` with their checksums and release notes. If Eloria is running from `target/release`, set `CARGO_TARGET_DIR` to another folder first.

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE).
