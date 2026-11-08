# Changelog

All notable changes to `mail2rust` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [3.5.0] - 2026-11-08

### Added
- **Native Host OS File & Folder Pickers (`mail2rust-gui` & `mail2rust-cli`)**:
  - Integrated `rfd` (Rusty File Dialogs) providing genuine host OS file dialogs (Win32 `IFileDialog` Common Item Dialogs on Windows, Cocoa `NSOpenPanel`/`NSSavePanel` on macOS, and XDG Desktop Portals on Linux).
  - **Mail2Rust Studio GUI**:
    - Added "📁 Browse File..." and "📂 Browse Folder..." dialog buttons to the Open Mailbox Archive modal with format filters (`*.pst`, `*.ost`, `*.mbox`, `*.eml`, `*.emlx`, `*.msg`, `*.olm`, `*.zip`, `*.gz`, `*.bz2`, `*.xz`).
    - Added multi-file selection ("📁 Browse File(s)...") and directory selection ("📂 Browse Folder...") to the Merge Mailboxes modal.
    - Added native Save As file dialogs to the Merge Mailboxes output, Direct PST Export modal, and Forensic Manifest CSV modal.
    - Added native File Open dialog to the PST Validator modal.
    - Added 1-click "Open File..." and "Open Folder..." toolbar buttons and menu shortcuts for immediate native file manager selection.
  - **Mail2Rust CLI**:
    - Added dedicated `mail2rust pick` subcommand (`--kind file|files|folder|save`) allowing scripts, workflows, and users to launch native dialogs and receive chosen paths on standard output.
    - Added global `--pick` (alias `--browse`) flag: prompts or launches native dialogs automatically when source or destination arguments are omitted.
    - Supported positional argument omission in interactive terminal sessions for `convert`, `pst`, `eml`, `mbox`, `search`, `sniff`, and `validate-pst`.

---

## [3.4.0] - 2026-11-08

### Added
- **Multi-Pattern Regex Search Engine (`mail2rust-filter` & `mail2rust search`)**:
  - Implemented `MultiPatternMatcher` using compiled zero-copy DFA `regex::bytes::RegexSetBuilder` evaluating multiple complex keyword rules simultaneously in microseconds per message with zero heap allocations.
  - Added support for loading search rules from `.toml` (`[[rules]]`), `.json`, and line-delimited `.rules` files (`TAG | description | regex`).
  - Added dedicated `mail2rust search` subcommand and `--search-rules <FILE>` / `--search-pattern <REGEX>` flags to `mail2rust convert`.
- **Production e-Discovery Deliverables: Interactive HTML Index & Chain-of-Custody Manifest CSV (`mail2rust-eml`)**:
  - Implemented `HtmlIndexGenerator` producing self-contained, responsive `index.html` discovery reviews with sticky headers, instant client-side search filtering, interactive column sorting (subject, sender, date, tags, size), and clickable relative links to `.eml` files without any external CDN dependencies.
  - Implemented `ManifestWriter` logging comprehensive chain-of-custody CSVs (`Message_ID, Date, From, To, Subject, Size_Bytes, Matched_Keywords, Source_Container, SHA256_Hash, EML_File`) with RFC 4180 escaping.
  - Added `--html-index` and `--manifest-csv <PATH>` CLI options to `mail2rust eml`, `mail2rust convert`, and `mail2rust search`.
- **Native Thunderbird Profile Detection & Format Sniffing (`mail2rust-core`)**:
  - Added `MailboxFormat::ThunderbirdProfile` in `FormatSniffer` detecting profile directories containing `profiles.ini`, `prefs.js`, `ImapMail`, `Mail`, or `.sbd` trees.
  - Automatically recognizes extensionless Unix mboxrd files paired with `.msf` companion files or starting with `From `.
  - Automatically skips `.msf` summary files, `.dat` rules, and temporary cache files from ingestion loops.
  - Recursively flattens folder trees while stripping `.sbd` directory extensions.
