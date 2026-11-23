# GUI Framework Decision & Comparison

Following the GUAP `*2rust` architecture blueprint, **`mail2rust`** pairs **`winsafe 0.0.29`** (native Win32 Common Controls on Windows) with **`iced 0.14`** (retained-mode cross-platform UI on Linux, macOS, and `--iced` on Windows), sharing 100% of the domain crates (`mail2rust-core`, `mail2rust-pst`, `mail2rust-eml`, `mail2rust-mbox`, `mail2rust-filter`, `mail2rust-forensics`, `mail2rust-sync`, and `mail2rust-imap`).

---

## Why Dual Native Pure-Rust GUIs?

| Criterion | `mail2rust-win32` (`WinSafe 0.0.29`) | `mail2rust-iced` (`Iced 0.14`) | Electron / WebView2 / Python Tkinter GUIs |
| :--- | :--- | :--- | :--- |
| **Runtime Dependencies** | **Zero** — links directly against OS `user32.dll` / `comctl32.dll` | **Zero** — self-contained static binary | 150–300+ MiB Chromium/Node.js or Python interpreter + Tcl/Tk |
| **GPU / Hardware Acceleration** | **Zero** — pure Win32 GDI & Common Controls v6 | `wgpu` GPU renderer + **`tiny-skia` CPU fallback** | GPU process / WebView2 runtime required |
| **Windows Server RDP & VM Stability** | **100% immune to headless GPU / DXGI driver issues** | Safe via `ICED_BACKEND=tiny-skia` | Frequent black screens or elevated admin WebView2 failures |
| **Platform Coverage** | Windows (`x86_64` / `aarch64`) | **Linux, macOS, Windows** | Multi-OS with heavy memory footprint |
| **Memory Usage (Idle)** | **~8–14 MiB** | **~25–45 MiB** | 250–600+ MiB |
| **System File Dialogs** | Native COM `IFileOpenDialog` / `IFileSaveDialog` | `rfd` native OS file/folder pickers | IPC bridge dialogs |

---

## Key Technical Implementation Notes

1. **Common Controls v6 & High-DPI Manifest (`mail2rust-win32`)**:
   - `crates/mail2rust-win32/build.rs` embeds `app.manifest` (`Microsoft.Windows.Common-Controls` v6 + `PerMonitorV2` High-DPI awareness) so native ListView, TreeView, Tab, and StatusBar controls render with crisp modern styling at 125%–250% DPI scaling.
2. **CLI-to-GUI Dispatcher (`mail2rust gui`)**:
   - Running `mail2rust` with no subcommands or `mail2rust gui` spawns `mail2rust-win32.exe` on Windows (automatically falling back to `mail2rust-iced.exe` if `mail2rust-win32.exe` is not alongside the binary) and `mail2rust-iced` on Linux and macOS.
   - Passing `--iced` (`mail2rust gui --iced`) selects the Iced 0.14 interface on any platform, forwarding `--sample` and optional mailbox `<PATH>` arguments.
3. **Zero-Outlook Cross-Platform Parity**:
   - Unlike legacy Windows email migration utilities that wrap Outlook's `MSMAPI32.DLL` or `pywin32` COM objects, both `mail2rust-win32` and `mail2rust-iced` call `mail2rust-pst` directly in pure Rust — allowing Linux and macOS forensics analysts to inspect, validate, split, merge, and generate 64-bit Unicode `.pst` archives identically to Windows users.
