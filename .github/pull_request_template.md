## Summary
<!-- Briefly describe what this PR changes and why. -->

## Verification Checklist
- [ ] `cargo check --workspace --all-targets` passes cleanly
- [ ] `cargo fmt --all -- --check` passes cleanly
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes with zero warnings
- [ ] `cargo test --workspace` passes cleanly
- [ ] Zero Microsoft Outlook / MAPI COM runtime dependencies preserved across all mailbox parsers and `[MS-PST]` writers