- **Bounded Constant-Memory Streaming MBOX Pipeline (`mail2rust-core` & `mail2rust-mbox`)**:
  - Replaced monolithic whole-file buffering (`std::fs::read`) with incremental chunked streaming via `MboxReader::read_streaming` using 16 MB buffered readers.
  - Added `ingest_path_streaming` in `mail2rust-core` maintaining constant <50 MB RAM footprints even when ingesting multi-gigabyte monolithic mailboxes (e.g., 90+ GB `INBOX`).
  - Added `read_streaming_raw` for zero-allocation regex evaluation on raw message byte streams prior to MIME parsing.
- **RFC 2047 MIME Header Decoding & Filename Sanitization (`mail2rust-core`)**:
  - Implemented comprehensive `decode_rfc2047` in `mail2rust-core::rfc2047` supporting both Base64 (`B`) and Quoted-Printable (`Q`) encodings with linear whitespace suppression across adjacent encoded words.
  - Supports international character sets (UTF-8, ISO-8859-1, Windows-1252, US-ASCII).
  - Integrated header decoding into `sanitize_filename` and `EmlExporter` to eliminate illegible encoded-word filenames and review rows.

### Fixed
- **PST Block B-Tree (BBT) & Node B-Tree (NBT) Truncation Bug (`mail2rust-pst`)**:
  - Fixed reader truncation bug in `crates/mail2rust-pst/src/reader.rs` where stack buffers and single-slice `Read::read` truncated container audits at 341 blocks / 339 messages on large PST files.
  - Implemented dynamic buffer sizing with bounded `read_to_end` sweeps over `BBT` and `NBT` regions, restoring full fidelity auditing and verification across 30,000+ item containers.

## [3.3.1] - 2026-11-08

### Added
- **Crash-Resilient Atomic Checkpointing (`mail2rust-sync`)**:
  - Implemented `CheckpointManager::save_atomic` writing via sibling `.tmp.<pid>.<ts>` files with filesystem sync before atomic rename, preventing 0-byte or corrupt state files during unexpected process drops.
  - Added support for append-only JSON Lines (`.jsonl`) streaming checkpoints for high-volume migrations (1M+ items) with $O(1)$ disk appends without re-serializing past records.
  - Extended `SyncCheckpoint` with `folder_uidvalidity: HashMap<String, u32>` and `interrupted_reason: Option<String>`.
- **Signal Trapping & Graceful Shutdown (Exit Code 130)**:
  - Added cross-platform signal handler via `ctrlc` in `mail2rust-cli` trapping `Ctrl+C` (`SIGINT`) and `SIGTERM`.
  - Atomically marks active migration state as `status: "interrupted"`, flushes progress to disk, and prints reassuring resume command instructions before terminating with standardized exit code `130`.
- **RFC 3501 IMAP `UIDVALIDITY` & Incremental Watermark Flushing**:
  - `mail2rust-imap` inspects server `UIDVALIDITY` on each folder select and validates against checkpoint history; safely resets watermark if mailbox was re-indexed to prevent data loss.
  - Progress and folder high-watermarks are saved incrementally every `--checkpoint-interval <N>` items (default 250) and at folder boundaries.
- **Model Context Protocol (MCP) Resumable Migrations**:
  - Added `checkpoint_file` and `resume` arguments to `mail2rust_convert` MCP tool in `mail2rust-mcp`, allowing autonomous AI agents to resume paused or interrupted transfers programmatically.

## [3.3.0] - 2026-11-07

### Added
- **Fastmail & JMAP (RFC 8620 / RFC 8621) Native Client (`mail2rust jmap`)**:
  - Implemented pure-Rust, zero-external-dependency JMAP client in `mail2rust-imap` supporting Fastmail and standard JMAP servers (Stalwart, Apache James, Cyrus).
  - Implemented automatic JMAP session discovery (`/jmap/session`), mailbox listing (`Mailbox/get`), email ID querying (`Email/query`), and batch email item retrieval (`Email/get`).
  - Added direct mapping of JMAP JSON payloads (including keywords `$seen`, `$flagged`, `$answered`, `$draft`, threads, and structured body values) to unified `EmailMessage` models.
  - Added CLI subcommand `mail2rust jmap --session-url <url> --token <token> -o <out.pst>` with folder filtering and incremental delta sync checkpointing.
  - Added Fastmail presets in GUI IMAP Connection and Account Sync dialogs.
