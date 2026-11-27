# `mail2rust` Project Wiki

Welcome to the official documentation hub for **`mail2rust` (`v3.5.0`)** — a **100% pure-Rust `[MS-PST]` container engine, universal mailbox migration suite, forensic eDiscovery toolkit, dual desktop studio (`WinSafe Win32` + `Iced 0.14`), CLI orchestrator, and Model Context Protocol (MCP) server**.

- **Repository:** <https://github.com/adpaolucci/mail2rust>
- **Ecosystem Hub:** <https://guap.ca/mail2rust/> (part of the GUAP ecosystem)
- **Current Version:** `3.5.0` (`Rust 2024 Edition`, requires Rust `1.88+`)
- **License:** Apache-2.0

---

## `*2rust` Architecture & Tooling Reference

### Getting Started & Architecture
* **[Workspace Architecture](Architecture)**: 12-crate dependency graph, streaming ingestion pipeline, `[MS-PST]` NDB/LTP engine, and dual-GUI structure.
* **[Building, Testing & CI/CD](Building-Testing-and-CI-CD)**: Rust `1.88+` toolchain setup, platform dependencies, workspace test suites, `cargo-deny`, and GitHub Actions CI/Release workflows.

### Desktop GUIs, Security & Reference
* **[Dual-GUI Guide (Win32 & Iced)](Dual-GUI-Guide)**: Native Win32 (`mail2rust-win32` on `winsafe 0.0.29`) and cross-platform Iced (`mail2rust-iced` on `iced 0.14`) feature walkthrough.
* **[GUI Framework Decision & Comparison](GUI-Framework-Decision-and-Comparison)**: Tradeoffs between WinSafe Win32 and Iced 0.14 for elevated/RDP stability and cross-platform UI parity.
* **[Security & Stability Considerations](Security-and-Stability)**: `#![forbid(unsafe_code)]` across core/domain crates, archive traversal protection, OAuth2 PKCE loopback handling, and MSVC binary hardening.
* **[CLI Reference](CLI-Reference)**: Global flags, two-stage `Ctrl+C` handler, and all 19 `mail2rust` subcommands.
* **[MCP Server Guide](MCP-Server-Guide)**: JSON-RPC 2.0 `stdio` setup (`mcp.json`, Smithery) and all 6 MCP tools (plus **[Model Context Protocol Deep Dive](Model-Context-Protocol-MCP)**).
* **[Reports & Forensic Exports](Reports-and-Forensic-Exports)**: CSV/JSON chain-of-custody manifests, Bates numbering, RFC 6234 Merkle root verification, and air-gapped HTML deliverable indexes.
* **[Validation Rules & Status Codes](Validation-Rules-and-Status-Codes)**: `[MS-PST]` NDB/LTP structural validator checks (`!BDN`, CRC-32, NBT/BBT roots) and deterministic CLI exit codes.

---

## Mailbox Migration & Protocol Engineering Knowledge Base

### 1. Provider Restrictions & Endpoints
* **[IMAP Rate Limits and Provider Quotas](IMAP-Rate-Limits-and-Provider-Quotas)**: Exhaustive reference of concurrent socket caps, bandwidth limits, daily quotas, and error signatures across Google, Microsoft, Yahoo, Apple, Zoho, Fastmail, and Proton.
* **[Yahoo and Rogers Export Endpoints](Yahoo-and-Rogers-Export-Endpoints)**: Deep dive into Yahoo Mail and Rogers Yahoo Mail, the 10,000-message per folder IMAP restriction, and how to use `export.imap.mail.yahoo.com`.
* **[OAuth2 and App-Specific Passwords](OAuth2-and-App-Specific-Passwords)**: Authentication requirements, token exchange, and generation of App Passwords for legacy IMAP access post-Basic Auth deprecation.

