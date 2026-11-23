# Security & Stability Considerations (`mail2rust`)

---

## 1. Memory Safety & `#![forbid(unsafe_code)]`

11 of the 12 crates in `mail2rust` compile with **`#![forbid(unsafe_code)]`**:
- `mail2rust-core`, `mail2rust-pst`, `mail2rust-eml`, `mail2rust-mbox`, `mail2rust-filter`, `mail2rust-forensics`, `mail2rust-sync`, `mail2rust-imap`, `mail2rust-mcp`, `mail2rust-cli`, and `mail2rust-iced`.
- `unsafe` is strictly isolated to `mail2rust-win32` for native Win32 Common Control (`winsafe`) windowing and Vista+ COM dialog handles.

---

## 2. Adversarial Input & Parser Hardening

- **ZIP / Archive Traversal Guards**: `ArchiveReader` and `OlmReader` strip `.` and `..` path traversal segments and sanitize filenames before creating output paths.
- **OLE2 (`[MS-CFB]`) Cycle Bounds**: `MsgReader` enforces bounded sector traversal so malformed `.msg` files cannot trigger infinite loops or OOM panics.
- **Mutational Fuzzing**: Core MIME, MBOX, EMLX, MSG, OLM, and PST parsers are tested against 70,000+ mutational fuzz inputs without panicking.
- **CSV Formula Neutralization**: Forensic CSV manifests neutralize leading `=`, `+`, `-`, `@`, `\t`, and `\r` characters to prevent spreadsheet formula execution.

---

## 3. Authentication, Network & Release Hardening

- **Strict Local-Only Default**: Offline format conversions, PST reading/writing, validation, search, and auditing perform zero network calls.
- **OAuth 2.0 Loopback Isolation**: `mail2rust oauth-login` binds strictly to `127.0.0.1` for RFC 8252 PKCE redirects and exchanges authorization codes over TLS.
- **Air-Gapped HTML Reports**: `--html-index` outputs enforce `Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; img-src data:; base-uri 'none'; form-action 'none';`.
- **MSVC Binary Mitigations**: Windows release builds enable Control Flow Guard (`-C control-flow-guard=on`), `/CETCOMPAT`, and `/STACK:16777216`.
- **Vulnerability Reporting**: See [`SECURITY.md`](https://github.com/adpaolucci/mail2rust/blob/main/SECURITY.md) or submit a private advisory via GitHub Security Advisories.