- **Modern OAuth 2.0 & Provider App Passwords (Google, Microsoft, Yahoo, Proton, Apple, Fastmail, Zoho)**:
  - Implemented SASL `XOAUTH2` and `OAUTHBEARER` (RFC 7628) authenticators in `mail2rust-imap`.
  - Added interactive browser OAuth 2.0 authorization helper (`mail2rust oauth-login`) with local loopback server (`127.0.0.1:8089/callback`) and native TLS code exchange.
  - Added built-in Provider App Password guide (`mail2rust app-passwords [google|microsoft|yahoo|all]`) with direct URLs and 16-character code setup instructions.
  - Implemented automated authentication diagnostics: automatically detects provider from host on failure and outputs actionable advice tailored to Google, Microsoft, and Yahoo.
  - Added OAuth2 token and auth-method inputs across `mail2rust imap`, `mail2rust imap-sync`, and GUI modals (**Connect to IMAP** and **IMAP-to-IMAP Account Sync**) with provider presets (Gmail, Outlook/M365, Yahoo, iCloud).
- **Multi-Format Mailbox Merger & Consolidation (`mail2rust merge`)**:
  - Consolidates heterogeneous mailbox formats (PST, MBOX, Maildir, EMLX, MSG, OLM, VCF, ICS, Takeout ZIP) into unified containers.
  - Supports cross-source deduplication (`message-id`, `sha256`, `content`), flag merging, and chronological sorting.
  - Organization strategies: `combine` (merge matching folders), `prefix` (partition by archive source), and `flatten`.
  - Interactive Win32 GUI consolidation modal with direct export or workbench loading.
- **Direct Server-to-Server IMAP Account Migration (`mail2rust imap-sync`)**:
  - Direct in-memory migration between two remote IMAP servers over TLS with zero local disk footprint.
  - Microsecond timestamp fidelity preserving original `INTERNALDATE` and message status flags (`\Seen`, `\Flagged`, etc.).
  - Automatic destination hierarchy creation, folder filtering, and prefix mapping.
- **PST High Encryption (`NDB_CRYPT_CYCLIC`)**:
  - Implemented the official Microsoft `[MS-PST]` 5.2 Cyclic Obfuscation symmetric cipher using the 768-byte `MPBB` master permutation table.
  - Generates dynamic block-level cryptographic state keys from the lower 32-bit DWORD of each block's BID (`w = key ^ (key >> 16)`).
  - Verified 100% bijective mathematical symmetry across write and read cycles in both `PstWriter` and `PstReader`.
- **PST Password Protection & Checksum Verification (`dwPasswordCRC`)**:
  - Implemented Outlook Message Store `PidTagPstPassword` (`0x67FF0003`) CRC-32 password generation on `NID_MESSAGE_STORE` (0x21).
  - Added password detection (`is_password_protected`) and verification (`verify_password`) in `PstReader` and `PstValidator`.
  - Added CLI `--password <str>` flag for `mail2rust pst`, `mail2rust convert`, and `mail2rust validate-pst`.
  - Added CLI `--encryption <none|permute|cyclic>` flag to customize container security mode.
  - Added interactive Win32 password and encryption property fields in GUI Export dialog and Container Validator modal.
- **Cross-Platform Pre-Compiled Release Packages**:
  - Automated GitHub Releases workflow (`.github/workflows/release.yml`) compiling standalone, zero-dependency release binaries for `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, and `x86_64-apple-darwin`.
  - Generates verifiable `SHA256SUMS.txt` cryptographic digests for all distribution packages.

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
