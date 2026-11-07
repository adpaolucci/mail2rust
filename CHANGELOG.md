# Changelog

All notable changes to `mail2rust` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [3.2.0] - 2026-11-07

### Added
- **Multi-Crate Rust Workspace**: Architected 9 specialized crates (`mail2rust-core`, `mail2rust-pst`, `mail2rust-eml`, `mail2rust-mbox`, `mail2rust-filter`, `mail2rust-forensics`, `mail2rust-sync`, `mail2rust-cli`, `mail2rust-gui`).
- **100% Pure-Rust Native GUI (`mail2rust-gui`)**: High-performance immediate-mode desktop workbench implemented using `eframe`/`egui`. Zero external webview, Electron, or Node.js runtime dependencies. Features classic 3-pane Outlook desktop layout (collapsible folder tree with unread badges, searchable message list with flag indicators, full message preview with RFC 5322 headers, MAPI DASL inspector, and forensic hashes).
- **Embedded Modals in Pure Rust**: Direct PST generator, Multi-Volume Auto-Splitter, Query & Date Filter Rules, eDiscovery Forensics & Bates Stamping, Pre-Flight Dry-Run Inspector, and MS-PST Container Validator dialogs.
- **CLI GUI Launch (`mail2rust gui`)**: Subcommand to launch the native GUI directly from the CLI tool.
- **Pure-Rust MS-PST Writer**: Complete `[MS-PST]` Unicode 64-bit container generator with NDB header, CRC-32 partial checksum, B-Trees (NBT/BBT), and `NDB_CRYPT_PERMUTE` compressible encryption with zero Outlook dependencies.
- **MS-PST Container Validator**: Validates header magic (`!BDN`), version, CRC-32 checksums, and BREF roots.
- **Universal Format Parsers**: Native streaming readers for MBOX (RFC 4155), Maildir/Maildir++, Apple Mail EMLX, Outlook MSG (MS-CFB / OLE2), Outlook for Mac OLM (ZIP), MH / Sylpheed / Claws Mail, MMDF, and Emacs Babyl.
- **Conversation Thread Clustering Auto-Splitter**: Splits mailboxes at custom size boundaries (e.g. 20 GB / 45 GB) while maintaining thread integrity across volumes.
- **Forensic eDiscovery Suite**: Bates stamping engine, SHA-256 and MD5 cryptographic hashing, legal hold flagging, and export of CSV/JSON chain-of-custody manifests.
- **Resumable Delta Sync Checkpointing**: Incremental checkpointing engine with folder high-watermark UID tracking.
- **High-Throughput Mutational Fuzz Testing Suite**: Deterministic `FuzzRng` PRNG testing suite executing 70,000+ mutational iterations across RFC 5322 MIME decoding, MBOX streaming, Apple Mail EMLX XML plists, Outlook MSG CFB sectors, Babyl & MMDF delimiters, PST NDB validation, format sniffing, and CLI filter syntax.
- **Parser Security & Hardening**:
  - Bound MIME recursion depth to 16 to prevent stack exhaustion from recursive multipart MIME bombs.
  - Hardened `EmailAddress::parse` against inverted angle bracket pairs (`> ... <`).
  - Hardened `EmlxReader` against out-of-order XML integer tags.
  - Hardened `EmlReader` boundary and filename extraction using key-value splitting on `=`.
  - Added ASCII validation and length guards to `parse_cli_datetime` to eliminate non-char-boundary slicing.
  - Added `f64` NaN/infinite and `usize::MAX` boundary checks to `parse_size_bytes`.
- **Comprehensive Test Suite**: Unit, integration, and fuzz tests with 100% pass rate.
