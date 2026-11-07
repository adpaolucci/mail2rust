# Contributing to `mail2rust`

Thank you for your interest in contributing to `mail2rust`!

## Code Architecture

The workspace is organized into modular crates:
- `crates/mail2rust-core`: Core data models and format readers (MBOX, Maildir, EMLX, MSG, OLM, MH, MMDF, Babyl).
- `crates/mail2rust-pst`: Pure-Rust MS-PST writer and validator (NDB and LTP layers).
- `crates/mail2rust-eml`: EML layout generator and filename sanitizer.
- `crates/mail2rust-mbox`: RFC 4155 mboxrd generator with `>From ` escaping.
- `crates/mail2rust-filter`: Query filter engine, de-duplication, and thread-cluster auto-splitter.
- `crates/mail2rust-forensics`: Bates stamping, cryptographic SHA-256 chain-of-custody, and audit manifests.
- `crates/mail2rust-sync`: Incremental delta sync and checkpoint persistence.
- `crates/mail2rust-cli`: Command-line executable (`mail2rust`).

## Development Workflow

1. Fork and clone the repository.
2. Ensure you have Rust 1.80+ installed (`rustup update stable`).
3. Create a feature branch (`git checkout -b feature/my-enhancement`).
4. Run checks and tests:
   ```bash
   cargo check --workspace
   cargo test --workspace
   ```
5. Follow Rust formatting standards:
   ```bash
   cargo fmt --check
   ```
6. Commit with clear, descriptive commit messages.
7. Open a Pull Request on GitHub.

## Guidelines
- **Zero Outlook Dependency**: All format parsers and writers must remain 100% platform-independent and free of Windows COM / MAPI requirements.
- **Testing**: Every new feature or format reader should include unit tests and, if applicable, integration tests.
- **Logging**: Use `tracing` macros (`trace!`, `debug!`, `info!`, `warn!`, `error!`) to ensure operations are observable and auditable.
