# Installation

## Install a release

Download the installer for your system from the [releases page](https://github.com/a-dera/eloria/releases/latest).

| System | File | Notes |
|---|---|---|
| Windows 10/11, x64 | `Eloria_<version>_x64-setup.exe` (or `.msi`) | Per-user install, no admin rights needed (`.exe`) |
| Windows 11, ARM64 | `Eloria_<version>_arm64-setup.exe` (or `.msi`) | Native on Snapdragon PCs |
| macOS, Apple Silicon | `Eloria_<version>_aarch64.dmg` | macOS 11 or later |
| macOS, Intel | `Eloria_<version>_x64.dmg` | macOS 10.15 or later |
| Linux, x64 | `.deb` (Debian, Ubuntu), `.rpm` (Fedora), `.AppImage` (any) | Needs WebKitGTK 4.1 |

The installers are not code-signed yet. On Windows, SmartScreen may show a warning on first launch (*More info › Run anyway*). On macOS, right-click Eloria and choose *Open* the first time, or run:

```bash
xattr -dr com.apple.quarantine /Applications/Eloria.app
```

Every release lists the SHA-256 checksum of each file (`SHA256SUMS.txt`).

### Uninstalling

Uninstalling removes the application only. Your vaults (note folders) and your settings are never deleted.

### Optional tools

| Tool | Used for |
|---|---|
| [Zotero](https://www.zotero.org) 7 with [Better BibTeX](https://retorque.re/zotero-better-bibtex/) | Library mirror and stable citation keys |
| [Pandoc](https://pandoc.org) | Export to Word, ODT, LaTeX, PDF |
| [Typst](https://typst.app) | Lightweight PDF engine for Pandoc |
| [Tectonic](https://tectonic-typesetting.github.io), MiKTeX or TeX Live | LaTeX compilation (Eloria can install Tectonic for you) |

On Windows: `winget install JohnMacFarlane.Pandoc Typst.Typst`. On macOS: `brew install pandoc typst`. On Linux: your package manager.

## Build from source

### Prerequisites

- [Rust](https://rustup.rs) (stable) and [Node.js](https://nodejs.org) 20 or later.
- The Tauri system dependencies for your platform: <https://v2.tauri.app/start/prerequisites/>.
  - **Windows**: Microsoft C++ Build Tools (*Desktop development with C++*, plus the ARM64 tools on ARM PCs) and the Windows 11 SDK. WebView2 ships with Windows 11.
  - **macOS**: Xcode Command Line Tools (`xcode-select --install`).
  - **Linux (Debian, Ubuntu)**:
    ```bash
    sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev \
      libayatana-appindicator3-dev libxdo-dev libssl-dev build-essential
    ```
    Fedora: `webkit2gtk4.1-devel gtk3-devel librsvg2-devel libappindicator-gtk3-devel libxdo-devel openssl-devel`.

### Run and build

```bash
npm install
npm run app       # development mode (hot reload for the interface)
npm run bundle    # installers for the current platform, in target/release/bundle/
```

The first Rust build takes several minutes. `npm run bundle` also builds the MCP server (`eloria-mcp`) and installs it next to the application.

## Where data is stored

| Data | Location |
|---|---|
| Your notes | The folder you open (the vault), as `.md` files |
| Index, version history, trash | `<vault>/.eloria/` (the index can be rebuilt; exclude it from Git) |
| Settings | Windows `%APPDATA%\com.adera.eloria\`, macOS `~/Library/Application Support/com.adera.eloria/`, Linux `~/.config/com.adera.eloria/` |
| API keys | The system credential store (Windows Credential Manager, macOS Keychain, Linux keyring) |

Deleting `<vault>/.eloria/index.db` is safe: the index is rebuilt when the vault is opened. Keep `.eloria/history/` if you want to keep note versions.

## Troubleshooting

| Symptom | Likely cause and fix |
|---|---|
| `link.exe` or `cl.exe` not found (Windows) | The C++ build tools are missing: install them as above |
| Build error in `libsqlite3-sys` | Same cause: no C compiler |
| Blank window on launch (Windows) | WebView2 missing or blocked: `winget install Microsoft.EdgeWebView2Runtime` |
| "Pandoc not found" when exporting | Install Pandoc, or set its path in Settings › Export |
| PDF export refused | Install Typst |
| Zotero library not found | Set the Zotero data folder (the one containing `zotero.sqlite`) in Settings › Zotero |
