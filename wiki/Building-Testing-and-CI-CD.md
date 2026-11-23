# Building, Testing & CI/CD (`mail2rust`)

`mail2rust` is a 12-crate Cargo workspace targeting **Rust 2024 Edition** with a minimum supported Rust version (MSRV) of **1.88+**.

---

## 1. Local Toolchain & Platform Dependencies

### Windows (`x86_64-pc-windows-msvc`)
- Rust `1.88+` (`rustup toolchain install stable`) with `rustfmt` and `clippy`.
- Builds all 4 binaries (`mail2rust`, `mail2rust-win32`, `mail2rust-iced`, `mail2rust-mcp`).

### Linux (`ubuntu-latest` / Debian-based)
```bash
sudo apt-get update && sudo apt-get install -y \
  libssl-dev libfontconfig1-dev libgtk-3-dev \
  libx11-dev libxi-dev libxrandr-dev libxinerama-dev libxcursor-dev \
  libgl1-mesa-dev libglu1-mesa-dev libwayland-dev libxkbcommon-dev \
  libxkbcommon-x11-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev
```

### macOS (`aarch64-apple-darwin` & `x86_64-apple-darwin`)
- Xcode Command Line Tools (`xcode-select --install`) and Rust `1.88+`.

---

## 2. Build & Verification Commands

```bash
# Format, lint, test, and build release binaries
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --verbose
cargo build --workspace --release
```

### 12 Workspace Crates
`mail2rust-core`, `mail2rust-pst`, `mail2rust-eml`, `mail2rust-mbox`, `mail2rust-filter`, `mail2rust-forensics`, `mail2rust-sync`, `mail2rust-imap`, `mail2rust-cli`, `mail2rust-win32`, `mail2rust-iced`, `mail2rust-mcp`.

---

## 3. GitHub Actions CI, `cargo-deny` & Releases

- **`.github/workflows/ci.yml`**: Runs `cargo fmt --check` (Linux), `cargo clippy -- -D warnings`, `cargo test --workspace`, and `cargo build --workspace --release` across `ubuntu-latest`, `windows-latest`, and `macos-latest`, plus `EmbarkStudios/cargo-deny-action@v2` (`deny.toml`) auditing advisories, licenses, and crate sources.
- **`.github/workflows/release.yml`**: Builds release archives for `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, and `x86_64-apple-darwin`, embeds `SBOM-crates.txt` (`cargo tree --workspace --locked`), generates per-archive `.sha256` sidecars and `SHA256SUMS.txt`, and publishes GitHub Releases.
- **`packaging/`**: Houses ready-to-submit WinGet (`packaging/winget/`) and Homebrew (`packaging/homebrew/mail2rust.rb`) manifests.
