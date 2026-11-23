# Contributing to `mail2rust`

Thank you for your interest in contributing to `mail2rust`!

## Code Architecture

The workspace is organized into 12 modular crates:
- `crates/mail2rust-core`: Core data models, RFC 2047 decoding, deduplication, and format readers (MBOX, Maildir, EMLX, MSG, OLM, MH, MMDF, Babyl).
- `crates/mail2rust-pst`: Pure-Rust MS-PST writer, reader, and validator (NDB and LTP layers).
- `crates/mail2rust-eml`: EML layout generator, HTML review index, and filename sanitizer.
- `crates/mail2rust-mbox`: RFC 4155 mboxrd generator with `>From ` escaping.
- `crates/mail2rust-filter`: Query filter engine, de-duplication, multi-pattern regex search, and thread-cluster auto-splitter.
- `crates/mail2rust-forensics`: Bates stamping, cryptographic SHA-256 chain-of-custody, and audit manifests.
- `crates/mail2rust-sync`: Incremental delta sync and atomic checkpoint persistence.
- `crates/mail2rust-imap`: Pure-Rust IMAP/TLS & JMAP client, OAuth2, and server-to-server migrator.
- `crates/mail2rust-mcp`: Model Context Protocol (`stdio` JSON-RPC 2.0) server (`mail2rust-mcp`).
- `crates/mail2rust-cli`: Command-line executable (`mail2rust`).
- `crates/mail2rust-win32`: Native zero-GPU Win32 desktop studio built on `winsafe 0.0.29` (`mail2rust-win32`).
- `crates/mail2rust-iced`: Cross-platform desktop studio built on `iced 0.14` (`mail2rust-iced`).

## Development Workflow

1. Fork and clone the repository.
2. Ensure you have Rust 1.88+ installed (`rustup update stable`).
3. Create a feature branch (`git checkout -b feature/my-enhancement`).
4. Run checks, clippy, and tests:
   ```bash
   cargo check --workspace --all-targets
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```
5. Follow Rust formatting standards:
   ```bash
   cargo fmt --all -- --check
   ```
6. Commit with clear, descriptive commit messages.
7. Open a Pull Request on GitHub.

## Guidelines
- **Zero Outlook Dependency**: All format parsers and writers must remain 100% platform-independent and free of Windows COM / MAPI requirements.
- **Testing**: Every new feature or format reader should include unit tests and, if applicable, integration tests.
- **Logging**: Use `tracing` macros (`trace!`, `debug!`, `info!`, `warn!`, `error!`) to ensure operations are observable and auditable.
- **License**: All contributions are licensed under the [Apache License, Version 2.0](LICENSE).
