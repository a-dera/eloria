# Security

## Reporting a vulnerability

Please do not open a public issue for security problems. Use GitHub's [private vulnerability reporting](https://github.com/a-dera/eloria/security/advisories/new) instead. You will get an answer within a few days.

## Design

- **Local first.** Notes stay on your computer. Eloria contacts online services only when you use an online feature (metadata lookup, AI, similar articles).
- **Zotero is read-only.** Eloria reads a copy of `zotero.sqlite` and never writes to Zotero.
- **API keys** are stored in the operating system's credential store (Windows Credential Manager, macOS Keychain, Linux keyring), never in settings files or vaults.
- **Extensions** run only after explicit approval, per vault and per SHA-256 fingerprint of their code; a changed extension is disabled until approved again. An approved extension has the same rights as the application, so only enable code you trust. See [docs/extensions.md](docs/extensions.md).
- **Local API** (web capture) is off by default, listens on `127.0.0.1` only and requires a token.
- **MCP server** is read-only by default; writing requires the `--ecriture` flag.
