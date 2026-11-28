# CLI Reference (`mail2rust`)

Invoking `mail2rust` with no arguments launches the platform-default desktop GUI (`mail2rust-win32.exe` on Windows; `mail2rust-iced` on Linux/macOS).

---

## 1. Global Flags & Signal Handling

All global flags can be passed before or after any subcommand:

| Flag | Default | Description |
| :--- | :--- | :--- |
| `-l, --log-level <LEVEL>` | `info` | Logging verbosity (`error`, `warn`, `info`, `debug`, `trace`). Respects `RUST_LOG`. |
| `--log-file <PATH>` | *(None)* | Write structured diagnostic logs to a file. |
| `--audit-log <PATH>` | *(None)* | Write per-message SHA-256 chain-of-custody CSV manifest (alias: `--audit-manifest`). |
| `--error-log <PATH>` | *(None)* | Optional error log output path. |
| `--bates-prefix <PREFIX>` | *(None)* | Apply sequential forensic Bates numbering (e.g., `CASE-2026`). |
| `--bates-start <N>` | `1000` | Starting Bates counter number (formatted as 7 digits, e.g., `CASE-2026-0001000`). |
| `--state-file <PATH>` | *(None)* | Path to JSON/JSONL checkpoint file for incremental tracking (alias: `--checkpoint-out`). |
| `--resume <PATH>` | *(None)* | Resume an interrupted migration from an existing checkpoint file. |
| `--checkpoint-interval <N>` | `250` | Number of messages between incremental checkpoint flushes. |
| `--dedup <STRATEGY>` | *(None)* | In-flight deduplication mode: `sha256`, `message-id`, `content`, or `none`. |
| `--dry-run` | `false` | Pre-flight inspection mode: analyzes message counts, dates, folders, and oversized attachments without writing output. |
| `--since <DATE>` | *(None)* | Inclusive lower date bound (`YYYY-MM-DD`, ISO-8601, or RFC 2822; alias: `--after`). |
| `--until <DATE>` | *(None)* | Inclusive upper date bound (`YYYY-MM-DD`, ISO-8601, or RFC 2822; alias: `--before`). |
| `--start-year <YYYY>` | *(None)* | Inclusive start year filter. |
| `--end-year <YYYY>` | *(None)* | Inclusive end year filter. |
| `--include-folder <GLOB>` | *(None)* | Case-insensitive folder glob pattern to include (repeatable). |
| `--exclude-folder <GLOB>` | *(None)* | Case-insensitive folder glob pattern to exclude (repeatable). |
| `--include-deleted` | `false` | Include messages marked with `\Deleted` / expunged flags. |
| `--pick` / `--browse` | `false` | Launch native host OS file/folder dialog when paths are omitted. |

### Two-Stage `Ctrl+C` (`SIGINT`) Handler
- **First `Ctrl+C`**: Flushes the active `CheckpointManager` state to disk and exits cleanly at the current message boundary.
- **Second `Ctrl+C`**: Terminates immediately with exit code `130`.

---

## 2. Subcommands (22 Total)

### 1. `convert` — Universal Auto-Detect Converter
```bash
mail2rust convert [SOURCE] [DESTINATION] \
  [--max-pst-size <SIZE>] [--split-by <MODE>] [--splits <N>] \
  [--pst-root <NAME>] [--flatten] [--encryption none|permute|cyclic] [--password <PASS>] \
  [--search-rules <FILE>] [--search-pattern <REGEX>] [--html-index] [--manifest-csv <PATH>]
```

### 2. `pst` — Direct 64-Bit Unicode MS-PST Generator
```bash
mail2rust pst [SOURCE] [DESTINATION] \
  [--max-pst-gb <GB>] [--max-pst-size <SIZE>] [--split-by <MODE>] [--splits <N>] \
  [--pst-root <NAME>] [--flatten] [--cluster-threads <BOOL>] [--compress <BOOL>] \
  [--encryption none|permute|cyclic] [--password <PASS>]
```

### 3. `eml` — RFC 5322 `.eml` Hierarchy Exporter
```bash
mail2rust eml [SOURCE] [DESTINATION] \
  [--layout folder|year|month|flat] [--naming subject|index|hash|message-id] \
  [--max-per-dir <N>] [--html-index] [--manifest-csv <PATH>]
```

### 4. `search` — Zero-Copy Regex e-Discovery Search & Extraction
```bash
mail2rust search [SOURCE] [DESTINATION] \
  [-r|--rules <FILE>] [-p|--pattern <REGEX>...] \
  [--format eml|pst|mbox|manifest-only] [--html-index] [--manifest-csv <PATH>] \
  [--encryption none|permute|cyclic] [--password <PASS>]
```

### 5. `mbox` — Standard `mboxrd` Exporter
```bash
mail2rust mbox [SOURCE] [DESTINATION] [--compress] [--append] [--split-size <SIZE>]
```