### 2. Modern Protocols & Alternative Pathways
* **[Alternative Export Pathways](Alternative-Export-Pathways)**: Bypassing IMAP bottlenecks completely via modern APIs (JMAP, Microsoft Graph, Gmail REST), official bulk archives (Google Takeout, Apple Privacy, Yahoo Privacy), and local client caches (`.ost`, `.emlx`).
* **[JMAP Protocol Specification](JMAP-Protocol-Specification)**: Overview of RFC 8620 (JMAP Core) and RFC 8621 (JMAP Mail) and why JMAP is vastly superior to IMAP for high-volume exports.
* **[Model Context Protocol (MCP)](Model-Context-Protocol-MCP)**: Tool-by-tool MCP agent walkthrough for `mail2rust-mcp`.

### 3. Engineering & Client Tuning
* **[Migration Best Practices and Tuning](Migration-Best-Practices-and-Tuning)**: Algorithmic mitigation strategies, adaptive exponential backoff, concurrency clamping, UID range paging, and non-destructive inspection (`BODY.PEEK[]`).
* **[Cross-Engine Compatibility and Checkpoints](Cross-Engine-Compatibility-and-Checkpoints)**: The decoupled interoperability contract, JSON checkpoints, SQLite WAL state cache, and test harness parity between Python (`mail2mail`) and Rust (`mail2rust`).

### 4. Binary Specifications & Format Internals
* **[MAPI and CFBF Binary Specifications](MAPI-and-CFBF-Binary-Specifications)**: Microsoft Compound File Binary Format ([MS-CFB]), Outlook `.msg` ([MS-OXMSG]), and MAPI property tag specifications without requiring Windows DLLs.
* **[PST Internals and Capacity Splitting](PST-Internals-and-Capacity-Splitting)**: Unicode vs ANSI PST architecture, overcoming Outlook's 50 GB administrative ceiling, and dynamic volume auto-splitting (`--max-pst-size` / `--split-size`).
* **[Mac Mail Formats (EMLX and OLM)](Mac-Mail-Formats-EMLX-and-OLM)**: Structural analysis and extraction of Apple Mail `.emlx` (length prefix + MIME + XML plist trailer) and Outlook for Mac `.olm` ZIP archives.
* **[Mailbox Format Conversion Matrix](Mailbox-Format-Conversion-Matrix)**: Direct conversion matrix, property mappings, and multi-mailbox merging across PST, OST, MBOX, EML, EMLX, MSG, Maildir, and JMAP.
* **[Forensic Auditing and Chain of Custody](Forensic-Auditing-and-Chain-of-Custody)**: Cryptographic SHA-256 verification manifests, tamper-evident audit logs (`--audit-log`), and RFC 5322 normalization.

---

## Quick Reference: Provider Limits at a Glance

| Provider | Standard IMAP Host | Concurrency Limit | Daily / Folder Quota | Dedicated Export / Alternative Pathway |
| :--- | :--- | :--- | :--- | :--- |
| **Google Gmail** | `imap.gmail.com:993` | 15 sockets | **2.5 GB / day** download limit | **Google Takeout** (`takeout.google.com`) or **Gmail REST API** |
| **Microsoft 365** | `outlook.office365.com:993` | ~20 sockets / mailbox | Dynamic throttling (~15–20 GB/day) | **Microsoft Graph API** or **Desktop Outlook `.ost` cache** |
| **Yahoo / Rogers** | `imap.mail.yahoo.com:993` | **5 sockets** | **10,000 messages / folder** ceiling | **`export.imap.mail.yahoo.com:993`** (concurrency=1) |
| **Apple iCloud** | `imap.mail.me.com:993` | ~2 sockets safely | Heuristic bandwidth throttle | **Apple Privacy Export** or **macOS `~/Library/Mail/V10/` (`.emlx`)** |
| **Fastmail** | `imap.fastmail.com:993` | Adaptive | 500 logins / 10 minutes | **JMAP API** (`https://api.fastmail.com/jmap/api/`) |
| **Proton Mail** | `127.0.0.1:1143` *(Bridge)* | Unlimited (local) | Local GPG decryption CPU bound | **Proton Mail Export Tool** or Local Bridge with concurrency <= 4 |
