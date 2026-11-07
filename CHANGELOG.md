# Changelog

All notable changes to `mail2rust` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [3.2.0] - 2026-11-07

### Added
- **Multi-Crate Rust Workspace**: Architected 10 specialized crates (`mail2rust-core`, `mail2rust-pst`, `mail2rust-eml`, `mail2rust-mbox`, `mail2rust-filter`, `mail2rust-forensics`, `mail2rust-sync`, `mail2rust-imap`, `mail2rust-cli`, `mail2rust-gui`).
- **Direct IMAP Live Ingestion (`mail2rust-imap`)**:
  - Implemented direct IMAP streaming client with native SSL/TLS encryption (`native-tls` / SChannel / SecureTransport / OpenSSL).
  - High-performance RFC 3501 protocol streaming with `BODY.PEEK[]` fetching (preserving unread/read server flags without side effects).
  - Multi-folder discovery, delimiter hierarchy splitting (`/`, `.`, `\`), and recursive mailbox tree ingestion.
  - Automatic IMAP flag mapping (`\Seen` unread state, `\Flagged`, `\Answered`, `\Draft`, `\Deleted`) and internal date fallback.
  - Seamless integration with resumable delta sync checkpoints (`high_watermark_uid` tracking per folder).
  - CLI subcommand `mail2rust imap --host <host> --username <user> --output <archive.pst>` with live progress display.
  - Classic Win32 GUI dialog `Connect to IMAP Mailbox (Live Ingestion)` with credentials masking, TLS toggle, and real-time ingestion telemetry.
- **Pure-Rust MS-PST and Offline OST Reader (`mail2rust-pst`)**:
  - Implemented pure-Rust bidirectional reading and extraction of `.pst` and `.ost` files without Outlook COM.
  - Implemented official Microsoft `[MS-PST]` 5.1 Permutative Encoding (`mpbbR` / `mpbbI`) decryption with 100% bijective roundtrip validation.
  - Traversed NBT (Node B-Tree) and BBT (Block B-Tree) hierarchies and parsed Property Context (`0xBC`) structures for MAPI tags (Subject, Body, HTML Body, Sender, Headers).
  - Built-in heuristic block-carving fallback scanner for orphaned or damaged OST/PST files.
  - Full integration into CLI (`convert`, `split`, `audit`) and GUI (`File -> Open Mailbox...`).
- **Contacts and Calendars Ingestion (`IPM.Contact` / `IPM.Appointment`)**:
  - Implemented native vCard (`.vcf`, RFC 6350 / RFC 2426) parser extracting names, organizations, titles, emails, phones, and notes.
  - Implemented native iCalendar (`.ics`, RFC 5545) parser extracting event summaries, descriptions, locations, timestamps, organizers, and attendees.
  - Automatic format sniffing for `.vcf` and `.ics` files and standard MIME envelopes.
  - Automatic routing of items into Outlook `Contacts` and `Calendar` folders with corresponding `PR_MESSAGE_CLASS_W` MAPI message properties.
- **Selective Checked Item Export & Multi-Format Targets in GUI**:
  - Added individual message checkbox selection column with Win32 `[X]`/`[ ]` glyphs and row highlight tinting.
  - Added selection commands: "Select All Filtered" (`Ctrl+A`), "Invert Selection", and "Clear Selection" (`Ctrl+D`).
  - Added multi-scope export support: `All Messages`, `Current Filter`, and `Selected Items Only`.
  - Added universal export targets: direct export to `PST` (Unicode 64-bit), `MBOX` (RFC 4155), or `EML` directory tree with Bates stamping.
- **Classic Windows Management Console (MMC / Win2k / WinXP) UI Redesign**:
  - Transformed GUI from modern dark-slate into a high-density, minimalist early 2000s Windows administrative utility design.
  - Classic Win32 menu bar (`File`, `Action`, `View`, `Tools`, `Help`) with standard keyboard shortcuts.
  - Compact utility toolbar replacing bulky ribbon banners, featuring flat 22px utility buttons and an integrated Quick Find filter.
  - Win32 Details Report ListView with clickable sortable column headers (`!`, `Flags`, `From`, `Subject`, `Received`, `Size`) and sort glyphs (`▲`/`▼`).
  - Sunken white TreeView with `[+]`/`[-]` expandable nodes, bold unread badges, and folder item counters.
  - Classic Windows Property Sheet with tabbed inspector for Body, RFC 5322 Headers, MAPI DASL tags, and Forensics with 1-click clipboard copy.
  - Multi-segment sunken status bar (`Ready`, `Active Folder`, `Message Counts`, `Selection Metadata`, `Architecture`).
  - Redesigned dialogs with etched GroupBoxes, recessed white edit fields, and standard 75x23px Win32 command buttons (`[ OK ]`, `[ Cancel ]`, `[ Apply ]`).
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
- **Cross-Platform Linux / Ubuntu CI Fix**: Configured `x11` and `wayland` features on `eframe` and added Linux system graphics/windowing library dependencies (`libx11-dev`, `libwayland-dev`, `libgl1-mesa-dev`, `libxkbcommon-dev`, etc.) to the GitHub Actions workflow to ensure Ubuntu runners build cleanly alongside Windows and macOS.
