# Outlook `.NK2` AutoComplete, Attachments, Stats, Contacts & Office Add-Ins

In `mail2rust v3.5.0`, the **`mail2rust-outlook`** workspace crate adds dedicated pure-Rust Outlook desktop forensic and administrative modules backed by **`guap-kit`** (`guap-table`, `guap-export`, `guap-mcp`), bringing the suite to **13 workspace crates**, **22 CLI subcommands**, and **15 MCP agent tools**.

---

## 1. Outlook AutoComplete (`mail2rust nk2`)

Inspects legacy Outlook `.NK2` binary caches and modern Outlook `Stream_Autocomplete_*.dat` files without requiring Microsoft Outlook:

- **Read-Only Inspection (`mail2rust nk2 inspect`)**: Decodes binary property tags (display name, SMTP address, address type `EX`/`SMTP`, search string, drop-down weight, entry flags) into `guap-table` views with `GuapQuery` filtering and 7-format `guap-export` outputs (`csv`, `tsv`, `json`, `jsonl`, `html`, `html-vertical`, `markdown`).
- **Policy-Gated Editing (`mail2rust nk2 edit`)**: Supports `--dry-run` preview or explicit `-y` / `--allow-mutating` confirmation before modifying `.NK2` / `Stream_Autocomplete_*.dat` files on disk.
- **MCP Tools**: `mail2rust_nk2_inspect` (read-only) and `mail2rust_nk2_edit` (mutating, policy-gated).

```powershell
# Inspect an AutoComplete cache in Markdown or CSV
mail2rust nk2 inspect "%LOCALAPPDATA%\Microsoft\Outlook\RoamCache\Stream_Autocomplete_0.dat"
```

---

## 2. Attachments, Communication Statistics & Contacts (`mail2rust outlook`)

Three read-only diagnostic commands (plus policy-gated attachment stripping):

| Command | Capability | MCP Tool(s) |
| :--- | :--- | :--- |
| `outlook attach` | Enumerate all embedded/MIME attachments across PST, OST, MSG, MBOX, and EML; safely extract payloads to disk; or perform policy-gated attachment stripping (`--dry-run` preview / `-y --allow-mutating`). | `mail2rust_outlook_attach`, `mail2rust_outlook_attach_extract`, `mail2rust_outlook_attach_strip` `[MUT]` |
| `outlook stats` | Aggregate sent/received message counts, attachment volumes, and byte totals grouped by correspondent email, domain, folder, or calendar month. | `mail2rust_outlook_stats` |
| `outlook contacts` | Decode contact cards and distribution lists (`IPM.Contact`, `IPM.DistList`) from PST/OST containers, Offline Address Books (`.oab`), `.NK2` caches, or `.vcf` vCards. | `mail2rust_outlook_contacts` |

---

## 3. Microsoft Office & Outlook Add-Ins (`mail2rust office addins`)

Inspects COM, VSTO, XLL (Excel), WLL (Word), DOTM, and OTM add-ins across live Windows Registry hives (`HKCU`/`HKLM`) or offline `regf` Registry hive snapshots (`NTUSER.DAT`, `SOFTWARE`):

- **`mail2rust office addins`** / **`mail2rust_office_addins`** *(Read-Only)*: Reports ProgID, friendly name, description, manifest/DLL path, bitness/hive origin, and numeric `LoadBehavior`.
- **`mail2rust_office_addins_toggle`** *(`[MUT]`)*: Toggles add-in `LoadBehavior` only when explicit mutating policy gates are satisfied.
