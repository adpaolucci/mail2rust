# Reports & Forensic Exports (`mail2rust-forensics` & `mail2rust-eml`)

`mail2rust` provides tamper-evident chain-of-custody reporting, sequential Bates stamping, RFC 6234 SHA-256 Merkle root computation, and self-contained interactive HTML evidence deliverable indexes.

---

## 1. Chain-of-Custody Audit Manifests (`mail2rust audit` & `--audit-log`)

Run a standalone forensic audit over any PST, OST, MBOX, Maildir, EML tree, or archive:

```bash
mail2rust audit "D:\Evidence\Mailbox.pst" \
  --output-csv "D:\Audit\manifest.csv" \
  --output-json "D:\Audit\audit_certificate.json" \
  --case-number "CASE-2026-001" \
  --examiner "A. Paolucci" \
  --bates-prefix "EVD" \
  --bates-start 1000
```

Or attach `--audit-log <PATH>` globally to any `convert`, `pst`, `eml`, `mbox`, `imap`, or `jmap` command.

### Computed Forensic Artifacts
- **Per-Message SHA-256 Digest**: Deterministic digest computed from normalized message payloads and headers.
- **Balanced SHA-256 Merkle Root (`ForensicEngine::compute_root_hash`)**: Cryptographic root digest summarizing all exported items into a single verifiable seal.
- **Sequential Bates Numbers**: 7-digit zero-padded identifiers (`<PREFIX>-0001000`, `<PREFIX>-0001001`, ...).
- **CSV Formula Injection Neutralization**: Cells starting with `=`, `+`, `-`, `@`, `\t`, or `\r` are safely neutralized for spreadsheet review.

---

## 2. Air-Gapped HTML Deliverable Index (`--html-index`)

When exporting EML production trees via `mail2rust convert`, `mail2rust eml`, or `mail2rust search`:

```bash
mail2rust search "D:\Evidence\Case.pst" "D:\Production" \
  -p "(?i)wire\s+transfer" \
  --format eml \
  --html-index \
  --manifest-csv "D:\Production\search_manifest.csv"
```

The generated `index.html` enforces a strict air-gapped **Content-Security-Policy** (`default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; img-src data:; base-uri 'none'; form-action 'none';`) with zero external network dependencies, providing instant client-side search and column sorting over exported `.eml` files.

---

## 3. Incremental Sync Checkpoints (`--state-file` & `--resume`)

`mail2rust-sync` persists atomic JSON/JSONL migration checkpoints tracking processed `Message-ID`s, SHA-256 digests, and IMAP/JMAP folder high-watermarks so interrupted transfers resume without duplicating already committed items.
