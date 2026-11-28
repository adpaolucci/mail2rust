# Dual-GUI Guide (`mail2rust-win32` & `mail2rust-iced`)

`mail2rust` ships two pure-Rust desktop applications built on top of the exact same `mail2rust-core`, `mail2rust-pst`, `mail2rust-filter`, `mail2rust-forensics`, and `mail2rust-imap` crates:

| Feature / Surface | `mail2rust-win32` | `mail2rust-iced` |
| :--- | :--- | :--- |
| **Rendering Stack** | Native Win32 Common Controls (`winsafe 0.0.29`) | Iced `0.14` (`wgpu` + `tiny-skia` CPU fallback) |
| **Platforms** | Windows (`x86_64`, default Windows GUI) | Linux, macOS, Windows (`mail2rust gui --iced`) |
| **Zero-GPU / RDP Safety** | 100% Win32 GDI / ComCtl v6 — runs over Windows Server RDP & VMs | `ICED_BACKEND=tiny-skia` software rasterizer fallback |
| **Mailbox Browser & Folder Tree** | Hierarchical folder pane, virtualized message list, checkbox selection, quick search filter | Folder sidebar, searchable message table with read/flagged/attachment badges, multi-select checkboxes |
| **Message & MAPI Inspector** | Formatted message preview, headers tab, raw RFC 5322 MIME view, and MAPI / Hex stream inspector | Rich message reader, header table, attachment extractor, and raw MAPI property / hex viewer |
| **Direct PST / MBOX / EML Export** | Universal export dialog with PST encryption (`None`, `Permute`, `Cyclic`), password protection, deduplication, and thread auto-splitting | Modal export studio with format selector, volume splitting (`--max-pst-size`), and selective checked-item export |
| **Multi-Format Merger** | Multi-archive source list with `combine` / `prefix` / `flatten` folder strategies and cross-source deduplication | Interactive Merger modal consolidating PST, MBOX, Maildir, EML, EMLX, MSG, OLM, VCF, and ICS |
| **Live IMAP & Server-to-Server Sync** | One-click provider presets (**Gmail**, **Outlook**, **Yahoo**, **Proton Bridge**, **iCloud**, **Fastmail**) and IMAP-to-IMAP sync | Live IMAP ingestion modal and zero-disk IMAP-to-IMAP account migration modal |
| **Forensics & Bates Stamping** | eDiscovery Audit modal with Case Number, Examiner, Bates Prefix/Start, SHA-256 Merkle root, and CSV/JSON certificate export | Forensic Chain-of-Custody modal with Bates numbering and compliance certificate generation |
| **Outlook Extensions (`mail2rust-outlook`)** | Virtualized `guap-gui-win32` views & 7-format `guap-export` integration for NK2 AutoComplete, Attachments/Stats/Contacts, and Office Add-Ins | Cross-platform `guap-gui-iced` `IcedDataGrid` panels for NK2, Outlook Attachments/Stats/Contacts, and Office Add-Ins |

---

## Launching the Desktop Studios

```powershell
# Launch the default GUI (mail2rust-win32 on Windows; mail2rust-iced on Linux/macOS)
mail2rust
# or
mail2rust gui

# Launch the cross-platform Iced 0.14 studio explicitly on Windows
mail2rust gui --iced

# Open a specific mailbox file or directory on startup
mail2rust gui "D:\Archive\Mailbox.pst"

# Launch with the built-in multi-format synthetic sample dataset
mail2rust gui --sample
mail2rust gui --iced --sample
```
