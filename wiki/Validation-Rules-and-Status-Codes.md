# Validation Rules & Status Codes (`mail2rust`)

`mail2rust-pst` includes an independent pure-Rust `[MS-PST]` container validator (`PstValidator::validate`), and `mail2rust` enforces deterministic CLI exit codes across automated pipelines.

---

## 1. `[MS-PST]` Binary Container Validation (`mail2rust validate-pst`)

Invoke structural verification directly on any `.pst` or `.ost` container without Microsoft Outlook:

```bash
mail2rust validate-pst "D:\Migration\Archive.pst"
mail2rust validate-pst "D:\Migration\Encrypted.pst" --password "secret"
```

| Validation Check | Offset / Field | Expected Value |
| :--- | :--- | :--- |
| **Header Magic (`dwMagic`)** | `0x0000..0x0004` | `!BDN` (`0x2142444E` / `0x4E44424E`) or `!BDO` (`.ost`) |
| **Client Magic (`wMagicClient`)** | `0x0008..0x000A` | `0x534D` (`"SM"`) |
| **Format Version (`wVer`)** | `0x000A..0x000C` | `23` (64-bit Unicode PST) or `14`/`15` (legacy ANSI) |
| **Partial Header CRC-32 (`dwCRCPartial`)** | `0x0004..0x0008` | CRC-32 over header bytes `0x0008..0x01DB` (`471` bytes) |
| **Encryption Method (`bCryptMethod`)** | `0x01CD` | `0x00` (`None`), `0x01` (`Permute`), or `0x02` (`Cyclic`) |
| **NBT / BBT Root BREF** | `0x00DC..0x00FC` | Non-zero BID and valid in-bounds file offset `< file_len` |
| **Password CRC (`PidTagPstPassword`)** | Message Store PC | Optional CRC-32 verification when `--password` is supplied |

---

## 2. CLI Exit Codes

| Exit Code | Meaning |
| :--- | :--- |
| `0` | Success (conversion, search, validation, audit, or `--dry-run` completed cleanly). |
| `1` | Runtime, I/O, validation, authentication, or CLI argument error. When `--error-log <PATH>` is provided, structured JSONL diagnostics are appended to the error log. |
| `130` | Graceful user cancellation via `Ctrl+C` (`SIGINT`) after saving checkpoint state (or immediate termination on second `Ctrl+C`). |