### 6. `split` — Thread-Preserving Multi-Volume Splitter
```bash
mail2rust split <SOURCE> <DESTINATION_DIR> \
  [--max-volume-mb <MB>] [--split-size <SIZE>] [--split-by size|year] [--splits <N>] \
  [--cluster-threads <BOOL>] [--target-ext pst|mbox|eml]
```

### 7. `audit` — Forensic Chain-of-Custody & Merkle Root Generator
```bash
mail2rust audit <SOURCE> -o|--output-csv <CSV_PATH> \
  [--output-json <JSON_PATH>] [--case-number <ID>] [--examiner <NAME>]
```

### 8. `validate-pst` — Pure-Rust `[MS-PST]` Binary Validator
```bash
mail2rust validate-pst [PST_FILE] [--password <PASS>]
```

### 9. `imap` — Live IMAP/TLS Mailbox Ingestion
```bash
mail2rust imap --host <HOST> [--port 993] -u|--username <USER> [-p|--password <PASS>] \
  [--auth-method password|xoauth2|oauthbearer] [--oauth-token <TOKEN>] \
  [--no-tls] [--insecure] -o|--output <PATH> [--folders <LIST>] \
  [--max-per-folder <N>] [--since <DATE>] [--checkpoint <PATH>]
```

### 10. `imap-sync` — Zero-Disk Server-to-Server IMAP Migration
```bash
mail2rust imap-sync \
  --src-host <HOST> [--src-port 993] --src-user <USER> [--src-pass <PASS>] \
  [--src-auth-method password|xoauth2|oauthbearer] [--src-oauth-token <TOKEN>] [--no-src-tls] \
  --dst-host <HOST> [--dst-port 993] --dst-user <USER> [--dst-pass <PASS>] \
  [--dst-auth-method password|xoauth2|oauthbearer] [--dst-oauth-token <TOKEN>] [--no-dst-tls] \
  [--insecure] [--folders <LIST>] [--folder-prefix <PREFIX>] [--max-per-folder <N>] \
  [--no-flags] [--no-dates] [--dry-run]
```

### 11. `merge` — Multi-Format Mailbox Consolidation
```bash
mail2rust merge <SOURCE_1> <SOURCE_2> [MORE_SOURCES...] -o|--output <DEST> \
  [--folder-strategy combine|prefix|flatten] [--source-prefixes <CSV>] \
  [--sort-by-date <BOOL>] [--format pst|mbox|eml] [--max-pst-size <SIZE>] \
  [--encryption none|permute|cyclic] [--password <PASS>] [--eml-layout <LAYOUT>] [--dry-run]
```

### 12. `jmap` — RFC 8620 / 8621 JMAP Ingestion
```bash
mail2rust jmap [--session-url <URL>] -t|--token <BEARER_TOKEN> -o|--output <DEST> \
  [--folders <LIST>] [--checkpoint <PATH>]
```

### 13. `oauth-login` — Interactive Loopback OAuth 2.0 Token Exchange
```bash
mail2rust oauth-login [--provider google|microsoft|yahoo] \
  --client-id <ID> --client-secret <SECRET> [--port 8089] [-o|--output <JSON_PATH>]
```

### 14. `app-passwords` — Provider App Password Setup Guides
```bash
mail2rust app-passwords [google|microsoft|yahoo|proton|all]
```

### 15. `sniff` — Magic-Byte & Directory Format Classifier
```bash
mail2rust sniff [PATH]
```

### 16. `pick` — Native OS File/Folder Picker Utility
```bash
mail2rust pick [-k|--kind file|files|folder|save] [--title <TITLE>] [--ext <EXT>] [--dir <PATH>]
```

### 17. `generate-samples` — Multi-Format Synthetic Sample Generator
```bash
mail2rust generate-samples [-o|--output-dir <DIR>]
```

### 18. `gui` — Native Desktop Studio Launcher
```bash
mail2rust gui [PATH] [--iced] [--sample]
```

### 19. `mcp` — JSON-RPC 2.0 `stdio` MCP Server
```bash
mail2rust mcp [--tools]
```

### 20. `nk2` — Outlook AutoComplete (`.NK2` / `Stream_Autocomplete_*.dat`)
```bash
mail2rust nk2 inspect [OPTIONS] <PATH>
mail2rust nk2 edit [OPTIONS] <PATH>    # Mutating: requires -y/--allow-mutating or --dry-run
```

### 21. `outlook` — Attachments, Correspondent Stats & Contacts
```bash
mail2rust outlook attach [OPTIONS] <PATH>     # Enumerate, extract, or policy-gated strip attachments
mail2rust outlook stats [OPTIONS] <PATH>      # Sent/received aggregation by correspondent, domain, folder, month
mail2rust outlook contacts [OPTIONS] <PATH>   # Decode PST/OST, OAB, NK2, or vCard contacts & distribution lists
```

### 22. `office` — Microsoft Office / Outlook Add-Ins
```bash
mail2rust office addins [OPTIONS]   # List or toggle COM/VSTO/XLL/WLL/DOTM/OTM add-ins (live Registry or offline regf)
```
