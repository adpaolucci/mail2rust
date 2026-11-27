# Packaging Manifests (`WinGet` & `Homebrew`)

> **Templates, not published packages.** The `sha256` / `InstallerSha256` fields are zeroed placeholders because the release artifacts do not exist at commit time. Run `python scripts/packaging_hashes.py --repo mail2rust --version <ver> --assets-dir <dir>` at release time to fill in the real hashes.

This directory stores ready-to-submit package manager manifests matching published GitHub Releases for **`mail2rust`**.

## 1. Windows Package Manager (`winget`)

Validate and submit the YAML manifests in `packaging/winget/manifests/a/adpaolucci/mail2rust/<version>/`:

```powershell
# Local validation using winget CLI
winget validate --manifest .\packaging\winget\manifests\a\adpaolucci\mail2rust\3.5.0

# Or submit an automated PR to microsoft/winget-pkgs using wingetcreate
wingetcreate submit .\packaging\winget\manifests\a\adpaolucci\mail2rust\3.5.0
```

Installed portable command aliases: `mail2rust`, `mail2rust-win32`, `mail2rust-iced`, `mail2rust-mcp`.

## 2. Homebrew (`macOS` & `Linux`)

Copy `packaging/homebrew/mail2rust.rb` into `Formula/mail2rust.rb` of a tap repository (e.g., `adpaolucci/homebrew-tap`):

```bash
brew install adpaolucci/tap/mail2rust
# Or test locally directly from file:
brew install --formula ./packaging/homebrew/mail2rust.rb
brew test mail2rust
```
